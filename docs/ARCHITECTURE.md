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

## Crates

| Crate | What it is |
| --- | --- |
| `api-common` | Shared types: `Asset`, `Price`, `Quantity`, `Timestamp`, `ChartData`, and formatting helpers. No dependencies. |
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
the Trunk proxy configured in `src/web-main/Trunk.toml`.
