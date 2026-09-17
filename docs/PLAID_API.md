# The Plaid API, and what each call costs

[PLAID.md](PLAID.md) is how our integration works. This is the other half:
what Plaid actually sells, how each piece is billed, and which of it we use.
It exists because the calls we make were chosen while getting connected, not
by looking at this list — #51 is what that looks like from the outside.

**Plaid publishes no rates.** Everything here is the billing *model* — which
is documented — plus third-party-reported ranges where noted. The actual
numbers for our account are in the dashboard, and reading them off is still an
open item in [BUSINESS.md](BUSINESS.md).

## The four billing shapes

Every product falls into one of these, and the shape matters more than the
rate, because it decides what a *user's behaviour* can cost us.

| Shape | Charged when | What it means |
| --- | --- | --- |
| Free | Never | Rate limits are the only constraint |
| One-time, per Item | The product first touches the Item | A connection cost, paid once |
| Subscription, per Item per month | While the access token exists | A holding cost. Billed **even if no calls are made**, and even if calls fail |
| Per request | Each successful call | The only shape where app usage is a bill |

## The catalog

### Free

| Endpoint | What it does |
| --- | --- |
| `/accounts/get` | Accounts and **cached** balances. On an Item with Transactions the cache refreshes about daily |
| `/item/get`, `/item/remove` | Item status and disposal |
| `/link/token/create`, `/link/token/get` | Opening and inspecting a Link session |
| `/item/public_token/exchange` | Turning a finished session into an access token |
| `/institutions/search`, `/institutions/get_by_id` | The bank directory |
| `/webhook_verification_key/get` | The key that signs webhooks |
| `/sandbox/*` | All of it — none exists in Production |

One footnote: Hosted Link itself is free, but asking Plaid to *deliver* the
link by SMS or email is billed per delivery. We hand the URL over ourselves,
so this does not touch us.

### One-time, per Item

| Product | Endpoints | What it is |
| --- | --- | --- |
| Auth | `/auth/get` | Account and routing numbers, for moving money |
| Identity | `/identity/get`, `/identity/match` | The name and address on the account. `/identity/match` bills per request on top |
| Income | `/credit/*` | Payroll and document income verification |
| Layer | — | Plaid's instant-onboarding product |

If Auth or Identity ride along as `optional_products` on the link token, the
charge waits until the endpoint is first called — so listing them costs
nothing until used.

### Subscription, per Item per month

| Product | Endpoints | Note |
| --- | --- | --- |
| Transactions | `/transactions/sync`, `/transactions/get` | The calls themselves are then unmetered — a sync costs nothing at the margin. `/transactions/refresh` is the exception, billed per request |
| Recurring Transactions | `/transactions/recurring/get` | **A separate subscription**, not bundled with Transactions. This answers a BUSINESS.md open question |
| Liabilities | `/liabilities/get` | Loan and card terms, APRs, due dates |
| Investments | `/investments/holdings/get`, `/investments/transactions/get` | Two separate subscriptions, one per endpoint family |

The subscription runs while the access token exists. An Item whose user
stopped opening the app costs the same as one syncing daily; the only way to
stop paying is `/item/remove` — which on our Trial plan also burns the slot
forever, so removal is not a cost lever for us yet.

### Per request

| Product | Endpoints | Note |
| --- | --- | --- |
| Balance | `/accounts/balance/get` | Real-time, goes to the bank. Billed per successful call. Reported at $0.05–0.15 |
| Signal | `/signal/evaluate` | ACH risk scoring. Bills even when data extraction partly fails, if a ruleset ran |
| Refreshes | `/transactions/refresh`, `/investments/refresh` | On-demand re-pulls outside the subscription cadence |
| Assets, Statements, Enrich | various | Flexible fees — priced by history days, statement count, or transaction count |

### Money movement

Payment Initiation, Transfer, and Investments Move are their own pricing
world (per payment, plus operational fees). Meridian moves no money, so they
are out of scope entirely.

## The plans

| Plan | Terms |
| --- | --- |
| Trial | Free, 10 Production Items ever, removal does not free a slot. **We are here** |
| Pay-as-you-go | No minimum, no commitment. The natural next step |
| Growth | Minimum spend, annual commitment, lower rates. For usage up to ~$6k/mo |
| Custom | Higher commitment, lowest rates, enterprise features |

## What Meridian calls, and what each costs

`PlaidEndpoint` (`plaid_ratelimit.rs`) is the complete list of what this app
can say to Plaid. Mapped onto the shapes above:

| We call | Shape | Marginal cost of one more call |
| --- | --- | --- |
| `/transactions/sync` | Subscription | **Zero** — the Item is paid for whether we sync or not |
| `/transactions/recurring/get` | Subscription (separate) | **Zero**, same reasoning |
| `/accounts/get` | Free | Zero — balances from Plaid's cache, refreshed about daily |
| `/link/token/create`, `/link/token/get` | Free | Zero |
| `/item/public_token/exchange` | Free | Zero, but creates a subscription and spends a Trial slot |
| `/webhook_verification_key/get` | Free | Zero |
| `/item/remove` | Free | Zero, but the slot does not come back |

And what we notably do *not* call: `/accounts/balance/get`, which asks the
bank live and bills per successful call. Plaid's own guidance is that it is
for the moment before moving money; a personal-finance display is what the
cached number from `/accounts/get` is for, and that is the one the sync path
uses.

## What this means

Two consequences fall straight out of the table:

- **A user's marginal cost is zero.** Every call the app makes is either free
  or already paid for by the subscription, so `FRESH_FOR_SECS` protects rate
  limits rather than a bill.
- **The cost that actually scales with users is the subscription**, not any
  call we make: Transactions plus Recurring, per Item, per month, from
  exchange until removal. The BUSINESS.md contribution table is right to be
  built on it.
