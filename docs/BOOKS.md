# Books

The bookkeeper: a Python container (`books/`, image `meridian-books`) that
rewrites a Google Sheet called **Meridian Books** once a day, so the money can
be read every morning without asking anybody. It is the automated version of
the "Monthly actuals" table in [BUSINESS.md](BUSINESS.md), and that document
is where every figure it uses is explained.

## The sheet

One tab per month, named `2026-10`. One column per day, one row per line,
money in against money out:

- **Money in** — subscription charges, gross, from Stripe balance
  transactions.
- **Money out** — Stripe's fee on each charge (the fee Stripe actually took,
  not an estimate), refunds, Plaid, and every fixed cost from
  `books/costs.toml` amortized over the month's days.
- **Net**, then subscriber and bank-Item counts for context.

Days that have not happened yet stay blank — the sheet records, it does not
project. The **Summary** tab in front is one row per month.

Plaid is computed rather than fetched — Plaid has no billing API — as live
Items × the per-Item rate in `costs.toml`. Reconcile it against the invoice
when one arrives, and re-quote the rates in `costs.toml` if they move.
Deleted Items undercount past days by exactly the Items Plaid also stopped
billing for, which is the right direction to be wrong in.

## Each pass

Rewrites the current month and the one before it, so the first pass after
midnight on the 1st finalizes the month that ended. A source being down is a
logged gap for that pass, not a crash: the next day's pass rewrites the same
tabs from scratch and heals it.

Run one by hand from the repo root, against whatever `.env` names:

```
python books/main.py once
```

## Configuration

| Variable | What |
| --- | --- |
| `BOOKS_GOOGLE_KEY` | Path to the service-account key. `certs/books-google-key.json` by default; the container mounts it at `/certs/` |
| `BOOKS_STRIPE_KEY` | A **restricted, read-only** Stripe key — balance transactions is all it reads. Never the server's secret key |
| `DATABASE_URL` | Where the Item and subscriber counts come from |
| `BOOKS_SHEET_TITLE` | `Meridian Books` |
| `BOOKS_TZ` | `America/New_York` — the timezone a "day" means |
| `BOOKS_RUN_AT` | `06:30`, in `BOOKS_TZ` |

## Google

The sheet is written as a Google service account, which is a robot Google
account with a key file instead of a password, so no OAuth token ever
expires. The document itself belongs to a person: Google gives a service
account no Drive storage, so one it created would have nowhere to live.
Making the document is therefore a one-time step by hand — create a Google
Sheet named exactly `Meridian Books`, and share it with the address below as
an editor. The service finds it by name and never needs the id.

The account is `books-479@meridian-509902.iam.gserviceaccount.com` in the
Google Cloud project `meridian` (`meridian-509902`), which exists for this
and nothing else. The Sheets and Drive APIs are enabled there; the service
account has no project roles, because it only ever touches files it owns.
A replacement key is minted in the console under IAM & Admin → Service
Accounts → Keys, and the old one revoked in the same place.

The key is a secret. It lives in `certs/`, which is already gitignored and
never enters an image; the container reads it from a read-only mount.

## A new box

Two things reach a box by hand, once, beside the `.env` it already has:

```
scp certs/books-google-key.json meridian@box:/srv/meridian/certs/
```

and in its `.env`:

```
BOOKS_STRIPE_KEY=rk_live_…
```

Both are allowed to be missing: the container starts, says what it is
missing, and writes what it can. A missing *key file* becomes an empty
directory at the mount path — Docker's way — and the pass fails loudly until
the file is there.
