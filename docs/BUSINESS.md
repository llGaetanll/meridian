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
| Spaces: `meridiancash-backups` | 5.00 | Daily encrypted `pg_dump`, 30-day retention (#38). $5 is the Spaces minimum |
| Resend | 0.00 | Free to 3,000 emails/mo, capped 100/day |
| `meridiancash.io` renewal | 3.33 (?) | `.io` is ~$40/yr; amortised |
| Apple Developer Program | 8.25 | $99/yr, unavoidable to ship on iOS |
| Google Play | 0.17 | $25 once |
| New York LLC upkeep | 2.46 (?) | $25/yr state filing fee, $9 biennial statement. See below |
| **Total** | **26.41** | Of which **12.20** is the server and its backups |

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

**Plaid publishes no rates.** The pricing page is a contact-sales page; the
real numbers appear during the Production access request and then in the
billing dashboard. Secondhand reports put per-Item subscriptions in the
$0.30–$3.00/mo range, which is a factor of ten and therefore not a number to
plan on.

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

At $5/mo billed monthly, Stripe leaves $4.55. What Plaid then takes depends
entirely on the per-Item rate:

| Plaid $/Item/mo | 1 Item | 2 Items | 3 Items |
| --- | --- | --- | --- |
| 0.30 | 4.25 | 3.95 | 3.65 |
| 1.00 | 3.55 | 2.55 | 1.55 |
| 2.00 | 2.55 | 0.55 | −1.45 |
| 3.00 | 1.55 | −1.45 | −4.45 |

Fixed costs are $26.41/mo, so break-even is a single-digit number of
subscribers at any rate in the top half of that table. **The price is set by
what the product is worth, not by what it costs to run** — the risk is not the
server, it is the bottom-right of that table.

## What breaks the model

- **A high per-Item rate.** Everything above depends on one number nobody has
  yet. Get it before setting a price.
- **A user with six banks.** Cost is per Item, revenue is not. Either cap
  Items on the cheap tier, or price by Item.
- **Balance uncached.** See above.
- **The Trial plan's 10 Items.** See above.

## Confirm these

- [ ] Plaid: the actual per-Item Transactions rate on the account's plan, from
      the billing dashboard, not from a blog post. **Everything else is
      guesswork until this one is known.**
- [ ] Plaid: is Recurring Transactions a separate subscription or bundled?
- [ ] Plaid: the per-request Balance rate, and whether a failed call bills.
- [ ] Plaid: what plan replaces Trial, its Item ceiling, and its monthly
      minimum — Growth has one.
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
