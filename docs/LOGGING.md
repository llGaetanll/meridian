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
- `INFO app quote{symbol=SPY range=1D} quote asked for`
- `INFO api quote{symbol=SPY range=1D} -> request id=1 source=yahoo method=GET url="/v8/finance/chart/SPY?period1=1785542400&period2=1785628800&interval=1m"`
- `INFO api quote{symbol=SPY range=1D} <- response id=1 source=yahoo status=200 ms=412 body="{\"chart\":{\"result\":[{\"meta\":{\"currency\":\"USD\",\"symbol\":\"SPY\"…"`
  — the body as it arrived, cut at 400 characters.
- `INFO cache quote{symbol=SPY range=1D} served symbol=SPY interval=Min1 window=1785542400..1785628800 bars=930 cached=false`
- `INFO cache quote{symbol=BTC-USD range=1D} source substituted an interval symbol=BTC-USD asked=Min1 served=Hour1`
- `WARN api quote{symbol=SPY range=1D} request failed id=2 source=yahoo ms=30012 error="Network error: operation timed out"`
  — nothing came back, so there is no response line.
- `ERROR api quote{symbol=SPY range=1D} [parse error id=3] source=yahoo error="missing field \`chart\`"`
  — the body is already on the `<- response` line for the same `id`.
- `DEBUG api quote{symbol=SPY range=1W} request cancelled id=4 source=yahoo ms=12`
  — the request was dropped before an answer, which is what switching range
  mid-fetch looks like from here.
- `WARN trace log file unavailable error="logs/meridian.log: permission denied"`
- `ERROR panic panicked at=src/web-main/src/pages/markets.rs:112 message="…"`

`cargo run -p api-trace --example lines` prints these for real.
