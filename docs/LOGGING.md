# Logging

Logging is `tracing`, set up by the `api-trace` crate. Call `api_trace::init`
first thing in `main`, before anything worth logging happens.

## The rules

- **One event is one line.** Long values are escaped and cut, never wrapped.
- **Structured, concise, easy to skim.** Anything worth reading back goes in a
  `key=value` field rather than into prose. Messages are short and always the
  same words for the same event, so a log can be scanned down the left and
  grepped for. Nothing is logged that the reader could work out from the line
  above.
- **Every call to an external API is logged**, request and response both. A bug
  we cannot reproduce has to be fixed from the log alone.
- **A POST body is logged exactly as it is sent** — see the warning below.
- **Data served from the cache is logged too**, marked as cached. A log showing
  only the requests that went out would leave most of a session unaccounted
  for.

## The shape of a line

```
2026-08-07T18:42:03.401Z 6e917a6 INFO  api quote{symbol=SPY range=1D} -> request id=1 source=yahoo method=GET url=…
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
modules: `api` for anything crossing the network, `cache` for what the store
decides, `panic` and `trace` for logging's own problems. Do not invent others;
a module name is what tells you where to go and look.

Requests and responses come in pairs sharing an `id`, so they can be matched up
when several are in flight. `->` and `<-` mean only that: what left and what
came back. What we then make of a response — whether it parses, whether it
holds what we wanted — happened afterwards and gets its own line.

## A log file is currently a credential

**Request bodies are logged verbatim, and a Plaid body carries `client_id` and
`secret` in it.** That secret is account-wide rather than per-user: anyone
holding it can read every Item on the Plaid account. So a log file written by
this build must not be pasted into an issue, attached to a ticket, or sent
anywhere it would not be reasonable to send the credentials themselves.

It is here on purpose. A response tells you what the far end thought you asked
for; it never tells you what you actually sent, so a request carrying a field
we did not intend is invisible from the response half of the pair. That is the
failure mode that cannot be debugged after the fact, and while the Plaid
integration is being built it is the one worth paying for.

**What has to change before Production**, and neither has been done:

- Redact `secret` — and anything else credential-shaped — on the way into the
  line, rather than relying on nobody sharing the file.
- Decide what an access token counts as. It is not in a request body today
  because the server holds it, but `/item/public_token/exchange` answers with
  one, and that is a *response* body, which is also logged.

Everything above is Sandbox-only for now, where the credentials guard nothing
real. It stops being true the moment `PLAID_ENV` says otherwise.

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
const SOURCE: Source = Source::new("plaid").through(&[scrub::compact_json]);
```

Each rule is a function of the body and a `Context` — the source, the endpoint,
and whether this is the request or the response — and returns the body the next
rule sees. Rules compose because they are ordered, and they live on the source
so that adding one covers every call it makes.

`compact_json` is the only rule so far. Plaid pretty-prints its responses, and a
line is cut at 400 characters, so most of what we were keeping was indentation —
a balance response got as far as the second account's name. It squeezes
whitespace between tokens, leaves whitespace inside strings alone, and leaves a
body that does not start with `{` or `[` entirely alone, because an HTML error
page is at its most useful when it is least mangled.

Rules run on the way to the log and nowhere else. The body that gets parsed is
the one that came off the wire — a caller passes a body in by reference and
never gets one back, and `Scrub` cannot be run from outside `api-trace` at all.
That matters most for the rule we have not written yet: a redaction that leaked
into the request path would put the word `REDACTED` where a secret belonged.

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
- `INFO phone_main meridian phone starting version=0.1.0`
  — one per binary, each first thing in its own `main`.
- `INFO server_main::runtime_server store ready store=postgres`
  — once at startup. There is no second form: `DATABASE_URL` is required, and
  a server without a store has no access tokens and so no bank to reach.
- `WARN server_main::runtime_server EMAIL_SEND=log: sign-in codes are written to this log and no email is sent`
  — once at startup, and again on every sign-in below. A server running this
  way in front of anyone real is a mistake, so it says so rather than being
  quiet about it.
- `INFO server_main::runtime_server serving live plaid data env=sandbox collect=poll email=log`
  — once at startup. There is no second form of this line any more: the server
  starts with credentials or it does not start. Only ever Sandbox; see
  [PLAID.md](PLAID.md).
- `INFO server_main::runtime_server server listening port=8082`
- `INFO web_main::io::markets quote{symbol=SPY range=1D} quote asked for`
- `INFO src_plaid sandbox item created institution_id=ins_109508`
- `INFO src_plaid item created item_id=8Pd5Zx…`
  — linking a bank. The access token is deliberately absent from both: it is a
  credential, and a log is a file that gets pasted into issues.
- `INFO src_plaid::plaid_client plaid client env=sandbox base_url=https://sandbox.plaid.com/`
  — which environment a session ran against, logged once when the client is
  built. See [PLAID.md](PLAID.md).
- `INFO api -> request id=1 source=plaid method=POST url=https://sandbox.plaid.com/transactions/sync body="{\"access_token\":\"access-sandbox-1\",\"client_id\":\"6xxx\",\"cursor\":\"CAESJTM3…\",\"secret\":\"xxxx\"}"`
  — the body as sent, credentials included. See the warning above.
- `INFO api <- response id=1 source=plaid status=200 ms=284 body="{\"added\":[{\"account_id\":\"6Pd…"`
  — Plaid pretty-prints, and this is the same body with the whitespace taken
  back out. See [Rules a body passes through](#rules-a-body-passes-through).
- `INFO api -> request id=2 source=plaid method=POST url=https://sandbox.plaid.com/accounts/balance/get body="{\"access_token\":\"access-sandbox-1\",\"client_id\":\"6xxx\",\"secret\":\"xxxx\"}"`
- `INFO api -> request id=3 source=plaid method=POST url=https://sandbox.plaid.com/transactions/recurring/get body="{\"access_token\":\"access-sandbox-1\",\"client_id\":\"6xxx\",\"secret\":\"xxxx\"}"`
  — a GET carries no `body` field at all, rather than an empty one. Yahoo and
  CoinGecko lines below are the same shape as before.
- `INFO src_plaid transactions synced added=18 modified=0 removed=1`
  — a *diff*, not a total. A later sync answering `added=0` is normal and
  means nothing changed, not that the ledger is empty.
- `INFO src_postgres::postgres_migrate schema is current migrations=2`
  — on every boot. A no-op against a database that is already current, and a
  loud failure against one where an applied migration has been edited.
- `ERROR server_main::runtime_server asked to fetch an item with no token item_id=8Pd5Zx…`
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
- `WARN server_main::state_server a sign-in code was rejected email=alex@example.com verdict=Wrong`
  — `Wrong`, `Expired` or `Exhausted`. Which one is logged and deliberately not
  told to the caller: only somebody working through guesses benefits from
  knowing which.
- `WARN server_main::runtime_server a sign-in was asked for with something that is not an address`
- `INFO src_resend email sent id=… to=alex@example.com`
  — what a restart came back with, before anything is fetched. Zeroes are a
  first run, not a failure.
- `ERROR server_main::state_server could not read the store; starting empty message="connection refused"`
  — survivable. An empty ledger is what a first run has, and the next request
  fills it. Logged at ERROR because it means something is wrong even though
  nothing broke.
- `ERROR server_main::state_server could not write the store message=…`
  — what was fetched is still in memory and still being served. The cursor not
  having moved is what makes the next restart recover rather than skip.
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
- `WARN server_main::runtime_plaid recurring unavailable, continuing without it error="Plaid error PRODUCT_NOT_READY: …"`
  — recurring is derived from transactions, so a fresh Item has none yet. One
  thin screen rather than a failed sync.
- `WARN server_main::runtime_server plaid fetch failed code=ITEM_LOGIN_REQUIRED error="Plaid error ITEM_LOGIN_REQUIRED: …"`
  — the connection needs the user to sign in again, and no retry will fix it.
- `INFO phone_main::runtime_app snapshot asked for force=false`
  — the phone asking the server for everything. `force=true` is pull-to-refresh.
- `WARN phone_main::runtime_app load failed message="could not reach the server at http://127.0.0.1:8082 — is `cargo run -p server-main` running?"`
- `WARN phone_main::runtime_app first run is not drawn on the web action=StartBankLink`
  — first run is a phone flow for now, and the web answers `LoadSession` as
  already signed in so that it is never walked. A line here means that answer
  changed and the web reached a road it cannot draw.
- `INFO api quote{symbol=SPY range=1D} -> request id=2 source=yahoo method=GET url="/v8/finance/chart/SPY?period1=1785542400&period2=1785628800&interval=1m"`
- `INFO api quote{symbol=SPY range=1D} <- response id=2 source=yahoo status=200 ms=412 body="{\"chart\":{\"result\":[{\"meta\":{\"currency\":\"USD\",\"symbol\":\"SPY\"…"`
  — the body as it arrived, cut at 400 characters.
- `INFO cache quote{symbol=SPY range=1D} served symbol=SPY interval=Min1 window=1785542400..1785628800 bars=930 cached=false`
- `INFO cache quote{symbol=BTC-USD range=1D} source substituted an interval symbol=BTC-USD asked=Min1 served=Hour1`
- `WARN api quote{symbol=SPY range=1D} request failed id=3 source=yahoo ms=30012 error="Network error: operation timed out"`
  — nothing came back, so there is no response line.
- `ERROR api quote{symbol=SPY range=1D} [parse error id=4] source=yahoo error="missing field \`chart\`"`
  — the body is already on the `<- response` line for the same `id`.
- `DEBUG api quote{symbol=SPY range=1W} request cancelled id=5 source=yahoo ms=12`
  — the request was dropped before an answer, which is what switching range
  mid-fetch looks like from here.
- `WARN trace log file unavailable error="logs/meridian.log: permission denied"`
- `ERROR panic panicked at=crates/web-main/src/pages/markets.rs:112 message="…"`
- `INFO server_main::runtime_server shutting down` / `INFO server_main::runtime_server server stopped`
  — SIGINT becomes a `FromTerminal` event, so shutdown is a decision the state
  machine makes rather than something a signal handler does behind its back.
- `WARN server_main::runtime_server a connection needs the user to sign in again; syncs will fail until they do`
- `ERROR server_main::runtime_plaid a compiled-in fixture failed to parse fixture=sync error="…"`
  — a build-time mistake, logged rather than panicked on.

`cargo run -p api-trace --example lines` prints these for real.
