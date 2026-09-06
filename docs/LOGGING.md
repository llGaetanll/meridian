# Logging

Logging is `tracing`, set up by the `api-trace` crate. Call `api_trace::init`
first thing in `main`, before anything worth logging happens.

## The rules

- **Everything goes through `tracing`. Nothing prints.** No `println!`, no
  `eprintln!`, no hand-rolled logger — a print is a line with no level, no
  target, no fields and no commit. The exception is a command whose output *is*
  the answer.
- **One event is one line.** Long values are escaped and cut, never wrapped.
- **Structured, concise, easy to skim.** Anything worth reading back goes in a
  `key=value` field rather than into prose. Messages are short and always the
  same words for the same event, so a log can be scanned down the left and
  grepped for. Nothing is logged that the reader could work out from the line
  above.
- **Every call crossing the network is logged**, both halves of it. The ones
  we make out to Plaid and the price feeds, and the ones a phone makes of us.
  A bug we cannot reproduce has to be fixed from the log alone.
- **A POST body is logged as it is sent, minus its credentials.** The field
  names are all there; the value of a `secret` or an `access_token` is replaced
  by its length. See [Rules a body passes
  through](#rules-a-body-passes-through).
- **Data served from the cache is logged too**, marked as cached. A log showing
  only the requests that went out would leave most of a session unaccounted
  for.

## The shape of a line

```
2026-08-07T18:42:03.401Z 6e917a6 INFO  src_yahoo::yahoo_client quote{symbol=SPY range=1D} -> id=1 source=yahoo method=GET url=…
```

Timestamp (UTC), the commit the build came from, level, target, the spans the
event happened in, the message, then `key=value` fields.

The commit is read at build time by `api-trace`'s build script. A build that
cannot reach git — a container, whose context excludes `.git` on purpose —
takes it from `MERIDIAN_SHA` in the environment instead, and says `unknown` if
it has neither. **A deployed build must pass it**: a log that cannot be matched
to the source that produced it has lost most of what this line is for.
`scripts/dev.sh` passes it for every local build, suffixed `-dirty` when the
tree has changes the commit does not account for.

A line's target is the module it came from — `server_main::runtime_server`,
`web_main::io::markets` — unless it names one of the few that cut across
modules: `cache` for what the store decides, `panic` and `trace` for logging's
own problems. Do not invent others; a module name is what tells you where to go
and look.

`->` and `<-` lines are logged on a caller's behalf, inside `ApiCall` and
`Served`, so `tracing` reports them all as `api_trace::call` — it fixes a
target at the callsite, and the callsite is the helper. The caller passes its
own `module_path!()` in as a field instead and that is what prints, which is
why a Plaid request says `src_plaid::plaid_client` like everything else in the
file. The `api_trace` target underneath is left alone rather than replaced with
a made-up one: only a level filter sees it, and it is at least true.

Requests and responses come in pairs sharing an `id`, so they can be matched up
when several are in flight. `->` and `<-` mean only which way the traffic went:
`->` left this process, `<-` arrived at it. What we then make of a response —
whether it parses, whether it holds what we wanted — happened afterwards and
gets its own line.

So a call we make reads `->` then `<-` and names the far end with `source=`; a
call a client makes of us reads `<-` then `->` and names it with `client=`. The
first arrow and the field together say which kind of conversation a pair is,
and one counter mints both, so an `id` belongs to exactly one of them.

## Who the client is

`client=` is the socket's peer, until the server is behind a proxy — and then
the socket's peer is the proxy, and every line names it. Deployed, that made
every request read `client=172.18.0.4`, which is Caddy.

So when the peer is loopback or a private address, `X-Forwarded-For` is read
instead. Two things about that are worth knowing, because both are easy to get
backwards:

- **The last entry is taken, not the first.** Caddy appends what it saw to
  whatever the caller already sent, so the rightmost is our proxy's word and
  everything to its left is the caller describing itself.
- **A public peer's header is ignored.** Anyone can send `X-Forwarded-For`, so
  it is only believed from an address that could not have come through the
  proxy in the first place. Exposing the server directly therefore does not
  quietly turn a forgeable header into what callers are told apart by.

The address is parsed before it is logged. Unparsed it is attacker-controlled
text going into a line of `key=value` pairs, and a caller could forge fields on
the very line that names it.

**The address, never the port.** `client=` used to carry the socket's port when
there was no proxy in the way. It does not any more, because the rate limit
divides callers by exactly this field and a caller gets a fresh port on every
connection — a budget counted by address and port would be a budget per
request. One answer to "who is this", or the log and the limit are two rules
that can disagree.

**An IPv6 caller is named by their /64**, so those lines read
`client=2001:db8:1234:5678::/64` rather than an address. For the same reason
the port went: v6 delegates a whole /64 to a customer, and every address in it
is theirs to pick and change at will — the operating system already rotates
through them on a timer. An address is not a caller there; the prefix is. Over
v4, where a second address has to be bought, `client=` is still the address
whole.

## The requests clients make of us

The server logs every request it is asked and every answer it gives, in
`server_main::router_trace` — a layer rather than a line in each handler,
because a handler that forgets is the gap this closes, and because a 401, a 404
and a body that would not deserialize are all answered before any handler runs.
The preflight `OPTIONS` a browser sends is not logged: the layer sits inside
CORS, which answers those, and a line each would double the traffic in the log
without saying anything about it.

**Neither line carries a body**, which is the one place this half departs from
the other. A request body here is at most an address and a sign-in code, both
already logged by name where something is decided about them. A response body
is somebody's ledger, and the first 400 characters of it is enough to be
personal data and not enough to debug from — the server's own lines already say
what it decided to serve. `bytes` is what is worth knowing about an answer: a
snapshot that left as 300 bytes rather than 300 kilobytes is the bug.

**The phone writes nothing here.** The Swift and Kotlin clients do their HTTP
in platform code, and their half of the conversation is a log on a different
machine that we never see. That is most of why the server end of this matters:
it is the only end we have.

## A log file is not a credential, and is still personal data

**No Plaid credential reaches a line.** Request bodies are logged field for
field, and `redact_credentials` replaces the value of every credential-shaped
one with how many characters it took out:

```
body="{\"access_token\":\"[redacted 16]\",\"client_id\":\"[redacted 24]\",\"secret\":\"[redacted 30]\"}"
```

The field name stays because that is what the request half of a pair is for. A
response tells you what the far end thought you asked for; it never tells you
what you actually sent, so a request carrying a field we did not intend is
invisible from the response half. The *value* was never part of that question.

The length stays because it is not a secret and it is the one thing that tells
a wrong secret apart from an empty one — a misconfiguration no other line in
the log would show.

Six names are redacted, in requests and responses alike:

| Field | What it opens |
| --- | --- |
| `secret` | Every Item on the Plaid account. Account-wide, not per-user. |
| `client_id` | The other half of that pair. |
| `access_token` | One bank, permanently. Never expires. |
| `public_token` | Exchangeable for an access token, for thirty minutes. |
| `link_token` | A Link session belonging to one of our users. |
| `hosted_link_url` | The page carrying that token, so the URL is the token. |

**Still to do: a webhook body is now on an event.** `EventRouter::WebhookReceived`
carries the bytes as they arrived, so a `?events` line prints them. Plaid's
webhook bodies carry `item_id` and counts rather than credentials, so nothing on
the list above is at risk today — but `scrub` should learn the field before a
webhook type that carries more arrives.

The list is `CREDENTIALS` in `api_trace::scrub`, and it is Plaid's vocabulary
because Plaid is the only source that puts a credential in a body — a price
feed authenticates with nothing, and Resend with a header. **A source added
later brings its names to that list rather than writing its own rule.**

What a log file still holds is somebody's ledger, their email address, and —
under `EMAIL_SEND=log` — their sign-in code. It is personal data and should be
handled as such; it is no longer a key to the Plaid account.

## Where lines go

Native builds write `logs/meridian.log`, flushed line by line. The browser has
no file to append to, so the web build keeps the last 4096 lines in memory;
`wasmBindings.meridian_logs()` in the devtools console saves them.

Only the terminal gets colour, and only when stdout is a terminal that has not
set `NO_COLOR`. The file and the buffer are read back as text long afterwards,
and escape codes in them are noise.

Under docker that test is wrong: stdout is a pipe to the daemon, but the thing
reading the other end is `docker compose logs` in a terminal. `CLICOLOR_FORCE`
says so, and docker-compose.yml sets it. `NO_COLOR` still beats it.

## Rules a body passes through

A request or response body is logged as it crossed the network, because that is
the only version that can answer "what did we actually send". But it goes
through an ordered list of rules on the way — `api_trace::scrub` — and a source
picks the ones that apply to it:

```rust
const SOURCE: Source =
    Source::new("plaid").through(&[scrub::redact_credentials, scrub::compact_json]);
```

Each rule is a function of the body and a `Context` — the source, the endpoint,
and whether this is the request or the response — and returns the body the next
rule sees. Rules compose because they are ordered, and they live on the source
so that adding one covers every call it makes.

There are two rules, and Plaid runs both.

`redact_credentials` is the one that matters, and it goes first so that no rule
after it is handed a secret — see [A log file is not a
credential](#a-log-file-is-not-a-credential-and-is-still-personal-data) for
which fields it takes and why the length stays. It walks the text rather than
parsing, for the reason below and one of its own: a body that does not parse is
exactly when a rule that gave up would hand the log a secret. Anything it
cannot read to the end of — a response cut mid-flight — it redacts to the end
of the body rather than to where the reading stopped.

`compact_json` runs after it. Plaid pretty-prints its responses, and a line is
cut at 400 characters, so most of what we were keeping was indentation — a
balance response got as far as the second account's name. It squeezes
whitespace between tokens, leaves whitespace inside strings alone, and leaves a
body that does not start with `{` or `[` entirely alone, because an HTML error
page is at its most useful when it is least mangled.

Rules run on the way to the log and nowhere else. The body that gets parsed is
the one that came off the wire — a caller passes a body in by reference and
never gets one back, and `Scrub` cannot be run from outside `api-trace` at all.
That matters most for redaction: a rule that leaked into the request path would
send Plaid `[redacted 30]` where the secret belonged.

**A rule may not change what a body means.** Reformatting says the same thing in
less space; redaction — the next rule, and why this seam exists at all — says
plainly that something was removed. A rule that quietly rewrote a value would
leave a log that is worse than no log, because it would still be believed.

Level is INFO by default. `DEBUG` adds cancelled requests.

A few targets are held to `WARN` whatever the level says — `Config::quiet`.
They are libraries that narrate their own housekeeping at `INFO`, like sqlx
announcing that its migrations table already exists on every single boot. Every
line in the list below is one somebody chose to write, and a dependency talking
over them is how a log becomes something people skim past. Held rather than
silenced, so a genuine warning from one still arrives.

## Every line we log

Keep this list in step with the code. Adding, changing or removing a log line
means adding, changing or removing its example here.

- `INFO web_main meridian starting version=0.1.0`
- `INFO server_main meridian server starting version=0.1.0`
  — one per binary, each first thing in its own `main`.
- `INFO server_main::runtime_server store ready store=postgres`
  — once at startup. There is no second form: `DATABASE_URL` is required, and
  a server without a store has no access tokens and so no bank to reach.
- `WARN server_main::runtime_server EMAIL_SEND=log: sign-in codes are written to this log and no email is sent`
  — once at startup, and again on every sign-in below. A server running this
  way in front of anyone real is a mistake, so it says so rather than being
  quiet about it.
- `INFO server_main::runtime_server serving live plaid data env=sandbox collect=poll webhook=none email=log origins=http://127.0.0.1:8080`
  — once at startup. There is no second form of this line any more: the server
  starts with credentials or it does not start. Only ever Sandbox; see
  [PLAID.md](PLAID.md). `webhook` is the address Plaid is told to call, and
  `none` is `PLAID_COLLECT=poll`, where there is nothing to tell it.
- `INFO server_main::runtime_server server listening port=8082`
- `INFO server_main::router_trace <- id=7 client=192.168.1.24 method=GET url="/snapshot"`
- `INFO server_main::router_trace -> id=7 client=192.168.1.24 status=200 ms=41 bytes=91233`
  — a client's request and the answer it got, one pair per request the server
  is asked. `client` is whoever the socket says, which is what tells the web
  front-end on loopback apart from a phone on the LAN. Behind a proxy the
  socket is the proxy, so there it is the address Caddy reports instead — see
  [Who the client is](#who-the-client-is). No bodies; see
  [The requests clients make of us](#the-requests-clients-make-of-us).

  `/sync/status` reads oddly here and is right: the pair is written when the
  headers go out, so a stream that stays open for a whole sync shows `ms` in
  single figures and `bytes=0`, and everything written to it afterwards
  appears under no request line at all.
- `DEBUG server_main::router_trace answered nothing id=7 client=192.168.1.24 ms=12`
  — nothing went back: the client hung up mid-request, which is what closing
  the app during a refresh looks like from here. `DEBUG` because it is
  ordinary, logged at all because a request line with nothing under it reads
  like a hang.
- `INFO web_main::io::markets quote{symbol=SPY range=1D} quote asked for`
- `INFO src_plaid link session opened update=false hosted=true expires=2026-08-29T01:57:48Z`
- `DEBUG src_plaid link session polled sessions=1 finished=true tokens=1`
- `INFO server_main::runtime_server bank linked item_id=8Pd5Zx… user_id=e8b13a2b…`
- `WARN server_main::runtime_server could not open a link session error="Plaid error INVALID_FIELD: …"`
- `ERROR server_main::runtime_server a bank was linked against no session link_token=link-sandbox-…`
  — the Item exists at Plaid's end and we cannot say whose it is. Not handled:
  guessing an owner is the one mistake in this file that would put somebody
  else's transactions on your screen.
- `INFO src_plaid sandbox item created institution_id=ins_109508`
- `INFO src_plaid item created item_id=8Pd5Zx…`
  — linking a bank. The access token is deliberately absent from both: it is a
  credential, and a log is a file that gets pasted into issues.
- `INFO src_plaid::plaid_client plaid client env=sandbox base_url=https://sandbox.plaid.com/`
  — which environment a session ran against, logged once when the client is
  built. See [PLAID.md](PLAID.md).
- `INFO src_plaid::plaid_client -> id=1 source=plaid method=POST url=https://sandbox.plaid.com/transactions/sync body="{\"access_token\":\"[redacted 16]\",\"client_id\":\"[redacted 24]\",\"cursor\":\"CAESJTM3…\",\"secret\":\"[redacted 30]\"}"`
  — the body as sent, field for field, with the credentials replaced by their
  length. See [Rules a body passes through](#rules-a-body-passes-through).
- `INFO src_plaid::plaid_client <- id=1 source=plaid status=200 ms=284 body="{\"added\":[{\"account_id\":\"6Pd…"`
  — Plaid pretty-prints, and this is the same body with the whitespace taken
  back out. See [Rules a body passes through](#rules-a-body-passes-through).
- `INFO src_plaid::plaid_client -> id=2 source=plaid method=POST url=https://sandbox.plaid.com/accounts/balance/get body="{\"access_token\":\"[redacted 16]\",\"client_id\":\"[redacted 24]\",\"secret\":\"[redacted 30]\"}"`
- `INFO src_plaid::plaid_client -> id=3 source=plaid method=POST url=https://sandbox.plaid.com/transactions/recurring/get body="{\"access_token\":\"[redacted 16]\",\"client_id\":\"[redacted 24]\",\"secret\":\"[redacted 30]\"}"`
  — a GET carries no `body` field at all, rather than an empty one. Yahoo and
  CoinGecko lines below are the same shape as before.
- `INFO src_plaid transactions synced added=18 modified=0 removed=1 more=false`
  — one page, and a *diff* rather than a total. A later sync answering
  `added=0` is normal and means nothing changed, not that the ledger is empty.
  `more=true` means another page is waiting, which the machine asks for.
- `INFO src_postgres::postgres_migrate schema is current migrations=2`
  — on every boot. A no-op against a database that is already current, and a
  loud failure against one where an applied migration has been edited.
- `ERROR server_main::runtime_server asked to call plaid about an item with no token item_id=8Pd5Zx…`
  — the call is never made, and what reaches the machine says so: `NO_ACCESS_TOKEN`
  is ours and not a code Plaid has.
- `ERROR server_main::runtime_server a sync landed for an item with no token item_id=8Pd5Zx…`
  — the machine named an Item the runtime holds no access token for, which
  means its connection list and the store have drifted. Neither is handled on
  purpose: a plausible-looking fallback would hide the disagreement instead of
  reporting it.

  Two older lines of this shape are gone, because their cases are: being asked
  to use a store that is not there, and storing an Item with no credentials.
  Both the store and the credentials are required now.
- `WARN server_main::runtime_server EMAIL_SEND=log: no email sent, the code is on this line user_id=e8b13a2b… email=alex@example.com code=704924`
- `INFO server_main::runtime_server signed in user_id=e8b13a2b…`
- `ERROR server_main::state_server asked for a sign-in code before the machine was seeded`
  — the machine draws codes and tokens from a generator seeded at the edge,
  and mints neither before one arrives. There is no fallback: a predictable
  sign-in code is a way into somebody's bank.
- `WARN server_main::state_server a caller is over its rate limit client=203.0.113.7 route=/auth/request`
  — the machine refused a request for being one too many, and the route got a
  429. `WARN` rather than `INFO`: one line is somebody double-tapping, and a
  page of them is worth looking at. The caller is told none of this beyond the
  status — see `ActionRouter::TooMany`.
- `WARN server_main::state_server a caller is holding too many streams open user_id=e8b13a2b… open=10`
  — the other limit on `/sync/status`, and the one about how many are open at
  once rather than how fast they are asked for. Also a 429. A run of these is
  a client that is not closing what it opens, since a stream going away frees
  its slot in the turn it is reported.
- `WARN server_main::state_server a sign-in code was rejected email=alex@example.com verdict=Wrong`
  — `Wrong`, `Expired` or `Exhausted`. Which one is logged and deliberately not
  told to the caller: only somebody working through guesses benefits from
  knowing which.
- `WARN server_main::state_server a sign-in was asked for with something that is not an address`
  — nothing was stored and nothing was sent, and the route still answered 204.
  Saying otherwise would tell whoever asked which strings this server writes
  to, which is the leak the route's one answer exists to prevent.
- `INFO src_resend email sent id=… to=alex@example.com`
  — what a restart came back with, before anything is fetched. Zeroes are a
  first run, not a failure.
- `ERROR server_main::state_server could not read the store message=…`
  — a read the machine asked for came back with nothing it can use. The route
  is told it cannot be answered rather than refused: a sign-in turned down
  because the store was down would look exactly like a wrong code.
- `ERROR server_main::state_server the store answered banks nobody asked about call_id=1`
- `ERROR server_main::state_server the store answered sessions nobody asked about call_id=1`
  — the same shape as the two below, for the reads `/snapshot` and
  `/link/finish` ask for. All four cannot happen: a request is written down
  when its read goes out.
- `ERROR server_main::state_server the store answered a session nobody asked about call_id=1`
  — a session read answering a call the machine has no request for, which
  cannot happen: every one is recorded when the read is asked for. Nothing is
  answered, because there is no route to answer.
- `ERROR server_main::state_server the store answered a sign-in nobody submitted call_id=1`
  — a read answering a call the machine has no submission for, which cannot
  happen: every one is recorded when the read is asked for. Reported rather
  than guessed at, because there is nothing to check the code against.
- `ERROR server_main::state_server could not write the store message=…`
  — the route waiting on it is answered anyway and serves the rows we already
  had, because a request held for a write that failed is held forever. The
  cursor not having moved is what makes the next sync recover rather than skip.
- `DEBUG server_main::runtime_server written to the store item_id=8Pd5Zx… added=48 modified=0 removed=0`
  — one per sync. `DEBUG` because a healthy server writes on every refresh.
- `INFO src_postgres::postgres_item item removed with its history item_id=8Pd5Zx…`
  — the accounts, transactions and streams went with it, by the cascade.
- `WARN src_postgres::postgres_ledger entries that are not transfers were not stored skipped=1`
  — the ledger table holds one movement per row and a trade has two. Nothing
  produces trades today; the line is here so the day something does, it is a
  message rather than a mystery.
- `WARN src_postgres::postgres_ledger a timestamp fell outside postgres seconds=…`
  — one row stored at the epoch rather than a whole sync failing over a date
  no bank feed should have produced.
- `INFO src_plaid link session opened update=false hosted=true expires=2026-08-26T10:30:00Z`
  — a bank being attached, or `update=true` for one being repaired. Nothing is
  created yet: a link session nobody finishes costs nothing.
- `DEBUG src_plaid link session polled sessions=1 finished=true tokens=1`
  — one line per poll of `/link/token/get`, which is how the public token is
  collected without a public webhook URL. `DEBUG` because a single link is
  several of these.
- `INFO src_plaid item created item_id=8Pd5Zx…`
  — **the line that says a Production Item slot was spent**, and there are ten.
  The access token is deliberately not on it: it is a credential to somebody's
  bank, and a log is a file that gets pasted into issues.
- `INFO server_main::state_server a webhook arrived from plaid webhook_type=LINK webhook_code=SESSION_FINISHED`
  — one per webhook that was *believed*. Written after the signature, the
  replay window and the body hash have all passed, so a line here is Plaid and
  the line below is not.
- `WARN server_main::state_server a webhook was not believed rejection="the signature does not match"`
  — the other endings are `no Plaid-Verification header`, `algorithm is
  \`HS256\`, not ES256`, `issued 3600s away from now` and `the body does not
  match its hash`. All of them answer the caller the same 401: the reason is
  worth logging and is not worth telling somebody who is guessing. A `body does
  not match` is far more likely to be ours than theirs — Plaid hashes the bytes
  it sent, so anything that reparsed the body breaks it.
- `WARN server_main::state_server a webhook named a key plaid does not have kid=6c5516e1…`
  — refused without asking Plaid again. The miss stands for
  `WEBHOOK_KEY_MISS_GOOD_FOR_SECS`, which is what stops an invented `kid` from
  costing an outbound call per POST.
- `ERROR server_main::state_server a key answered a webhook nobody is waiting on call_id=1`
- `ERROR server_main::state_server the store answered a bank nobody asked about call_id=1`
  — the same shape as the other four of these, and impossible for the same
  reason: what a request was doing is written down when its read goes out.
- `INFO src_plaid webhook key fetched key_id=6c5516e1…`
  — one per `kid`, ever, because the answer is cached. Several of these for one
  id means the cache is not doing its job, which matters: this call is caused by
  an anonymous POST.
- `WARN server_main::runtime_server could not fetch a webhook key kid=6c5516e1… error="Plaid error INVALID_FIELD: …"`
  — remembered as a miss for five minutes, so a made-up `kid` costs one call
  rather than one per POST.
- `ERROR server_main::state_server refusing to fetch another webhook key: too many have been asked for kid=6c5516e1…`
  — the cache is full of live keys, which cannot happen from Plaid's own
  rotation. Refused rather than fetched: the cache exists to bound this call,
  so it bounds it when full too.
- `ERROR server_main::state_server plaid signed a body that is not JSON error=…`
  — the hash matched, so these really are Plaid's bytes, and they are still not
  something we can read.
- `ERROR server_main::runtime_server could not read the bank a webhook names item_id=8Pd5Zx… error=…`
- `WARN server_main::state_server plaid has news about a bank we do not hold item_id=8Pd5Zx…`
  — Plaid is still sending about an Item whose row has gone. Nothing to sync
  against, and the webhook is answered 2xx anyway, because anything else means
  it arrives again in a minute.
- `INFO server_main::state_server plaid has not said what a link session came to yet request_id=3`
  — `/link/finish` waited ten seconds and gave up, and the caller is told
  `pending` rather than "no bank". A run of these means webhooks are arriving
  late or not at all, which is the one thing that would otherwise look like
  everybody abandoning their sign-in.
- `DEBUG server_main::state_server a webhook nothing acts on webhook_type=ASSETS webhook_code=PRODUCT_READY`
  — taken and ignored. Plaid sends types nobody planned for; `DEBUG` because a
  healthy server sees them routinely.
- `WARN server_main::state_server recurring unavailable, the sync stands without it item_id=8Pd5Zx… code=PRODUCT_NOT_READY`
  — recurring is derived from transactions, so a fresh Item has none yet. One
  thin screen rather than a failed sync.
- `WARN server_main::state_server out of budget part-way through a history; the next sync resumes item_id=8Pd5Zx…`
  — a first history is several pages and Plaid meters them per minute. The
  cursor of the last page written is where the next sync starts, so this costs
  a delay and no transactions.
- `WARN server_main::runtime_server a plaid call failed item_id=8Pd5Zx… stage=accounts code=ITEM_LOGIN_REQUIRED error="Plaid error ITEM_LOGIN_REQUIRED: …"`
  — `stage` says which of a sync's three calls it was, because what a failure
  costs depends on that. `ITEM_LOGIN_REQUIRED` needs the user to sign in again
  and no retry will fix it.
- `INFO src_yahoo::yahoo_client quote{symbol=SPY range=1D} -> id=2 source=yahoo method=GET url="/v8/finance/chart/SPY?period1=1785542400&period2=1785628800&interval=1m"`
- `INFO src_yahoo::yahoo_client quote{symbol=SPY range=1D} <- id=2 source=yahoo status=200 ms=412 body="{\"chart\":{\"result\":[{\"meta\":{\"currency\":\"USD\",\"symbol\":\"SPY\"…"`
  — the body as it arrived, cut at 400 characters.
- `INFO cache quote{symbol=SPY range=1D} served symbol=SPY interval=Min1 window=1785542400..1785628800 bars=930 cached=false`
- `INFO cache quote{symbol=BTC-USD range=1D} source substituted an interval symbol=BTC-USD asked=Min1 served=Hour1`
- `WARN src_yahoo::yahoo_client quote{symbol=SPY range=1D} request failed id=3 source=yahoo ms=30012 error="Network error: operation timed out"`
  — nothing came back, so there is no response line.
- `ERROR src_yahoo::yahoo_client quote{symbol=SPY range=1D} [parse error id=4] source=yahoo error="missing field \`chart\`"`
  — the body is already on the `<-` line for the same `id`.
- `DEBUG src_yahoo::yahoo_client quote{symbol=SPY range=1W} request cancelled id=5 source=yahoo ms=12`
  — the request was dropped before an answer, which is what switching range
  mid-fetch looks like from here.
- `DEBUG server_main::runtime_server turn events=[FromRouter { request_id: 3, event: SnapshotRequested { .. } }] actions=[ToSource { action: FetchBalances { call_id: 1, .. } }]`
  — one per turn of the loop, both vectors paired. A quiet server logs none of
  these: a turn with nothing queued does not happen.
- `WARN server_main::state_server a call took too long and was given up on call_id=7 request_id=3`
  — a source went quiet rather than failing, so no event ever came. The route
  waiting is answered with what the store already holds, because a request
  held for an answer that is not coming is held forever.
- `ERROR server_main::state_server a source answered a call nobody is waiting on call_id=7`
  — an answer to a question we have no record of asking, which cannot happen
  while every call is minted in the machine. Dropped rather than guessed at:
  there is no route to send it to and picking one would hand somebody another
  person's bank.
- `ERROR server_main::router_exchange a route asked something nothing answered request_id=3`
  — every router event owes exactly one action back, so this is a machine that
  went quiet on a request rather than an outcome.
- `DEBUG server_main::router_exchange a route hung up before its answer arrived request_id=3`
  — an ordinary dropped connection.
- `DEBUG server_main::runtime_router pushed to a stream that is not open conn_id=4`
  — a sync moved and what was watching it had gone. Ordinary: the connection
  and the event saying so cross, and the machine finds out one turn later.
- `ERROR server_main::runtime_server the router is gone; a push was dropped conn_id=4`
  — the same as the line above it for an answer, on the other half of the
  channel. Only reachable while shutting down.
- `ERROR server_main::source_pool a call to a source did not finish source=plaid error="task panicked"`
  — the call itself panicked, so whatever was waiting on its answer still is.
  This line is the only trace of why.
- `ERROR server_main::source_pool a source pool is gone; its call was dropped source=plaid`
  — the pool's task ended while the server is still serving, which should not
  happen while the process is up.
- `WARN trace log file unavailable error="logs/meridian.log: permission denied"`
- `ERROR panic panicked at=crates/web-main/src/pages/markets.rs:112 message="…"`
- `INFO server_main::state_server shutting down` / `INFO server_main::state_server stopped` / `INFO server_main::runtime_server server stopped`
  — three lines and three moments. SIGINT becomes a `FromTerminal` event, so
  the decision to stop is the machine's rather than something a signal handler
  does behind its back; `stopped` is the machine saying nothing is outstanding
  any more, which is what ends the loop; and the last is the loop having ended.
  The gap between the second and the first is whatever was still in flight —
  bounded by `CALL_GOOD_FOR_SECS`, because a source that goes quiet is given
  up on rather than waited for.
- `INFO server_main::runtime_server swept expired sessions sessions=3`
  — the hourly sweep the machine asks for, logged only when it dropped
  something. Most hours it drops nothing and says nothing.
- `WARN server_main::runtime_server a connection needs the user to sign in again; syncs will fail until they do`
- `ERROR server_main::runtime_plaid a compiled-in fixture failed to parse fixture=sync error="…"`
  — a build-time mistake, logged rather than panicked on.

`cargo run -p api-trace --example lines` prints these for real.
