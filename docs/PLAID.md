# Plaid

Plaid is where transactions come from. `src-plaid` talks to it, the same shape
as the price sources: plain async functions over plain data, no signals and no
decisions.

## Sandbox only

**Any agent working on Plaid uses Sandbox. Only the user switches to
Production, and only by saying so.**

This is not caution for its own sake. The account is on Plaid's Trial plan,
which allows **10 Production Items** — one Item is roughly one bank login — and
`/item/remove` **does not give the slot back**. Ten careless connections and
the account is finished, permanently. Sandbox has no such limit and no real
credentials in it, so every piece of development happens there.

The default is structural, not a promise: `PLAID_ENV` unset means Sandbox, and
Production takes a deliberate `PLAID_ENV=production`. The lazy path is the safe
one, and there is no way to reach real data by forgetting something.

Sandbox logins are `user_good` / `pass_good` at any institution.

## The two environments

| Env | Host | Data |
| --- | --- | --- |
| Sandbox | `https://sandbox.plaid.com` | Fake institutions, fake credentials |
| Production | `https://production.plaid.com` | Real banks, real money |

There is no Development environment. It was removed in 2024, so a tutorial that
mentions one is out of date about more than that.

## Credentials

Three variables, never committed:

```sh
export PLAID_CLIENT_ID=…    # one per team, same in both environments
export PLAID_SECRET=…       # one per environment — Sandbox and Production differ
export PLAID_ENV=sandbox    # or unset, which means the same thing
```

They live in the Dashboard under
[developers/keys](https://dashboard.plaid.com/developers/keys).

The secret is what makes the server exist. It grants full access to every Item
on the account, so it cannot ship inside the app — anyone who unzips the
binary would have it. Every call that carries the secret happens server-side,
and the only thing the phone ever sees is the transactions that come back.

## The flow

```
server: POST /link/token/create          -> link_token, hosted_link_url
phone:  open hosted_link_url             -> the user logs into their bank
server: POST /item/public_token/exchange -> access_token, stored, permanent
server: POST /transactions/sync          -> added / modified / removed
```

Only the middle step is not ours: the user is on Plaid's own page for it.
That is Hosted Link, which is a URL rather than an SDK, so nothing
platform-specific has to be integrated to connect a bank.

The `access_token` is permanent and the flow above is run once, ever. The
`cursor` from `/transactions/sync` is what makes the next sync incremental, so
it is saved beside the token and passed back on the following call.

## Three ways to be quietly wrong

`src-plaid` holds Plaid's types as Plaid reports them, so none of these is
handled there. Whatever reads a `PlaidTransaction` into the ledger owns them
all, and each is worth a test, because none looks like a bug from the outside —
the numbers stay plausible either way.

**Sign.** Plaid reports a card purchase as a *positive* `amount` — positive
means money left the account. The ledger has the opposite convention, so
something has to negate, once, in one place.

**Pending duplicates.** A pending transaction is not updated in place when it
posts. A *new* transaction arrives with a different `transaction_id` and a
`pending_transaction_id` pointing at the old one, which must then be dropped.
Ignore this and every purchase is counted twice for the few days between
authorisation and settlement.

**A sync is a diff, and the second one usually returns nothing.** This is the
one that actually bit us. `/transactions/sync` answers with what has changed
since the cursor, so the first call returns the whole history and a call a
minute later legitimately returns three empty lists. Assign that over the held
ledger instead of applying it and the app goes blank — while every log line
still says the sync succeeded, because it did.

The fix is that the ledger accumulates: `added` joins, `modified` replaces by
id, `removed` leaves, and nothing else is touched. `Ledger::apply` does this
and `StateServer` is what calls it. It is worth knowing that no unit test
starting from a fresh state can catch this; it only shows up on the *second*
fetch, so the test has to be one.
