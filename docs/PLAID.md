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

`PLAID_ENV` is **required**, and matched exactly: `sandbox` or `production`,
and anything else — unset, empty, `Sandbox`, `dev`, a leftover from another
project — fails the process on the way up.

This used to be a default. Unset meant Sandbox, on the reasoning that the lazy
path should be the harmless one. The reasoning was right and the mechanism was
wrong: a default is a decision made by whoever did not look, and it made a
whole family of mistakes indistinguishable from a correct configuration. An
environment that has not said which Plaid it means now reaches neither.

Sandbox logins are `user_good` / `pass_good` at any institution.

Link's first screen asks for a phone number, and in Sandbox it takes only
Plaid's own test numbers — `415-555-0011` and its family. A real number is
rejected there, and the rejection is Plaid's page rather than anything of ours;
nothing in Meridian asks for a phone number at all. The screen offers a way
past it to the institution list.

## Linking a Sandbox bank

Two ways. The real one is Hosted Link, which the server now speaks:

```
POST /link/start   -> { "url": "https://secure.plaid.com/hl/…" }
                      open it, sign in as user_good / pass_good
POST /link/finish  -> { "linked": true }
```

Both need a session — see the sign-in flow. `/link/start` writes the session
down against the caller *before* handing back the URL, which is what stops one
account collecting a bank another account connected.

The other way needs no browser, which is what makes it useful from a terminal:

```sh
./scripts/db-init.sh                                              # if Postgres is not up
cargo run -p server-main --example link_sandbox -- you@example.com
cargo run -p server-main --example link_sandbox -- you@example.com ins_109509
```

It creates a Sandbox Item and writes it to `items`, access token and all. The
server picks it up at boot and syncs on the first request.

**It costs nothing.** `/sandbox/public_token/create` does not exist in
Production, so there is no version of this call that can spend an Item slot.
Run it as often as you like.

It skips the middle of [the flow](#the-flow) — the part where a person signs
into their bank on Plaid's page. Everything after that is the ordinary code
path: the same `public_token`, the same exchange, the same Item.

**`is_mobile_app` is false on the link token we create.** Plaid rejects the
request otherwise unless a top-level `redirect_uri` is set too, and that one
has to be an https address registered in the dashboard — unlike the custom
scheme in `completion_redirect_uri`, which needs nothing. It changes how an
OAuth bank hands control back, so it has to be revisited before a real bank is
linked from a real phone.

Sandbox generates transactions asynchronously, so a `/transactions/sync` in the
first seconds after linking can answer empty or `PRODUCT_NOT_READY`. Ask again.
Recurring streams are derived from transactions and stay thin until those land.

## The two environments

| Env | Host | Data |
| --- | --- | --- |
| Sandbox | `https://sandbox.plaid.com` | Fake institutions, fake credentials |
| Production | `https://production.plaid.com` | Real banks, real money |

There is no Development environment. It was removed in 2024, so a tutorial that
mentions one is out of date about more than that.

## Credentials

The variables, never committed:

```sh
export PLAID_CLIENT_ID=…      # one per team, same in both environments
export PLAID_SECRET=…         # one per environment — Sandbox and Production differ
export PLAID_ENV=sandbox      # required; `sandbox` or `production`, exactly
export PLAID_COLLECT=poll     # required; `poll` or `webhook`
export PUBLIC_URL=…           # required only when PLAID_COLLECT=webhook
```

`DATABASE_URL` is required too, and for a reason that belongs here rather than
in [DATABASE.md](DATABASE.md): **the access tokens live in Postgres**, one row
per Item. A server without a database has no way to reach any bank.

**Every variable is required and none has a default.** `server-main` starts
with all of them or does not start. It used to fall back to captured fixtures
when the credentials were missing, which meant an empty screen had two
explanations and the log said the same thing for both.

There is no `PLAID_ACCESS_TOKEN`. There was, and it was one hardcoded token for
one institution — which is all an access token ever is, so a variable could
only ever hold the first one. They come from the store now, keyed by `item_id`,
which is what lets there be a second bank.

`PLAID_COLLECT` is not `PLAID_ENV` and must not be folded into it. It says how
a finished Link session reaches us — see [The flow](#the-flow) — and the two
are independent: polling against Production is sensible, and so is a webhook
against Sandbox once there is a tunnel to receive it.

`PUBLIC_URL` is required *only* when `PLAID_COLLECT=webhook`, the same way
`RESEND_*` is only read when `EMAIL_SEND=resend`. It is this server's address
on the internet, https and with no trailing slash — Plaid is told to call
`<PUBLIC_URL>/plaid/webhook`. A process cannot work this out for itself: it
knows the port it bound and nothing about the name or the proxy in front of it.
A server in webhook mode that has not been told fails on the way up, rather
than starting, telling Plaid nothing, and waiting forever for a call.

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

## Webhooks

The other channel, and the one a deployed server should prefer. Plaid POSTs
`<PUBLIC_URL>/plaid/webhook` and the server acts on it; nothing is polled. `PLAID_COLLECT` picks between the two, and the laptop stays on `poll`
because Plaid cannot reach a home network.

The route is registered in the Plaid dashboard *and* sent on each link token
(`notifying()`), which is what makes `SESSION_FINISHED` arrive at all — the
dashboard address covers Item and Transactions webhooks, and a Link session
only calls the address its own token carried.

Three types are acted on. Everything else is taken and ignored, because Plaid
sends types nobody planned for and **retries anything not answered 2xx**:

| Webhook | What happens |
| --- | --- |
| `LINK` / `SESSION_FINISHED` | The `public_token` is exchanged. This is what polling does. |
| `TRANSACTIONS` / `SYNC_UPDATES_AVAILABLE` | That Item is synced, freshness window skipped. |
| `ITEM` / `ERROR` with `ITEM_LOGIN_REQUIRED` | Written down, so a restart does not ask again and be refused again. |

`/link/finish` is still called by the phone in both modes. In webhook mode it
no longer asks Plaid anything: by then the bank is stored, so the route reads
how many banks arrived while a session of this user's could have been open.
There is a race in that — a phone that calls it before the webhook lands is
told nothing was attached — and the bank still appears on the next snapshot.

### Believing one

**This is the first unauthenticated route in the server that does work.**
Anyone can POST to it, so every webhook is verified before anything at all is
decided, in this order and no other:

1. The JWT comes out of the `Plaid-Verification` header.
2. Its `alg` must be `ES256`. **Checked first**, because step 4 is an outbound
   call and `alg: none` must never get that far.
3. `kid` comes off the JWT header.
4. The key is fetched from `/webhook_verification_key/get` — **and cached by
   `kid`, misses included**. Without the cache an anonymous POST is a Plaid
   request, which is a denial of service we would have built ourselves.
5. The signature is checked against the key.
6. `iat` must be within five minutes, in either direction. The signature never
   expires, so this is the only thing that stops a captured webhook being
   replayed tomorrow.
7. The **raw body bytes** are SHA-256'd and compared with
   `request_body_sha256`, in constant time.

Step 7 is the one that catches people. Plaid hashes what it put on the wire,
which is pretty-printed with two spaces, so a body that has been through a JSON
extractor and printed again hashes to something else and *every* webhook is
refused for a reason that reads like an attack. The route takes `Bytes`, and
the body is not parsed until the hash has matched.

Every failure answers 401 and says nothing about which check failed. The reason
is in the log.

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
