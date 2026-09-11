# Architecture

Meridian ships as a desktop app (Tauri), a web app, and a phone app. The same
crates back all three, so the split between them is what keeps that possible.

## Keep state and IO apart

This is the rule everything else follows from. Three layers, and each one only
ever knows about the one below it:

- **IO** — the `src-*` crates and the `io` module of a front-end. Plain async
  functions over plain data: fetch, parse, map onto the app's own types. No
  signals, no resources, no components. Anything here can be called from a
  test, a background job, or another platform's shell.
- **State** — the `state` module of a front-end (and `web-state`). Signals and
  resources, and the rules for when to refetch. It calls into IO; it never
  builds a request itself.
- **View** — `web-components` and `web-ui`. Components take plain values or
  signals as props and render them. They do not fetch, and they do not reach
  for global state; a page wires state into them.

The point is that a component can be dropped into a different page, an IO
function can be run from a CLI, and a state object can be tested with a stub —
because none of them are entangled with the other two.

### The IO layer is thin

**A method on an `src-*` client makes at most one request.** It matches an
endpoint the remote API actually has, and its name says which one.
`/transactions/sync` is a method; "sync, then fetch the balances" is not. A
client may hold a base URL, credentials and a shared `reqwest` client, and that
is the whole of what it owns.

Two calls in a row are a decision — what the first answer means, what to do
when it fails, whether the second is still wanted — so they go through the
state machine as two actions and two events. A helper doing both would put that
decision where no `eval!` test can reach it and no event log records it.

## Events in, actions out

The layering above says where code lives. This says how it talks.

Nothing that decides anything is allowed to do IO. Instead:

1. Something happens — the user clicks, a fetch comes back, a timer fires.
   That is an **event**: a plain Rust enum value, nothing more.
2. The event goes into the **state machine**, a struct holding all of the app's
   state and a synchronous method that updates it. It reads no clock, opens no
   socket, awaits nothing.
3. The **actions** it hands back are the IO: fetch this range, show that error,
   write this row. A thin runtime carries them out, and whatever comes back
   enters as the next event.

So the loop is `event -> state machine -> action -> IO -> event`, and the only
part with a decision in it is the one part that cannot touch the outside world.

### Why

One day this app is in the hands of people we cannot sit next to. One of them
will hit a bug, and often we will get exactly one shot at it: a log, no repro,
no second run. That is the whole reason [logging](LOGGING.md) is as careful as
it is, and this is the other half of it.

If every decision is a pure function of state and an event, then a log of the
events is a recording of the session. We replay it, get the same state and the
same actions, see the bug, fix it, and keep the event list as a test. A bug we
have seen once never comes back. If a decision can instead depend on a clock, a
network race, or the order two futures happened to resolve in, none of that
holds, and we are guessing from a log.

### The shape of the types

Events and actions are the vocabulary the whole app shares, so both name where
they came from and where they are going:

```rust
enum Event {
    FromUi(UiEvent),        // a click, a range picked, a symbol typed
    FromSource(SourceEvent) // a quote arrived, a request failed
}

enum Action {
    ToUi(UiAction),         // a toast, a navigation
    ToSource(SourceAction)  // fetch these bars
}
```

A `FromCli` arm and the rest follow the same pattern if we need them. Grouping
by origin keeps a match on `Event` readable as the app grows, and makes the
recorded log read as a story rather than a flat list.

### The rules a machine follows

These are settled. A new state machine follows all of them.

1. **`handle_events` is the only entry point, and the only way state changes.**
   The fields are private. A `pub` field is a way to mutate state without an
   event, which is the one thing this design exists to prevent.

2. **Time is an event, not a call.** `EventProcess::Tick { now }` on the
   server and `EventRuntime::Tick { now }` on the phone, about once a second,
   read at the edge by the runtime. Nothing inside reads a clock, which is what
   makes a replay land where it did the first time.

   A tick is also the only event that arrives when *nothing* has happened, and
   that turns out to be load-bearing: a source that goes quiet rather than
   failing produces no event, so without a tick there is no turn in which to
   give up on it. `server-main` expires a call that has been out too long and
   answers the route waiting on it — see `CALL_GOOD_FOR_SECS`.

   The phone's tick has a second job: its countdown on the code screen is the
   one thing in the app that changes while nobody is touching it. The machine
   decides which ticks are worth drawing for — today only that screen's — so
   the runtime sends them unconditionally and holds no rule of its own.

   **The clock is the device's, and only the device's.** A snapshot carries the
   timestamps *of* things — when a sync landed, when a stream is next due — and
   never a reading of its own. The phone has a clock, it is right, and a phone
   that had to be told the time would draw nothing until the first fetch
   answered and would stop moving between fetches. `synced_at` on a connection
   is data; `now` was a second clock, and there is one clock.

   **Randomness arrives the same way, as a seed.** `server-main` takes 32 bytes
   of OS entropy on `EventProcess::Seeded` and draws sign-in codes and session
   tokens from a ChaCha20 generator seeded with it. So generation is a decision
   like any other — reproducible from the log, reachable from `eval!` — while
   staying unpredictable to anyone without the seed. What this asks for is that
   the generator be cryptographic; the seed itself is in place before anything
   asks for a credential, because seeding happens in `Starting` (rule 3).

   It also buys the bigger prize: with the generator inside, a whole run is
   reproducible from one seed, which is what makes deterministic simulation
   testing possible later.

3. **A machine's life is `Stopped`, `Starting`, `Started`, `Stopping`.**
   Everything the machine needs in order to work — its seed, its
   configuration, whatever a source had to be asked for first — is gathered in
   `Starting`, and `Started` holds it outright. So nothing in `Started` checks
   whether setup happened: the phase is the answer. A machine asking "have I
   been seeded yet?" on a turn has spread its startup across its whole life,
   and every branch of that question is a state nobody tested.

4. **Nothing derived is stored.** Filtering, grouping and totals are computed
   on the way out. A field that could be recomputed is a field that can
   disagree with itself.

5. **Every field is deterministic.** A `HashMap` iterates in an order that
   changes from run to run, so a machine holding one can produce the same
   actions in a different order from the same events — and a replay that
   lands somewhere else is not a replay. `BTreeMap` where a table is keyed,
   `Vec` where it is ordered.

6. **Derive as little as possible on events and actions.** `Debug` for logging
   and for failure messages; `Clone` only where something must own one. Not
   `PartialEq` — tests match on patterns via `eval!`, and that derive spreads a
   bound through every type it touches.

7. **No panics.** `debug_assert!` for invariants, `unreachable!` where a case
   is provably impossible, `Result` for anything depending on input or IO. See
   [STYLE.md](STYLE.md).

8. **Split a machine by process.** A sub-machine (`StateSignup`) owns a
   process end to end: its state, its rules, and the events that drive it. The
   parent's `handle_events` still routes — it is the only way anything changes
   — but what a signup event means is decided in the signup machine, not in
   the parent with the signup's fields spread across it.

   Shared data stays on the parent: two copies of the ledger are two copies
   that can disagree.

9. **Anything that is a *rule* belongs in the machine, even if it looks like
   IO's business.** Rate limiting is the example, in both directions. "May I
   send this?" is arithmetic over what we have already sent, and "may they ask
   this?" is arithmetic over what we have already answered; neither needs a
   clock or a socket, so both live in the machine (`api_common::ratelimit`,
   with each limiter beside the limits it knows — `src-plaid` for Plaid's,
   `server-main` for our own) rather than being guessed at by the runtime.

   What that buys is that a limit is an `eval!` test. "The twenty-first request
   in a minute is refused, and starts no fetch" is a sequence of events and the
   actions they do not produce, which is not a thing you can write about a
   limiter living in a middleware.

10. **A process with more than one step is one entry in one table, and the
    entry's variant is the step.** A route in flight is a `Pending`, keyed by
    its request: parked on the session read, parked on a Link session, decided
    and waiting for its writes to land. A Link session in its race is a
    `LinkSession`, keyed by its token: reported and not yet asked about, or
    asked about and not yet reported. The parent holds the table and the id;
    which step an entry is at is the entry's own business.

    What this buys is that the table *is* the list of what is in flight. An
    entry that is still there after its answer went out is a leak by
    definition, which is what `census()` counts and `state_leaks.rs` asserts.
    Three tables keyed three ways — a call, a request, a token — were three
    places one route could be forgotten in.

    The one table keyed by something other than the process it describes is
    `Waiting`, by call: an answer names the call it is for and not the route,
    so that is the index from one to the other, and the list the tick gives up
    on.

### Waiting without sleeping

A machine often needs to wait: back off before retrying, give up on a call that
has been out too long, do not ask Plaid again yet. That is a rule, so by rule 9
it belongs in the machine — but sleeping is IO, and the machine cannot do IO.

It does not need to. It holds the time the last tick brought, so every waiting
rule is arithmetic it can do on the spot:

```rust
.filter(|(_, waiter)| now.since(waiter.since).as_secs() >= CALL_GOOD_FOR_SECS)
```

So "wait half a minute, then answer the route anyway" is never a sleep
anywhere. It is *this call is older than thirty seconds* — asked again on every
tick, and answered the same way every time it is replayed.

**A second is close enough.** The machine's clock is a rounded copy of the real
one, and that is deliberate: a rule needing finer resolution than the tick is a
rule about to depend on scheduling luck.

### The runtime on the other side

Every program is two pieces: a machine that decides and a runtime that acts.
The runtime mirrors the machine's shape, so an action and the thing carrying it
out sit at the same depth:

```rust
impl RuntimeExample {
    pub fn run() { … }                                   // the only public fn

    fn perform_actions(&self, actions: &[Action]) { … }
    fn perform_action(&self, action: &Action) { … }      // dispatch only
    fn perform_action_source(&self, action: &ActionSource) { … }
}
```

One turn is: collect events, hand them to the machine, carry out the actions,
log both. Whatever the IO produces re-enters as the next event, which is what
closes the loop. Only `run` is public — everything else is reachable solely by
carrying out an action, so nothing can do IO without an action asking for it.

### The line a runtime does not cross

**A runtime chooses *how* to carry out an action. It never chooses whether to,
or what to report back.**

Ordering five writes so the cursor lands last is *how* — the mechanics of one
action, which the machine neither knows nor cares about. Checking whether a
database was configured and, if not, emitting a different event is *whether*:
behaviour that no `eval!` test can reach and no event log records.

The fix has one shape: **the runtime reports a fact, the machine decides what
to do about it.** Whether there is a store, whether credentials exist, what a
source answered — facts, arriving as events. The tell that it has gone wrong is
a runtime reading its own state to pick an event to send: `if self.db.is_some()`.

`server-main` writes this out as a real loop over requests;
`swift/Sources/MeridianApp/RuntimeApp.swift` is the same thing driven by user
interaction rather than by a `loop`, because a phone app is woken rather than
polling.

### The shape of the machine

The machine owns its state and mutates it in place. Actions are not returned;
they are pushed into a buffer the caller owns, so a batch of events costs one
allocation and a replay can reuse the same one:

```rust
impl State {
    pub fn handle_events(&mut self, events: &[Event], actions: &mut Vec<Action>) {
        for event in events {
            self.handle_event(event, actions);
        }
    }

    fn handle_event(&mut self, event: &Event, actions: &mut Vec<Action>) {
        match event {
            Event::FromUi(e) => self.handle_event_ui(e, actions),
            Event::FromSource(e) => self.handle_event_source(e, actions),
        }
    }

    fn handle_event_ui(&mut self, event: &UiEvent, actions: &mut Vec<Action>) { … }
    fn handle_event_source(&mut self, event: &SourceEvent, actions: &mut Vec<Action>) { … }
}
```

`handle_events` is the only public entry point; everything under it is one
function per event group, mirroring the enums. `handle_event` itself does
nothing but dispatch, so the arms stay small and each group's rules sit in one
place. A test is a slice of events in, a `Vec<Action>` and a state to assert on
out — no runtime, no await, no mocks.

In practice a test is written with `eval!` (`crates/api-eval`), which is that
shape with the boilerplate removed. Its syntax and the rules for using it are
in [STYLE.md](STYLE.md).

### Where it lives

A state machine is a *program's* logic, not a shared type, so it lives with the
program: `client-state` for the apps, `server-main` for the server. The `api-*`
crates stay what they are — types and pure functions with no program in them.

The split falls differently for the two because their IO does:

- The **server** owns its machine and its runtime in one crate, because it owns
  both halves.
- The **clients** keep their machine in `client-state` alone, because each
  runs it differently: the phone's runtime is Swift and Kotlin, which go in no
  crate, and the web's is its own, in `web-main`.

### What this looks like once it is built

The phone is the worked example: `client-state` decides, and
`swift/Sources/MeridianApp` carries it out. Two of its properties are worth
stating here, because they are what the design actually buys:

- **The whole screen crosses, not a diff.** `UiAction::Render` carries the
  finished screen, so the view is a pure function of the last action it saw and
  holds no state to apply a diff against.
- **The clock is an event.** Nothing in a machine reads one, so `"in 3 days"`
  is a function of two timestamps — the tick that last arrived and the date on
  the thing — and a replay lands where it did the first time.

### Where Leptos fits

Today most of this goes through Leptos signals and resources directly: a click
sets a signal, a resource notices and fetches. Signals are not going away —
they are how the view stays up to date — but they move underneath the machine
rather than sitting in place of it. A component's handler dispatches an event;
the runtime writes the resulting state into signals; the view re-renders. The
signal graph becomes plumbing, not the place where behaviour is decided.

## Crates

| Crate | What it is |
| --- | --- |
| `api-common` | Shared types: `Asset`, `Price`, `Quantity`, `Timestamp`, `ChartData`, and formatting helpers. |
| `api-ledger` | Entries, positions, accounts, recurring streams, the ledger itself. |
| `api-eval` | The `eval!` macro every state machine is tested with. |
| `client-state` | The state machine the phone and the web both run: events, actions, every screen's logic and every string it prints. Synchronous, no IO. Reached from Swift and Kotlin over UniFFI — see [FFI.md](FFI.md) — and from the web directly. |
| `src-yahoo`, `src-coingecko` | Price sources. Requests, responses, and pure projections onto `api-common` types. |
| `src-plaid` | The bank connection, same shape as the price sources: sync, balances, recurring, and Plaid's rate limits. |
| `server-wire` | The JSON the server and the app share. One definition; its dependencies are only the value types the protocol carries. |
| `server-main` | The server. Holds the Plaid secret, which is why it exists; owns its machine, its runtime, and its wire types. Sandbox only. |
| `web-ui` | Design-system pieces: buttons, pills, cards, segmented controls, icons. No domain knowledge. |
| `web-components` | Domain components: the shell, charts, and the markets page's blocks. |
| `web-state` | State shared across pages. |
| `web-main` | The web front-end: `io`, `state`, `pages`, and the styles. |

## Talking to price sources

Requests carry a path, not a URL, and the client resolves it against a base
(see `YahooClient::with_base_url`). Native builds point that base at the source
directly. Browsers cannot: Yahoo sends no CORS headers, and the `User-Agent` it
insists on cannot be set from a page. So in the browser the base is the app's
own origin and something in front forwards the call — in development that is
the Trunk proxy configured in `crates/web-main/Trunk.toml`.
