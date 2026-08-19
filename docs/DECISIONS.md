# Decisions

Everything I chose while turning `sketches/mobile/transactions.html` into a
running app, and why. Written for you to disagree with over coffee — where a
call is close, I have said so and said what the alternative costs.

Branch: `phone-app`. Four commits, each one runnable.

## Run it

```sh
cargo run -p srv-main                  # terminal 1 — talks to Plaid Sandbox
cd crates/phone-main && trunk serve    # terminal 2 — the app, on :8081
```

The server answers on `:8082`. With no `.env` it serves the captured Sandbox
fixtures instead of calling out, so the app works from a clean checkout — but
your `.env` is filled in, so you will get live Sandbox data.

Widen the browser window past 460×880 and the app draws itself in a phone-shaped
frame; narrower than that it fills the viewport, which is what a real phone
does.

**One thing I changed in your `.env`:** the `PLAID_ACCESS_TOKEN` that was in
there had gone stale — Plaid answered `ITEM_LOGIN_REQUIRED` for it. I minted a
fresh Sandbox Item with `/sandbox/public_token/create` + `/item/public_token/exchange`
and swapped the token in. Sandbox only, so no Production Item was consumed.
`.env` is gitignored, so this is not in the diff.

## What works

All four screens the sketch draws, against real Plaid Sandbox data:

| | |
| --- | --- |
| **Home** | the highlight of each tab, tapping through to it |
| **Ledger** | 48 transactions grouped into 30 days across 4 months |
| **Recurring** | 8 streams, the monthly commitment, the breakdown bar |
| **Balances** | 14 accounts, held and owed split into their own blocks |

Working interactions: dock navigation, tap-to-reveal masking, the category
filter with live counts, three of the four time-range forms, pull-to-refresh,
and the empty and error states.

Every figure matches the sketch exactly — $86,541.74 held, −$163,705.89 owed,
−$3,771.40/mo recurring — because the sketch was drawn from this same Sandbox
feed and the arithmetic now agrees with it.

## The big calls

### 1. `srv-main` is an axum server, and it has its own state machine

You said actix was fine and asked for Rust; I went with **axum** because the
workspace is already on `reqwest`, `tokio` and `http`, and axum sits on that
same tower/hyper stack. It adds far less to the tree than actix would. Both are
good; this one was just cheaper here.

You also asked for the events/actions model on the server, and it earned its
keep immediately. `srv-main/src/core.rs` holds `ServerState` with the same
shape as `api-core`: events in, actions out, no IO. The rules it owns are
"should I go back to Plaid?" and "what do I do with an `ITEM_LOGIN_REQUIRED`?",
and both are now pure functions of state and a clock handed in — so the
30-second freshness window is testable without a stopwatch, and the
login-required case is provably not retried (Plaid counts every call).

### 2. The server answers with one document, not four endpoints

`GET /snapshot` returns transactions, accounts, streams and the server's clock
in one JSON body. The alternative — an endpoint per screen — would have put
"which of my four fetches has landed?" state into the client, and that is the
exact state the architecture doc asks us to remove.

It also makes tab switching free: there is nothing to fetch when you tap
Ledger, because the data is already there. That is most of why the dock feels
instant.

### 3. `api-core` decides everything, including every string

This is the part I would most want you to look at. `api_core::view` holds the
finished screens: a `TxRow` carries `"−$5.40"` and a `DaySection` carries
`"Wed, Aug 12"`. No timestamps, no `f64`s, no category enums.

It makes the type surface bigger, which `docs/FFI.md` already predicted and
accepted. What it buys is that **the Swift and Kotlin views cannot disagree
with each other**, because neither is given anything to decide. The rewritten
`kotlin/src/test/kotlin/Smoke.kt` is the proof: it drives all five screens and
does not contain a single `format!`-equivalent.

The rule I worked to, and would suggest keeping: *if a view layer would need an
`if`, a `switch` or a format call to use a field, the field is wrong.*

### 4. `UiAction::Render` hands over the whole screen

Not `ShowTxs`, `SelectTab`, `OpenFilter`. One action carrying everything.

A view that applies a fine-grained diff has to hold the state the diff applies
to — and that is precisely the state we are trying not to have above the
boundary. Sending the whole screen means the view is a pure function of the
last action it saw. The phone app has **one signal** in it as a result.

The cost is rebuilding the view value on every event. For 48 transactions that
is nothing. If a ledger of 50,000 rows ever makes it matter, the fix is
windowing inside the core (send the visible slice), not a diff protocol.

### 5. The clock is an event field, never a call

Nothing in `api-core` reads a clock. `"in 3 days"` is a function of two
timestamps, one of which arrived in `UiEvent::Started { now }` or in the
snapshot. This is what makes a replay of an event log land where it did the
first time — the whole point of the design.

Related: **the snapshot carries the server's clock, and it wins over the
phone's.** A device with a wrong date would otherwise render a confidently
wrong "in 3 days", and phones do have wrong dates.

### 6. The mask is one switch, and covering happens in the core

Figures start **covered** — that is why `State` writes its own `Default` rather
than deriving one, which would have silently started the app revealed.

The core substitutes the dots before the value crosses the boundary, so a
covered figure is not in the DOM at all. The sketch kept the real value
underneath at zero opacity to stop the layout shifting; I preferred not having
the number in the page, and matched the layout with a `min-width` instead.

Revealing is app-wide rather than per-screen. Revealing on Balances and staying
covered on Home would be a strange thing to have to explain.

Ledger rows are never covered. The sketch made this call and it is right: a
list of grey bars says nothing, and the figure worth hiding from someone
glancing over is the total, not a $4.33 coffee.

### 7. The ledger grew, rather than being bypassed

`docs/ABOUT.md` argues hard for one ledger rather than two models. A Plaid
transaction has a merchant, a category and a pending flag, and `Tx` has nowhere
to put any of them.

I added `api_ledger::Entry` — a `Tx` plus what is known about it — rather than
letting the phone read Plaid's types directly. `Tx` stays the arithmetic;
`Entry` is the row. Accounts and recurring streams went into `api-ledger` too,
beside the entries, because the screens that show them read the same feed.

### 8. Category is an enum, not a string

The app groups, filters and colours by category everywhere. A typo in a string
would have been a silently empty filter rather than a compile error.
`Category::from_plaid` maps all sixteen of Plaid's primaries and degrades
anything unknown to `Unknown` — one bad row rather than a failed sync.

The colour token (`"c-food"`) is decided in `api-ledger` and crosses the
boundary as a string. Swift and Kotlin map that token to their own palettes;
they never derive a colour from a category name.

## The two Plaid traps

`docs/PLAID.md` names both, and both are handled once and pinned by tests
against the real captured Sandbox responses.

**Sign.** Flipped in `srv-main::project::entry`, and nowhere else. A second
negation downstream would cancel this one and turn every expense into income —
which is why there is a test asserting the Uber ride comes out negative *and*
one asserting the United Airlines refund comes out positive.

**Pending duplicates.** `Entry::supersedes` carries Plaid's
`pending_transaction_id`, and `Ledger::apply` drops the superseded row. The
Sandbox feed has no pending transactions in it, so the test builds the case by
hand — worth knowing, because it means this path is proven by construction
rather than by observation.

### A third one I hit, which the docs do not mention

**`/transactions/sync` is a diff, and the second call returns nothing.**

The first sync returned all 48 transactions; the second, with the saved cursor,
correctly returned three empty lists — and the server was *assigning* that over
the ledger instead of applying it. The app went blank on refresh while every
log line still said the sync succeeded.

I only found this by driving the real app in a browser; no unit test I had
written would have caught it, because each one started from a fresh state. It
is now `ServerState::ledger` with `Ledger::apply`, and two tests named after
the failure. **Worth adding to `docs/PLAID.md` as a third way to be quietly
wrong** — I have left that to you since it is your document's voice.

## What I did not build, and why

**Search.** You said everything except search, and the sketch draws the dock
button but never the screen behind it. The button is present and disabled, with
a tooltip saying so. Inventing a search screen would have meant inventing a
design you had not made.

**The `Between` date range.** The other three range forms work. This one needs
a date picker, which is a screen of its own that the sketch does not draw. The
row is rendered and dimmed so the sentence still reads as the sketch has it.

**Reconnect.** The button is drawn and disabled. Clearing `ITEM_LOGIN_REQUIRED`
takes Plaid Link in update mode — a hosted page plus a server round trip that
do not exist yet. A live-looking button that did nothing would be worse than
one that says what it is waiting for. The Sandbox Item is healthy right now, so
you will not see this state unless you break it deliberately.

**A transaction detail screen.** `UiEvent::TxSelected` is dispatched on every
row tap and the core ignores it. The event exists so the view can already send
it and so the log records the taps — when a detail screen arrives, nothing in
the view layer changes.

**Three connections.** The sketch shows First Platypus, Houndstooth and Tartan.
One Sandbox Item gives all 14 accounts under First Platypus, so the app shows
one connection with everything under it. Multi-Item is a server change (a list
of access tokens rather than one), not an app change — every screen already
renders a `Vec<Connection>`.

## Things you might want to change

- **`FRESH_FOR_SECS = 30`** in `srv-main/src/core.rs`. Plaid rate-limits
  `/accounts/balance/get` hard, and a phone refetching per page load would hit
  it. Thirty seconds felt right for development; it is one constant.
- **The server URL is a constant** in `phone-main/src/io.rs`. Fine for a sketch
  crate that runs beside the server; the native apps will need real config.
- **CORS is wide open** (`allow_origin(Any)`). It is a localhost dev server, but
  it should name an origin before it is ever deployed.
- **`Stream::monthly_amount`** normalises weekly to 52/12 rather than 4. That is
  the arithmetic-correct choice and it makes the total differ slightly from a
  naive one. The Sandbox feed is all monthly, so it does not bite here.
- **Home previews the category as plain text**, not a chip. At preview scale a
  column of pills read as noise. The sketch has text there too, so I think this
  is right, but it is a look-at-it call.

## Unrelated fix

`web-main`'s `windows_end_now_and_reach_back` test was failing on `master`
before I started — it asserted a window ends at `now`, but `window()`
deliberately ends at the last *closed* bar, and `NOW` in the test is not
minute-aligned. The test was wrong, not the code. Renamed and corrected, so
`cargo test --workspace` is green.

## Where things are

```
crates/
  api-ledger/   Entry, Category, Account, Connection, Stream, Ledger
  api-core/     the state machine: events, actions, the five screens' logic
  api-wire/     the JSON both ends share
  srv-main/     axum + Plaid Sandbox, with its own events/actions core
  phone-main/   Leptos: runtime, four screens, one signal
```

Tests: 51 over the state machine (`api-core/tests/machine.rs`), 20 in
`srv-main`, 26 in `api-ledger`. All against the real captured Sandbox feed in
`crates/srv-main/fixtures/`, which doubles as the offline fallback.

`./scripts/kotlin-smoke.sh` compiles the regenerated Kotlin bindings against the
real library and drives all five screens through the FFI boundary. It passes.
That is the best evidence that the Swift and Kotlin work will be transcription
rather than reimplementation.

## One thing I left for you

Per `CLAUDE.md` I should tick off the work in the GitHub issues, and this
finishes most of **#10 (Route all app behaviour through an event/action state
machine)**. I have not posted to the tracker — that is outward-facing and you
were not around to okay it — so here is the comment, ready to paste:

> The phone app now runs entirely on this. `api-core` holds all four screens'
> logic — filtering, grouping, totals, masking, and every string the screens
> print — and `phone-main` renders what it is handed.
>
> Done:
> - [x] Events and actions as enums, grouped by origin
> - [x] A synchronous machine, no clock and no IO
> - [x] A runtime that performs actions and feeds results back as events
> - [x] The same shape on the server (`srv-main/src/core.rs`)
> - [x] 51 tests, including one asserting the same events give the same actions
>
> Two calls worth knowing, both in `docs/DECISIONS.md`:
> - `UiAction::Render` sends the whole screen, not a diff, so the view holds no
>   state to apply a diff against. The phone app has one signal in it.
> - The clock arrives in an event rather than being read, so a replay lands
>   where it did the first time.
>
> Not done: the assistant as an event source, and event-log recording/replay as
> an actual tool rather than a property the design has.

**#9 (implement search)** is untouched and is the natural next screen, since
the dock button is already sitting there disabled.
