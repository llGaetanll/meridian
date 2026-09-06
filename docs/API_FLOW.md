# The API flow

One flow, written out end to end: a phone that has never run before, from the
first request it makes to a ledger on screen. Every entry is one request or one
response, and every entry says who called and who answered.

The phone is the example because it is the only client that walks the whole
thing. The web app speaks the same HTTP on the same routes. It just starts
further along, with a session already in hand, and fetches snapshots.

## How to read it

`a` -> `b` is a request from `a` to `b`. `a` <- `b` is `b` answering it. The
party that started the exchange stays on the left either way, so the arrow is
the only thing that has to be read to know which direction the bytes went.

Flows are lettered, A to F, and C has two variants that do the same job by
different means. Each flow numbers its own entries from 1, so a bare number is a
reference within that flow and anything reaching across one names the route
instead. Where a request has more than one possible answer, every one of them is
lettered under it.

## The cast

| Who | What it is |
| --- | --- |
| `phone` | The iOS app. `RuntimeIo.swift` is every call it makes. |
| `sheet` | `ASWebAuthenticationSession`, a real browser the phone raises, not a screen the app draws. |
| `server` | `server-main`, at `MeridianServer` from the bundle. Holds the Plaid secret. |
| `plaid` | `https://sandbox.plaid.com`. Only ever Sandbox, see `docs/PLAID.md`. |
| `resend` | `https://api.resend.com`. Delivers the sign-in code, when `EMAIL_SEND=resend`. |

Link is Plaid's, and the capital is theirs: it is the product name for the
sign-in flow a bank customer walks, and Hosted Link is the variant we use,
where Plaid hosts the page instead of an SDK drawing it. So a Link session, a
`link_token` and Plaid's Link page are all one proper noun, and none of them is
a hyperlink.

Two variables change the shape of this. `PLAID_COLLECT` decides how the result
of a bank sign-in gets back to us, and it is the one that changes an actual
flow: C is written out twice, once per channel. `EMAIL_SEND` decides whether a
code is mailed or written to the log, and the list below is `EMAIL_SEND=resend`.

## Before any of it

The first thing the app does on launch is `LoadSession`, a read of the device's
own keychain, no network. On a first run it comes back empty, first run stays
on screen, and nothing has been sent to the server yet. So the numbered list
starts where the user types an address.

The web app is a browser, so every authenticated call below is preceded by an
`OPTIONS` preflight that the CORS layer answers above the routes. It is
transport, it is not logged, and the phone sends none of it.

## A. Signing in

Starts the moment somebody taps Continue on the address screen of first run,
and runs again on any launch where the keychain came back empty or where the
server has stopped honouring the token it held.

1. `phone` -> `server` - `POST /auth/request {"email"}`: *Asks for a six-digit sign-in code.*
2. `server` -> `resend` - `POST /emails`: *Sends the code out, its SHA-256 and ten-minute expiry already written to Postgres.*
3. `server` <- `resend` - `200 {"id"}`: *Accepted for delivery. The id is Resend's own id for that message, and the only thing tying a line in our log to a row in their dashboard.*
4. `phone` <- `server` - one of:
   a. `204 No Content`: *Taken, and deliberately the same answer whether or not that address has an account here.*
   b. `429 Too Many Requests`: *Too many asks from this caller, or too much mail already sent to that inbox.*
   c. `503 Service Unavailable`: *No entropy has reached the machine yet, and it will not mint a code it cannot make unpredictable.*
5. `phone` -> `server` - `POST /auth/verify {"email","code"}`: *Trades the typed digits for a session.*
6. `phone` <- `server` - one of:
   a. `200 {"token"}`: *A bearer token good for ninety days. The code is spent in the same turn, so a replayed one is worth nothing.*
   b. `401 Unauthorized`: *Wrong, expired, guessed at too many times, or no code standing at all. One shape for all four.*
   c. `429 Too Many Requests`: *Too many attempts from this caller.*
   d. `500 Internal Server Error`: *Postgres would not answer the read of the code standing against that address. The only way this happens is the database being down or the query failing.*

## B. Opening a bank sign-in

Starts on the Connect a bank tap, which is the screen the code screen leads to,
and runs again unchanged for Try again and for Add another bank later on.

1. `phone` -> `server` - `POST /link/start` with `Authorization: Bearer <token>`: *Asks for a Link session for whoever the token signs in.*
2. `server` -> `plaid` - `POST /link/token/create`: *Creates the hosted session, with `meridian://linked` as the completion redirect.*
3. `server` <- `plaid` - `200 {"link_token","hosted_link_url"}`: *The session and the page that hosts it. Nothing is spent by this, so it is safe to repeat.*
4. `phone` <- `server` - one of:
   a. `200 {"url"}`: *The page to open. A `link_sessions` row keyed by the `link_token` is written first, carrying the `user_id` that the bearer token resolved to, which is what stops one account collecting a bank another account connected.*
   b. `401 Unauthorized`: *No bearer token, or one the server no longer honours.*
   c. `429 Too Many Requests`: *Too many sessions opened from this caller.*
   d. `500 Internal Server Error`: *The session this caller holds could not be read. Not `502`: that points at Plaid, and this went wrong here.*
   e. `502 Bad Gateway`: *Plaid refused, was unreachable, or answered a `link_token` with no hosted URL. All three become one abandoned session inside.*
5. `sheet` -> `plaid` - `GET https://secure.plaid.com/hl/...`: *Plaid's own page, where the user picks a bank and signs into it. Nothing of ours is in this exchange.*
6. `sheet` <- `plaid` - one of:
   a. `302 meridian://linked`: *Closes the sheet. It carries no token and is not proof a bank was connected, but it is the one signal that separates a finished flow from a dismissed one, which is what decides whether the phone asks the next question at all.*
   b. nothing at all: *The sheet was dismissed, which is an ordinary ending. The phone calls it abandoned without asking the server anything.*

## C. Collecting what the session came to

Starts when Plaid's redirect closes the sheet. The redirect carries no token, so
the result has to be collected over a separate channel, and there are two of
them: `PLAID_COLLECT` picks which. They are independent of each other and both
are Plaid's own, so the choice is about where *we* are running rather than which
Plaid we are talking to.

Polling needs no address of our own, which is what makes it the one that works
from a laptop. The webhook is prompter and is what a deployed server should
prefer, but it needs an address Plaid can reach.

Either way the flow ends in the same place: an `access_token` in the runtime and
D starting on its own.

### C1. By polling (`PLAID_COLLECT=poll`)

Here `/link/finish` is a *trigger*. Nothing else ever collects, so this request
is what goes and asks Plaid, and a sheet the user dismissed never gets here at
all.

1. `phone` -> `server` - `POST /link/finish` with `Authorization: Bearer <token>`: *The sheet closed, so go and find out what happened.*
2. `server` -> `plaid` - `POST /link/token/get`: *Asks what became of every Link session this user has open, because the request does not say which one closed.*
3. `server` <- `plaid` - one of:
   a. `200` finished, with `public_token`s: *One per Item the session produced, so somebody who connected two banks in one sitting yields two. The first is exchanged and no more; see the note under this variant.*
   b. `200` finished, with none: *They closed the sheet without connecting anything. The session is marked collected so it is not polled forever.*
   c. `200` not finished: *Somebody is still typing their password. The session is left standing and the next close asks again.*
4. `server` -> `plaid` - `POST /item/public_token/exchange {"public_token"}`: *Trades it for the permanent one. The only irreversible call in the flow: in Production this spends an Item slot that removing the Item does not give back.*
5. `server` <- `plaid` - `200 {"access_token","item_id"}`: *The bank is attached. The access token stays in the runtime and never enters an event, an action or a log.*
6. `phone` <- `server` - one of:
   a. `200 {"outcome":"linked","count":1}`: *A bank is attached. The count, not a name: nothing knows what the bank is called yet.*
   b. `200 {"outcome":"abandoned"}`: *Nothing was collected. Today this is also what an unfinished session answers, so a bank Plaid has not stamped yet reads as one that was never connected.*
   c. `401 Unauthorized`: *No bearer token, or one the server no longer honours.*
   d. `429 Too Many Requests`: *Too many closes from this caller.*
   e. `500 Internal Server Error`: *This user's open sessions could not be read, or the poll asking Plaid about them never came back. Deliberately not "abandoned": that is a sentence about somebody's bank, and this is not knowing.*

One close attaches one bank. The first `public_token` found is exchanged and
that session is marked collected in the same turn, so a second Item made inside
the *same* Link session has no later close that can reach it: its token is
dropped. Sessions the caller has open *besides* that one are left standing, and
the next close asks about them again.

### C2. By webhook (`PLAID_COLLECT=webhook`)

Here collection happens unprompted, so `/link/finish` stops being a trigger and
becomes a *question*, one that can be asked a fraction of a second before the
answer exists. 1 and 2 race, and usually 2 wins: the phone has a redirect, an
app switch and a request to get through, and Plaid has none of that.

1. `phone` -> `server` - `POST /link/finish` with `Authorization: Bearer <token>`: *The sheet closed. It sets nothing going here; it asks what the session came to and waits up to ten seconds for an answer.*
2. `plaid` -> `server` - `POST /plaid/webhook` with `Plaid-Verification: <JWT>`: *`SESSION_FINISHED`, with the `public_token`s on it. Sent whether or not anybody asked.*
3. `server` -> `plaid` - `POST /webhook_verification_key/get`: *Fetches the key that JWT names. Once per `kid`, then cached, hits and misses both, so an anonymous POST cannot become an outbound call at will.*
4. `server` <- `plaid` - `200` JWK: *The key the signature is checked against, over the raw bytes Plaid put on the wire.*
5. `server` -> `plaid` - `POST /item/public_token/exchange {"public_token"}`: *One call per token, and every token is exchanged: Plaid says this once, so a token nobody trades is a bank somebody connected and does not get.*
6. `server` <- `plaid` - `200 {"access_token","item_id"}`: *One per exchange, and the same irreversible call as C1's step 4.*
7. `plaid` <- `server` - one of:
   a. `204 No Content`: *Believed and taken, sent once the exchanges have landed and been written. That is what makes a failure mid-exchange a webhook Plaid sends again rather than a bank quietly lost.*
   b. `401 Unauthorized`: *The POST was not Plaid's. The reason is logged and not returned.*
   c. `503 Service Unavailable`: *The machine could not take it, so Plaid sends it again in a minute. That is what we want.*
8. `phone` <- `server` - one of:
   a. `200 {"outcome":"linked","count":n}`: *The webhook landed first, or landed while this request waited.*
   b. `200 {"outcome":"abandoned"}`: *Plaid said the session produced nothing. The first time this is something Plaid said rather than something inferred from a poll that came back empty.*
   c. `200 {"outcome":"pending"}`: *Ten seconds passed and Plaid still has not said. Not "no bank": the phone goes to the sync screen and polls, and a late bank shows up there. It gives up after twenty.*
   d. `401 Unauthorized`: *No bearer token, or one the server no longer honours.*
   e. `429 Too Many Requests`: *Too many closes from this caller.*
   f. `500 Internal Server Error`: *This user's open sessions could not be read. Not `abandoned` and not `pending`: both of those are things Plaid said, and this is Postgres.*

## D. The first sync, and watching it

Starts in the same turn the Item is written down, before the answer above has
even gone out, and the phone watches it from the sync screen until the server
says the three Plaid calls are through.

1. `server` -> `plaid` - `POST /accounts/balance/get`: *The first of three, started without anybody asking rather than waiting for a `/snapshot` to trigger it.*
2. `server` <- `plaid` - `200` accounts and balances: *The first place the bank's name exists. It is in neither the Link session nor the exchange.*
3. `phone` -> `server` - `GET /sync/status` with `Authorization: Bearer <token>`: *The first ask, on the news that a bank landed. Every one after it is a tick's, about once a second.*
4. `phone` <- `server` - one of:
   a. `200 {"stage","institution","accounts","transactions","streams","finished":false}`: *How far the sync has got, answered immediately whatever is happening.*
   b. `200 {...,"finished":true}`: *Nothing is running any more, and the totals are on this reply. The screen that says "You're set." reads them from here.*
   c. `401 Unauthorized`: *No bearer token, or one the server no longer honours.*
   d. `429 Too Many Requests`: *Polling harder than the limit allows.*
   e. `500 Internal Server Error`: *This user's banks could not be read, so there is no telling what is running. An empty status would say "nothing is syncing", which is a different sentence and one we do not know to be true.*
5. `server` -> `plaid` - `POST /transactions/sync`: *The history, as a diff from the cursor. A bank attached seconds ago has no cursor, so this is the long one.*
6. `server` <- `plaid` - one of:
   a. `200` added, modified, removed and a `next_cursor`: *Written to Postgres, cursor last and separately, so a failure earlier in the batch means the next sync re-reads rather than skipping.*
   b. `400 PRODUCT_NOT_READY`: *Sandbox generates transactions asynchronously, so the first seconds after linking can answer this. Ask again.*
7. `server` -> `plaid` - `POST /transactions/recurring/get`: *The recurring streams.*
8. `server` <- `plaid` - one of:
   a. `200` streams: *Thin or empty on a fresh Item, because Plaid derives them from transactions that have only just landed. That is a thin screen, not a failed sync.*
   b. any error: *Logged and carried on from. Transactions are what the app is for, and a missing recurring list costs one screen rather than all of them.*
9. `phone` -> `server` - `GET /sync/status`: *3 again, and again, once a second while the sync screen is up.*
10. `phone` <- `server` - `200 {...,"finished":true}`: *The last one. The sync screen turns into the one with the button that ends first run.*

## E. Into the app

Starts on the tap that ends first run, and is the same pair of lines as every
launch and every pull-to-refresh afterwards.

1. `phone` -> `server` - `GET /snapshot` with `Authorization: Bearer <token>`: *First run is through, the session has been committed to the keychain, and this is the ledger. It takes no parameters: pull-to-refresh sends this exact request, because when to go back to a bank is the server's decision and not a gesture's.*
2. `phone` <- `server` - one of:
   a. `200` `WireSnapshot`: *Entries, connections and streams in one document, which is why switching tabs afterwards costs nothing.*
   b. `401 Unauthorized`: *The session has run out. It arrives as a failed load and a banner; the stored session is thrown away only on a deliberate sign-out.*
   c. `429 Too Many Requests`: *Refused before the ledger is read out of Postgres, so a declined request does not pay for the expensive part.*
   d. `503 Service Unavailable`: *The session or this user's banks could not be read. Not an empty ledger: that is what somebody with no bank sees, and serving it because Postgres was down would show a first run to somebody who has one.*

That is the whole of it. From here the phone makes exactly two kinds of call:
`GET /snapshot` on launch and on pull-to-refresh, and `POST /link/start` again
if somebody adds a second bank. Neither carries a way to insist on anything:
whether a `/snapshot` becomes three calls to Plaid or is served straight out of
Postgres is `should_fetch`'s decision, made against a freshness window and
Plaid's own per-endpoint budget.

## F. Webhooks once a bank is connected

Nothing to do with linking, and they arrive for the life of the Item. They need
`PLAID_COLLECT=webhook` only in the sense that they need an address Plaid can
reach; there is no polling equivalent for either, so a server without one learns
both of these late.

1. `plaid` -> `server` - `POST /plaid/webhook`: *`SYNC_UPDATES_AVAILABLE`, which starts a `/transactions/sync` nobody asked for and nobody waits on. Without it, new transactions land only when a client asks and the freshness window has expired.*
2. `plaid` -> `server` - `POST /plaid/webhook`: *`ITEM_LOGIN_REQUIRED`, written to the item's row so a restart does not cheerfully ask Plaid the same question and get the same refusal. Without it we find out one refused call later.*

Both are answered exactly as C2's step 7 answers a webhook, and for the same
reasons.

## Things worth knowing about the shape

Every route but the webhook needs the bearer token, and the webhook needs a
signature instead. There is no unauthenticated read here. `/snapshot` is
somebody's bank transactions.

The client never holds a Plaid token of either kind. The secret that signs
every Plaid call grants access to every Item on the account, so it cannot ship
inside an app anybody can unzip. That is why `link/token/create`,
`link/token/get` and `item/public_token/exchange` are the server's calls and
not the phone's.

Every route is rate limited except the webhook, which is protected by the
signature instead. A limit set slightly wrong there would break bank linking
silently. A refused request is a bare `429` with no `Retry-After`.

The phone polls because the server can only answer questions it is asked. Every
poll is an action the machine decided on a tick, which is what puts it in the
event log. A loop in the runtime would be behaviour no replay reproduces. Issue
30 is the push channel that removes it.
