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

It is now the only thing that picks. `runtime_plaid::client()` used to refuse
to build a client for anything but Sandbox, and that one `if` was the whole
protection for the ten slots. It is gone, because a refusal in the constructor
can only say no to everything, and what it was standing in for now exists
properly: entitlement on every route that can reach a bank, so only a paying
subscriber can spend a slot; a per-minute budget on every endpoint that can
spend one; and a write that is retried so a slot is never spent on a bank the
server then drops. **Nothing counts how many Items the account already
holds** — that is still a number somebody reads in the dashboard.

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
export PLAID_REDIRECT_URI=…   # optional; see The two redirects
```

`DATABASE_URL` is required too, and for a reason that belongs here rather than
in [DATABASE.md](DATABASE.md): **the access tokens live in Postgres**, one row
per Item. A server without a database has no way to reach any bank.

**Every variable is required and none has a default**, bar
`PLAID_REDIRECT_URI`, whose absence is an answer rather than a gap.
`server-main` starts with all of them or does not start. It used to fall back to captured fixtures
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

Every request in that list is counted before it goes out — see
`PlaidEndpoint`, which has one variant per endpoint this app calls, each with a
per-minute budget. `/item/public_token/exchange` has the narrowest of them,
because it is the one nothing can undo: each request that lands creates an Item
and the account may hold ten. That stops a burst and nothing slower — nothing
here counts how many Items the account already holds.

## Repairing a bank

A bank whose login has expired answers `ITEM_LOGIN_REQUIRED` to everything,
and no amount of syncing fixes it. The cure is Link's **update mode**: the
same Hosted Link page, requested with the Item's `access_token` and **no
products** — `LinkTokenRequest::update` builds that shape and `POST
/link/repair {"item_id"}` is the route that opens it. The access token stays
the same afterwards; what changes is that the bank starts answering for it
again.

Nothing in a repair can spend an Item slot. Its session's row carries the
`item_id` (`link_sessions.item_id`), and that column is what keeps the finish
from being exchanged — the one call that spends. What a successful repair does
instead is clear `items.error_code`, which is the column `should_fetch`
refuses on, so the next stale snapshot syncs the bank as if nothing had
happened.

`docs/API_FLOW.md` writes it out as flow G. Sandbox never expires a login, so
the way to try it there is `/sandbox/item/reset_login`, which puts a linked
Item into `ITEM_LOGIN_REQUIRED` on purpose.

## The two redirects

Plaid takes two addresses with similar names, and conflating them is why this
looks solved when it is not. They answer different moments and only one of them
can be a custom scheme.

| | Who opens it | When | What it has to be |
| --- | --- | --- | --- |
| `hosted_link.completion_redirect_uri` | Plaid's page | The flow has ended | Anything. `meridian://linked` for the phone, `<origin>/linked.html` for the web |
| `redirect_uri`, top level | **The bank's own app** | Part way through, at an OAuth bank | An https Universal Link, registered in the dashboard |

The first is decided per session, from the `Origin` that `/link/start` was
called with (`LinkOpener`). The phone sends none and gets the scheme. The web
app sends the origin it is served from, and if that is one of `WEB_ORIGINS`
the popup is sent back to a page there that closes itself; a browser cannot
follow a custom scheme, and the popup would otherwise sit on Plaid's last
screen. An origin that is not in the list is treated as none — a header is
the caller's to write, and an address Plaid sends somebody to is one we vouch
for.

Neither needs registering in the dashboard. Plaid's Hosted Link guide says so
of a custom scheme, and Sandbox takes an address it has never seen, http or
https, and sends the browser there when the session ends.

The second exists because an OAuth bank does not take the sign-in on Plaid's
page. It hands off to its own app, and when that app is done the only way it
has of returning control is to open a URL. A custom scheme cannot be that URL:
iOS gives one to whichever app registered it first, so a bank handing back to
`meridian://` is a bank handing a signed-in session to whatever else claimed
the name. Plaid says the same of `android_package_name` — under Hosted Link it
cannot stand in either.

Sending `is_mobile_app` without that address is worse than sending neither. It
tells Plaid a native app is what opened the session, and the user is then left
in their bank's app *after* signing in, with nowhere to go back to. So the two
travel together and there is no way through `LinkTokenRequest` to say one
without the other: `PLAID_REDIRECT_URI` is what sets both, and unset means the
link token is the one a browser gets, where an OAuth bank stays inside the web
session. A session the web app opens never carries them, set or not.

Three things have to be true before it can be set, and all three are about the
phone rather than about Plaid:

- the app is signed by a team holding the `associated-domains` entitlement
- `MERIDIAN_TEAM_ID` names that team on the box, so Caddy serves an
  `/.well-known/apple-app-site-association` that points at the app
- the address is registered in the Plaid dashboard

**It is testable without spending an Item.** Sandbox has OAuth institutions —
`ins_127287` for plain OAuth and `ins_132241` for the app-to-app handback this
is about. Any credentials are accepted, including none.

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

### What `/link/finish` is for

Two jobs, and only one of them survives webhooks.

With `PLAID_COLLECT=poll` it is a **trigger**: nothing else ever collects, so
this is what goes and asks Plaid. With `PLAID_COLLECT=webhook` collection
happens unprompted, and it degrades to a **question** — "what happened to my
session?" — asked, often, a fraction of a second before the answer exists.

That gap is not a bug to design away. Plaid redirects the *browser* and POSTs
the *server*, with no ordering between them, and the phone only ever sees the
first. Plaid's own documentation says you cannot assume the redirect means a
successful connection and must listen for `SESSION_FINISHED` or call
`/link/token/get`. So the server — the only party that sees both — waits:

- The outcome may already be here. Plaid usually wins the race, so a recorded
  one is looked for before anybody is parked, or the ordinary case would sit
  until it timed out.
- Otherwise the route parks on the caller's open sessions, matched by
  `link_token`. `SESSION_FINISHED` carries one and `link_sessions` already
  stores it, so this needs nothing new in the schema.
- After ten seconds it answers **pending**. Not failure: the bank may still be
  landing, and the screen that follows watches `/sync/status` and will see it.

A session is collected — `collected_at` — by whatever *reports* it, and not a
moment earlier. A collected session is one `/link/finish` can no longer see,
so collecting it when the webhook lands would throw away the record the route
is about to match against.

The three answers are `linked`, `abandoned` and `pending`. `abandoned` is now
something Plaid *said* — `SESSION_FINISHED` carries `status: SUCCESS | EXITED`
— where it used to be inferred from a poll that came back empty.

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
