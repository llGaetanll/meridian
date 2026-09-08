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
