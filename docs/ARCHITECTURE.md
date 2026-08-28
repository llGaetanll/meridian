# Architecture

Meridian ships as a desktop app (Tauri), a web app, and a phone app. The same
crates back all three, so the split between them is what keeps that possible.

## Keep state and IO apart

This is the rule everything else follows from. Three layers, and each one only
ever knows about the one below it:

- **IO** — `src-yahoo`, `src-coingecko`, and the `io` module of a front-end.
  Plain async functions over plain data: fetch, parse, map onto the app's own
  types. No signals, no resources, no components. Anything here can be called
  from a test, a background job, or another platform's shell.
- **State** — the `state` module of a front-end (and `web-state`). Signals and
  resources, and the rules for when to refetch. It calls into IO; it never
  builds a request itself.
- **View** — `web-components` and `web-ui`. Components take plain values or
  signals as props and render them. They do not fetch, and they do not reach
  for global state; a page wires state into them.

The point is that a component can be dropped into a different page, an IO
function can be run from a CLI, and a state object can be tested with a stub —
because none of them are entangled with the other two.

## Events in, actions out

The layering above says where code lives. This says how it talks. It is the
direction we are moving in, not what the code does everywhere today.

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

These are settled. A new state machine — and there will be more, one per
program — follows all of them.

1. **`handle_events` is the only entry point, and the only way state changes.**
   The fields are private. A `pub` field is a way to mutate state without an
   event, which is the one thing this design exists to prevent.

2. **Time is an argument, not a call.** `handle_events(now, events, actions)`.
   The clock is read once per batch, at the edge, by the runtime. Nothing
   inside reads one, which is what makes a replay land where it did the first
   time. The same goes for randomness and IO.

3. **Nothing derived is stored.** Filtering, grouping and totals are computed
   on the way out. A field that could be recomputed is a field that can
   disagree with itself.

4. **Derive as little as possible on events and actions.** `Debug` for logging
   and for failure messages; `Clone` only where something must own one. Not
   `PartialEq` — tests match on patterns via `eval!`, and that derive spreads a
   bound through every type it touches.

5. **No panics.** `debug_assert!` for invariants, `unreachable!` where a case
   is provably impossible, `Result` for anything depending on input or IO. See
   [STYLE.md](STYLE.md).

6. **Split by ownership, not by screen.** A sub-machine (`StateLedger`) is
   worth it when a screen owns state nothing else reads. Shared data stays on
   the parent — two copies of the ledger are two copies that can disagree — and
   sub-machines never handle events themselves; the parent routes to them, so
   `handle_events` remains the only way anything changes.

7. **Anything that is a *rule* belongs in the machine, even if it looks like
   IO's business.** Rate limiting is the example: "may I send this?" is
   arithmetic over what we have already sent, so it lives in the machine
   (`api-ratelimit`, with the per-API limiter in that API's crate) rather than
   being guessed at by the runtime.

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

Ordering five writes so the cursor lands last is *how* — it is the mechanics of
one action, and the machine neither knows nor cares. Checking whether a
database was configured and, if not, emitting a different event, is *whether*
and *what*: it is behaviour, it lives outside the machine, no `eval!` test can
reach it and no event log records it happening.

The fix is always the same, and it is worth recognising the shape: **the
runtime reports a fact, the machine decides what to do about it.** Whether
there is a store, whether credentials exist, what a source answered — those are
facts, and they arrive as events. Everything downstream of a fact is a
decision, and decisions live in one place.

The symptom to watch for is a runtime reading its own state in order to pick an
event to send. `if self.db.is_some()` is the tell. Nothing about that is
visible in a log, and nothing about it can be replayed.

`server-main` writes this out as a real loop over requests;
`crates/phone-main/src/runtime_app.rs` is the same thing driven by user
interaction rather than by a `loop`, because a browser app is woken rather than
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
shape with the boilerplate removed:

```rust
eval!(&mut state,
    < EventClient::SnapshotRequested { force: false },
    > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None } },
);
```

`<` is an event in, `>` is an action out, and adjacent events form one batch.
The `>` lines are patterns, so the macro expands to `matches!` and the types
need no `PartialEq`. See [STYLE.md](STYLE.md).

### Where it lives

A state machine is a *program's* logic, not a shared type, so it lives with the
program: `phone-state` for the app, `server-main` for the server. The `api-*`
crates stay what they are — types and pure functions with no program in them.

The split falls differently for the two because their IO does:

- The **server** owns its machine and its runtime in one crate, because it owns
  both halves.
- The **phone** keeps its machine in `phone-state` alone, because the runtime
  is Swift and Kotlin. `phone-main` is a third runtime — a Leptos one — that
  exists so the design can be run and judged without a Mac.

### What this looks like once it is built

`crates/phone-main` is the worked example. Two of its properties are worth
stating here, because they are what the design actually buys:

- **The whole screen crosses, not a diff.** `UiAction::Render` carries the
  finished screen, so the view is a pure function of the last action it saw and
  holds no state to apply a diff against.
- **The clock is an event field.** Nothing in a machine reads one, so `"in 3
  days"` is a function of two timestamps and a replay lands where it did the
  first time.

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
| `phone-state` | The phone app's state machine: events, actions, every screen's logic and every string it prints. Synchronous, no IO. Reached from Swift and Kotlin over UniFFI — see [FFI.md](FFI.md). |
| `src-yahoo`, `src-coingecko` | Price sources. Requests, responses, and pure projections onto `api-common` types. |
| `src-plaid` | The bank connection, same shape as the price sources: sync, balances, recurring, and Plaid's rate limits. |
| `server-wire` | The JSON the server and the app share. One definition; its dependencies are only the value types the protocol carries. |
| `server-main` | The server. Holds the Plaid secret, which is why it exists; owns its machine, its runtime, and its wire types. Sandbox only. |
| `web-ui` | Design-system pieces: buttons, pills, cards, segmented controls, icons. No domain knowledge. |
| `web-components` | Domain components: the shell, charts, and the markets page's blocks. |
| `web-state` | State shared across pages. |
| `web-main` | The web front-end: `io`, `state`, `pages`, and the styles. |
| `phone-main` | A Leptos runtime over `phone-state`. Renders what the machine decides and holds one signal; a runnable specification for the Swift and Kotlin views. |

## Talking to price sources

Requests carry a path, not a URL, and the client resolves it against a base
(see `YahooClient::with_base_url`). Native builds point that base at the source
directly. Browsers cannot: Yahoo sends no CORS headers, and the `User-Agent` it
insists on cannot be set from a page. So in the browser the base is the app's
own origin and something in front forwards the call — in development that is
the Trunk proxy configured in `crates/web-main/Trunk.toml`.
