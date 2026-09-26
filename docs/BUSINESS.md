# Business

One product, sold direct to individuals, monthly subscription. No ads, no
funding, no sales team. Growth is word of mouth, so the only lever is quality.

Every number marked **(?)** is a guess until confirmed. The list of what to
confirm is at the bottom.

## Fixed costs — do not move with subscribers

| Line | $/mo | Note |
| --- | --- | --- |
| DigitalOcean droplet `s-1vcpu-1gb` | 6.00 | 25 GB. Server 11 MiB, Postgres 98 MiB — it is not close to full |
| Droplet backups (20%, weekly) | 1.20 | Cheapest backup DO sells |
| Volume: `meridian-db`, 10 GiB | 1.00 | Postgres data, encrypted at rest by DO (#33) |
| Spaces: `meridiancash-backups` | 5.00 | Daily encrypted `pg_dump`, 30-day retention (#38). $5 is the Spaces minimum |
| Resend | 0.00 | Free to 3,000 emails/mo, capped 100/day |
| `meridiancash.io` renewal | 3.33 (?) | `.io` is ~$40/yr; amortised |
| Apple Developer Program | 8.25 | $99/yr, unavoidable to ship on iOS |
| Google Play | 0.17 | $25 once |
| New York LLC upkeep | 2.46 (?) | $25/yr state filing fee, $9 biennial statement. See below |
| **Total** | **27.41** | Of which **13.20** is the server and its storage |

Next step up is $12/mo (2 GB) — only when Postgres or the image build on the
box says so, which is not soon.

## The company — required before release, and the biggest bill so far

**Apple will not take a finance app from an individual, and Google Play likely
will not either.** The website has no such rule. Why, and what else applies,
is in [COMPLIANCE.md](COMPLIANCE.md).

The entity is **Meridian Studios LLC**, a New York LLC filed 2026-09-14 with an
Albany address. New York makes a new LLC **publish a notice in two newspapers
for six weeks**, within 120 days, in the county named on its filing. That
notice is most of the cost, and the county is the whole difference:

| Line | Manhattan | Albany address | Note |
| --- | --- | --- | --- |
| Articles of Organization | 200 | 200 | The filing that creates the LLC |
| **Newspaper notice** | **~1,800** | **~400** | Priced by the county, not the state |
| Certificate of Publication | 50 | 50 | Proof the notice ran |
| Registered agent in Albany | — | ~125 (?) | Only to have an Albany address. A Certificate of Change can move it back once the notice has run |
| D-U-N-S number | 0 | 0 | Apple and Google both require one |
| **One-time** | **~2,050** | **~775** | |

- **An Albany address is the usual way round it** and is what registered
  agents sell it for. Confirm with an accountant.
- **The notice has not started.** The Certificate of Publication is due by
  2027-01-12.
- **Delaware or Wyoming saves nothing.** Run from New York, the LLC still has
  to register here ($250) and still publish.
- **A New York corporation has no notice** ($125 to form), but it files its
  own tax return every year, where a single-member LLC is a line on a personal
  one. The accountant's fee for that return is likely to eat the saving within
  a year or two (?).
- **Not publishing** does not dissolve the LLC; it suspends its authority to
  do business in New York, including suing in its courts, until the notice
  runs.
- **Business tax in NYC** (UBT) only starts to matter past roughly $95k of
  revenue.

## Plaid

Plaid is the only cost that grows with use, and it is billed per **Item** — one
Item is one bank login, and a person has as many as they have banks. It is not
billed per user, so a user's cost is whatever their Items cost.

Three billing shapes, and which one a product uses is the whole of its cost:

| Shape | Charged | What Meridian uses |
| --- | --- | --- |
| Subscription | Monthly, per Item, for as long as the Item exists | `/transactions/sync`, `/transactions/recurring/get` |
| Per request | Per successful call | none |
| One-time | Once per Item, at link | none |

**The rates, quoted at the Production application 2026-09-26.** Pay As You
Go — no commitment, no quantity discounts:

| Product | $/Item/mo |
| --- | --- |
| Transactions | 0.30 |
| Recurring Transactions | 0.15 |
| Investments Holdings | 0.18 |
| Investments Transactions | 0.35 |

A bank Item — Transactions plus Recurring — costs **$0.45/mo**, and a
brokerage Item carrying all four subscriptions **$0.98/mo**. Recurring is
confirmed as its own subscription rather than bundled. The secondhand
$0.30–$3.00 range this replaces was right only at its floor.

Three things follow regardless of the rate:

- **A subscription bills until the Item is removed**, not until the user stops
  using it. A cancelled account means `/item/remove` the same day.
- **No call the app makes bills per use.** Balances come from `/accounts/get`,
  Plaid's free cache, rather than the billed `/accounts/balance/get` — so a
  daily user and a light user cost the same, and the per-request column stays
  empty unless somebody adds a call to it. `docs/PLAID_API.md` is where to
  check before adding one.
- **The Trial plan allows 10 Production Items and `/item/remove` does not free
  the slot.** That is a hard ceiling of a handful of users. Moving off Trial
  comes before the first paying customer, not after.

Resend is the other per-use cost and it is free at this size: roughly two
sign-in emails per user per month against a 3,000/mo allowance.

## Payments

**$5.99 includes sales tax.** New York taxes SaaS as prewritten software, and
the LLC is a New York one, so the tax is owed on New York customers from the
first sale. The price is the figure the customer is charged, so the tax comes
out of it rather than going on top of it.

At New York City's 8.875%, $5.99 is $5.50 of revenue and $0.49 of tax. Stripe's
fee is charged on the whole $5.99, not on the $5.50, so it is paid on money that
was never ours:

```
  5.99 − 0.49 tax − 0.474 Stripe = 5.03
```

Outside the city the rate is lower and the same arithmetic leaves more. Apple is
the merchant of record for In-App Purchase and handles the tax itself, so this
applies to the web rail only.

The tables below are still written against a $5 price and do not subtract tax.
They understate the fee and overstate the contribution.

Stripe takes **2.9% + $0.30** per charge. At $5/mo that is $0.445 — 8.9%, and
almost all of it the flat 30¢.

| Billing | Fee per user/yr | Effective take |
| --- | --- | --- |
| Monthly at $5 | $5.34 | 8.9% |
| Annual at $50 | $1.75 | 2.9% |

Annual billing is worth roughly one free month per user. Offer it.

## Per-user economics

```
  contribution = price − (2.9% × price + $0.30) − (Plaid per Item × their Items)
```

At $5/mo billed monthly, Stripe leaves $4.55. At the quoted rates a bank
Item takes $0.45 and a brokerage Item $0.98:

| Their Items | Plaid | Contribution |
| --- | --- | --- |
| 1 bank | 0.45 | 4.10 |
| 2 banks | 0.90 | 3.65 |
| 2 banks + 1 brokerage | 1.88 | 2.67 |
| 3 banks + 2 brokerages | 3.31 | 1.24 |

Fixed costs are $27.41/mo, so break-even is a single-digit number of
subscribers. **The price is set by what the product is worth, not by what it
costs to run** — and at these rates the old fear is retired: it takes a
double-digit Item count to push a $5 subscriber under water.

## What breaks the model

- **The quoted rates moving.** Pay As You Go commits neither side; the
  numbers above are 2026-09-26's, and a repricing lands on every Item at
  once.
- **A user with six banks.** Cost is per Item, revenue is not. Either cap
  Items on the cheap tier, or price by Item.
- **Balance uncached.** See above.
- **The Trial plan's 10 Items.** See above.

## Confirm these

- [x] Plaid: the per-Item rates, quoted at the application 2026-09-26 and in
      the table above. Confirm they match the billing dashboard once the plan
      is live
- [x] Plaid: Recurring Transactions is a separate subscription, priced apart
- [ ] Plaid: the per-request Balance rate, and whether a failed call bills.
- [x] Plaid: Pay As You Go replaces Trial — no minimum, no commitment. No
      Item ceiling was shown (?)
- [ ] `.io` renewal price at Namecheap.
- [ ] Stripe: whether Link/wallet or non-US cards change the 2.9% + 30¢.
- [ ] LLC: an accountant's word that an Albany address is fine for an LLC run
      from Manhattan.
- [ ] LLC: the $25/yr filing fee and $9 biennial statement, and what an Albany
      registered agent actually charges.

## Monthly actuals

Fill this in on the 1st. Estimates above are worth nothing next to two real
months.

| Month | Subs | MRR | Plaid | DO | Stripe | Other | Net |
| --- | --- | --- | --- | --- | --- | --- | --- |
| | | | | | | | |
