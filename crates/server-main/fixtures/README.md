# Fixtures

Real Plaid **Sandbox** responses, captured 2026-08-19 from institution
`ins_109508` (First Platypus Bank) with `initial_products: ["transactions"]`.

| File | Endpoint |
| --- | --- |
| `sync.json` | `/transactions/sync` |
| `balances.json` | `/accounts/get` |
| `recurring.json` | `/transactions/recurring/get` |

Captured with a throwaway Sandbox Item, so there are no real credentials and
no account details in here — Sandbox has neither. `request_id` was stripped;
nothing else was edited.

Two jobs:

- **The offline fallback.** `server-main` serves these when it has no Plaid
  credentials, so the phone app runs from a clean checkout with no `.env`.
- **The test corpus.** The parsing and projection tests read these rather than
  hand-written JSON, so what the tests prove is that we handle *Plaid's* shapes
  and not our idea of them.

They are formatted with 4-space indentation, per [STYLE.md](../../../docs/STYLE.md):
they exist to be read.

To refresh them, capture the three endpoints again against Sandbox. Sandbox only —
see [docs/PLAID.md](../../../docs/PLAID.md).
