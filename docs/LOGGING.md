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
- **Data served from the cache is logged too**, marked as cached. A log showing
  only the requests that went out would leave most of a session unaccounted
  for.

## The shape of a line

```
2026-08-07T18:42:03.401Z 6e917a6 INFO  api quote{symbol=SPY range=1D} -> request id=1 source=yahoo method=GET url=…
```

Timestamp (UTC), the commit the build came from, level, target, the spans the
event happened in, the message, then `key=value` fields.

Targets are `api` for anything crossing the network, `cache` for what the store
decides, `app` for the app itself, `panic` and `trace` for logging's own
problems.

Requests and responses come in pairs sharing an `id`, so they can be matched up
when several are in flight. `->` and `<-` mean only that: what left and what
came back. What we then make of a response — whether it parses, whether it
holds what we wanted — happened afterwards and gets its own line.

## Where lines go

Native builds write `logs/meridian.log`, flushed line by line. The browser has
no file to append to, so the web build keeps the last 4096 lines in memory;
`wasmBindings.meridian_logs()` in the devtools console saves them.

Level is INFO by default. `DEBUG` adds cancelled requests.

## Every line we log

Keep this list in step with the code. Adding, changing or removing a log line
means adding, changing or removing its example here.

- `INFO app meridian starting version=0.1.0`
- `INFO app meridian server starting version=0.1.0`
- `INFO app meridian phone starting version=0.1.0`
  — one per binary, each first thing in its own `main`.
- `INFO app serving live sandbox data source=plaid`
- `INFO app no PLAID_ACCESS_TOKEN, serving captured sandbox fixtures source=fixtures`
  — which of the two the server is answering from, logged once at startup.
  Only ever Sandbox; see [PLAID.md](PLAID.md).
- `INFO app server listening port=8082`
- `INFO app quote{symbol=SPY range=1D} quote asked for`
- `INFO app plaid client env=sandbox base_url="https://sandbox.plaid.com/"`
  — which environment a session ran against, logged once when the client is
  built. See [PLAID.md](PLAID.md).
- `INFO api -> request id=1 source=plaid method=POST url="https://sandbox.plaid.com/transactions/sync"`
  — the body is never logged: it carries the Plaid secret.
- `INFO api <- response id=1 source=plaid status=200 ms=284 body="{\"added\":[{\"account_id\":\"6Pd…"`
- `INFO api -> request id=2 source=plaid method=POST url="https://sandbox.plaid.com/accounts/balance/get"`
- `INFO api -> request id=3 source=plaid method=POST url="https://sandbox.plaid.com/transactions/recurring/get"`
- `INFO app transactions synced added=18 modified=0 removed=1`
  — a *diff*, not a total. A later sync answering `added=0` is normal and
  means nothing changed, not that the ledger is empty.
- `WARN app recurring unavailable, continuing without it error="Plaid error PRODUCT_NOT_READY: …"`
  — recurring is derived from transactions, so a fresh Item has none yet. One
  thin screen rather than a failed sync.
- `WARN app plaid fetch failed code=ITEM_LOGIN_REQUIRED error="Plaid error ITEM_LOGIN_REQUIRED: …"`
  — the connection needs the user to sign in again, and no retry will fix it.
- `INFO app snapshot asked for force=false`
  — the phone asking the server for everything. `force=true` is pull-to-refresh.
- `WARN app load failed message="could not reach the server at http://127.0.0.1:8082 — is `cargo run -p server-main` running?"`
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
- `INFO app shutting down` / `INFO app server stopped`
  — SIGINT becomes a `FromTerminal` event, so shutdown is a decision the state
  machine makes rather than something a signal handler does behind its back.
- `WARN app a connection needs the user to sign in again; syncs will fail until they do`
- `ERROR app a compiled-in fixture failed to parse fixture=sync error="…"`
  — a build-time mistake, logged rather than panicked on.

`cargo run -p api-trace --example lines` prints these for real.
