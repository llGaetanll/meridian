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
place. A test is a slice of events in, a `Vec<Action>` and a `State` to assert
on out — no runtime, no await, no mocks.

### Where it lives

Both enums, and the machine itself, want to be platform-agnostic: the same
decisions run on the desktop, the web and the phone. That points at an `api-*`
crate rather than `web-state`. `api-common` is meant to stay a
dependency-free bag of value types, so the machine belongs next to it in its
own crate — `api-core` — depending on `api-common` and on nothing else.

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
| `api-common` | Shared types: `Asset`, `Price`, `Quantity`, `Timestamp`, `ChartData`, and formatting helpers. No dependencies. |
| `api-core` | Planned. Events, actions, and the state machine that turns one into the other. Synchronous, no IO. |
| `api-ledger` | Transactions, positions, the ledger itself. |
| `src-yahoo`, `src-coingecko` | Price sources. Requests, responses, and pure projections onto `api-common` types. |
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
