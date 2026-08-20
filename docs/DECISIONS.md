# Decisions

What was chosen while building the phone app, and why. Two rounds: the original
build, and the changes that came out of your review of it.

## Run it

```sh
cargo run -p server-main               # terminal 1 — talks to Plaid Sandbox
cd crates/phone-main && trunk serve    # terminal 2 — the app, on :8081
```

The server answers on `:8082`. With no `.env` it serves the captured Sandbox
fixtures instead of calling out, so the app works from a clean checkout.

Widen the browser window past 460x880 and the app draws itself in a
phone-shaped frame; narrower than that it fills the viewport.

**Your `.env` was changed once:** the `PLAID_ACCESS_TOKEN` in it had gone stale
(Plaid answered `ITEM_LOGIN_REQUIRED`). A fresh **Sandbox** Item was minted with
`/sandbox/public_token/create` + `/item/public_token/exchange` and the token
swapped in. No Production Item was consumed. `.env` is gitignored.

## From the review

### Panics

`docs/STYLE.md` now says production code does not panic, and the code follows
it. No `assert!` outside tests anywhere in the workspace, and the `unwrap`s and
`expect`s in the app, the server and the shared crates are gone.

Three patterns replaced them, in order of preference:

- **`Result`** for anything depending on input or IO. The Sandbox guard in
  `runtime_plaid::fetch_all` used to be an `assert_eq!`; it returns an error
  now, with the same practical effect — the caller has no way past it.
- **`unreachable!` with a reason** where the case is provably impossible from
  the lines just above, which is the pattern you suggested and reads much better
  than the alternatives:
  ```rust
  let Some(month) = months.last_mut() else {
      unreachable!("a month was just pushed")
  };
  ```
- **A logged fallback** where continuing is genuinely better than stopping.
  `fetch_fixtures` used to `expect("fixture parses")` on data compiled into the
  binary; it now logs an error and returns empty. A broken fixture is a build
  mistake, and a test covers the happy path so it cannot reach a release
  unnoticed.

`debug_assert!` went in liberally, as you suggested, and **one of them
immediately earned its keep**: `"the ledger should arrive newest first"` in
`StateApp` failed the moment the tests ran, because `examples/snapshot.rs` was
serialising `fetched.added` (Plaid's order) rather than the applied ledger
(newest first). The fixture the app's tests read disagreed with what the running
server serves, and nothing else would have noticed.

### `Timestamp` is a real type

`type Timestamp = u64` is gone. It is now `Timestamp { t: u64 }` with:

- `Add`/`Sub<Duration>` — shifting by a duration gives a timestamp.
- `Sub<Timestamp> = Duration` — **the gap between two points is a duration, not
  a third point.** This is most of the value: `end - start` used to typecheck as
  a `Timestamp` and mean nothing.
- `days_until`, `day_index`, `start_of_day` — the operations date grouping and
  "how many days apart" actually wanted, in one place instead of re-derived per
  caller.
- `saturating_add_signed`, for timezone offsets.
- No `now()`. Reading a clock is IO.

The field is `pub` for one reason: `uniffi::custom_newtype!` needs to reach it.
That macro turned out not to be usable anyway (orphan rule — `Timestamp` is not
local to `phone-state`), so the one place a timestamp crosses FFI —
`RangeSelection::Between`, a date the user picked — carries a `u64` and converts
at the boundary via `window_bounds()`. Worth revisiting if a second one appears.

### Time moved into `handle_events`

`handle_events(now, &events, &mut actions)`. You were right that this is
cleaner: a batch happens at one instant, and `EventUi::Started { now }` was
asking the runtime to invent a reading it had no business inventing.

One wrinkle worth knowing: the app needs the *server's* clock rather than the
device's, so a phone with a wrong date does not render a confidently wrong "in
3 days". The machine keeps the server's reading paired with the device reading
it arrived at, and `now()` returns the former advanced by however much the
latter says has passed. Trusting the device outright renders wrong dates;
trusting the server outright freezes "in 3 days" until the next fetch. There is
a test.

### `eval!`

Built as `crates/api-eval`, exactly the shape you described:

```rust
eval!(&mut state,
    < EventClient::SnapshotRequested { force: false },
    > ActionSource::FetchPlaid { cursor: None },
);
```

Adjacent events batch; adjacent actions are what that batch produced. Assertion
is an `insta` snapshot, so **`PartialEq` came off every event and action** — you
were right that it spreads, and the snapshot output is more readable than an
equality failure anyway.

Two details that came up building it:

- A test calling `eval!` twice would have both calls start their step counter at
  zero and overwrite each other's snapshot. Rather than keying on the source
  line — which renames every snapshot when anything above it moves — the second
  call labels itself: `eval!(&mut state, as "later", at T0 + ..., ...)`.
- `at <timestamp>,` sets the clock for a batch. Left out, every test runs at
  2026-08-19, so snapshots do not change with the wall clock.

The `>` lines are documentation of intent rather than the assertion. The macro
could check them structurally, but that means parsing patterns and the snapshot
already covers it — say the word if you want it stricter.

### Derives

`Debug` only on events and actions. `Clone` stayed on the **view** types, and
that split looks right: a view value is data the UI owns (a Leptos signal stores
one, SwiftUI and Compose will each hold their own), while events and actions are
messages passed by reference all the way down. Written down in `docs/STYLE.md`.

### Crates and naming

- The app's machine moved out of `api-core` into **`phone-state`**. `api-*` is
  shared types; a state machine means a program, and a program owns its machine.
  `api-core` is gone.
- `srv-main` becomes **`server-main`**.
- `State*` / `state_*.rs`, `Runtime*` / `runtime_*.rs`, `View*` / `view_*.rs`,
  `Wire*` / `wire_*.rs`. One event or action per file.
- State machine fields are private everywhere.

### `api-wire`, and the one place the review was not followed exactly

You asked for the wire types to move into `server-main` and for `api-wire` to
stop existing. `api-wire` is gone, and `server-main` owns `wire.rs`.

But **both ends need them**, and `phone-state` cannot depend on `server-main`:
it is a `cdylib` linked into iOS and Android, and `server-main` pulls in axum,
tokio and reqwest. So each side owns its half — `server-main` serialises,
`phone-state` deserialises. They are separate types.

That is a real duplication and it can drift. What catches it:
`crates/phone-state/tests/snapshot.json` is generated by the server
(`cargo run -p server-main --example snapshot`) and parsed by the app's tests, so
a field renamed on one side fails the other side's suite. Worth a look — if you
would rather have a shared `api-wire` back after all, the argument for it is
exactly this drift, and the argument against is the dependency direction.

### Rate limiting

`api-ratelimit` holds the trait and a sliding `Window`; `src-plaid` holds
`PlaidRateLimiter`, keyed per endpoint per Item, which is how Plaid actually
meters. `StateServer` owns one and consults it in `should_fetch`.

Sliding rather than fixed: a fixed window lets twice the limit through across a
boundary, which is exactly when a burst happens. The trait takes `now` as an
argument, so it is IO-free and the machine can use it. There is a test asserting
a burst of 12 forced refreshes produces exactly 10 fetches.

The limits are set a little under Plaid's published numbers — being throttled
costs a user-visible failure and the headroom costs nothing at this scale.

### `FromTerminal`

Added, and it works: SIGINT becomes `EventTerminal::ShutdownRequested`, the
machine decides, and `ActionProcess::Shutdown` is what actually stops the
server. Verified by sending a real SIGINT — it drains and exits with
`INFO app shutting down` / `INFO app server stopped`. Shutdown is idempotent, so
a second ^C while draining does nothing, and `should_fetch` refuses new work
once it is set.

### Interior mutability

`StateServer` sits behind an `RwLock` rather than a `Mutex` now. Reads are the
common case — a request that finds fresh data only projects a snapshot — and
they proceed concurrently; writes are the fetch path, which is rare.

`handle_events` was **not** changed to take `&self`. Doing that means moving the
mutability inside the machine — a `RefCell` or per-field locks — and that trades
one contended lock for a machine that can no longer be reasoned about as a plain
`&mut self` function. Since the fetch is what actually blocks and there is
already a separate `fetching` mutex making that one-at-a-time, the `RwLock` gets
most of the win at none of that cost. **If you still want `&self`, say so and it
is a small change** — this is the one place the review was not followed and it is
worth a conversation.

### axum routing macros

There is no `#[get("/path")]` in axum. It ships `debug_handler`,
`debug_middleware`, `FromRequest` and `TypedPath`, and that is all — routing is
always a `Router`. Third-party crates exist but none is close to standard, and
`RuntimeServer::run` has two routes in it, so it was left alone.

### JSON fixtures

All committed JSON is 4-space indented now, and it is written down in the style
guide. `examples/snapshot.rs` writes it that way too, rather than needing a
reformat after every regeneration.

### Logs

`crates/src-plaid/logs/meridian.log` was tracked — `/logs` in `.gitignore` only
matched the root. Untracked, and the pattern is `logs/` + `*.log` now.

**No history rewrite.** The file was 3 lines from one failed Sandbox call, and
the only thing resembling a credential in it is the *phrase* `client_id` inside
a Plaid error message ("client_id must be a properly formatted, non-empty
string"). Nothing secret ever landed in the tree. Say the word if you would like
it purged anyway.

## Still open

**FFI bindings.** You asked whether they need committing. They are fully
generated — 8,100 lines from `./scripts/bindings.sh`. `docs/FFI.md` argues for
committing them, and the argument holds: nothing forces regeneration, no lint
can see stale bindings, and a dirty `git status` after running the script is the
only check that exists. The cost is real (that 2,800-line diff in an earlier
commit reviews as garbage). **CI regenerating and failing on a diff would close
it properly**, at which point they could be gitignored. Left committed for now.

**One big fetch, and all logic in the machine.** You said this is fine for now
and worth revisiting. Noted, not changed. When it does get revisited, the thing
to look at first is `ActionUi::Render` carrying the whole screen — that is what
forces every string through the machine, and relaxing it is how a little state
gets to live on the Swift/Kotlin side.

## What the app does

All four screens the sketch draws, against real Plaid Sandbox data: Home, Ledger
(48 transactions across 30 days and 4 months), Recurring (8 streams), and
Balances (14 accounts split into held and owed). Dock navigation, tap-to-reveal
masking, the category filter with live counts, three of the four time-range
forms, pull-to-refresh, and the empty and error states.

Not built, all deliberate and all visible-but-disabled rather than missing:
search (you asked for everything except), the `Between` date picker (a screen
the sketch does not draw), and Reconnect (needs Plaid Link update mode, which
does not exist yet).

## Where things are

```
crates/
  api-common/     types, incl. the Timestamp newtype
  api-ledger/     Entry, Category, Account, Connection, Stream, Ledger
  api-eval/       the eval! macro
  api-ratelimit/  the RateLimiter trait and a sliding window
  phone-state/    the app's state machine: events, actions, views, screens
  phone-main/     a Leptos runtime over it — the runnable specification
  server-main/    axum + Plaid Sandbox: its own machine, runtime, and wire types
  src-plaid/      Plaid's types as Plaid reports them, plus its rate limits
```

Tests: 51 over the app's machine, 24 over the server's, plus the crates' own.
All against the real captured Sandbox feed in `crates/server-main/fixtures/`,
which doubles as the offline fallback. `cargo clippy --workspace --all-targets`
is silent.

`./scripts/kotlin-smoke.sh` compiles the regenerated Kotlin against the real
library and drives all five screens through the FFI boundary. It passes.
