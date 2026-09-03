# The database

Postgres, in a container, reached through one crate. `src-postgres` is the only
place in the workspace that speaks SQL — every statement is a function in
there, so a query cannot be written at a call site and nothing else has to know
what the schema looks like.

## Running it

```sh
cp .env.example .env         # once — DATABASE_URL lives here
./scripts/db-init.sh         # brings Postgres up and waits until it answers
cargo run -p server-main     # applies migrations, then serves
```

`cargo test` needs the same two lines, and nothing else.

Or the whole thing as it is deployed, in one command:

```sh
docker compose --profile server up
```

The split is a development convenience. `cargo run` rebuilds in about three
seconds where an image rebuild is closer to twenty, so a database container and
a server on the host is what you want while writing code — but the server
container is the real target and is meant to stay working.

**`DATABASE_URL` is required.** Not as policy — structurally: the Plaid access
tokens live in `items`, one row per bank, so a server without a database has no
credential to reach any bank with and nothing to serve.

It used to be optional, holding state in memory and losing it on restart. That
went at the same time as the fixtures, and for the same reason: an empty screen
should have one explanation.

## Tests

Its queries are tested against a real Postgres, and they run as part of
`cargo test` — no flag, no opt-in. A test that has to be asked for is a test
that rots.

`#[sqlx::test]` gives each one its **own database**: it creates one, runs the
migrator into it, hands the test a pool and drops it afterwards. So they are
isolated from each other, and from the database you work in — **nothing a test
does can reach a real access token.** A failing test leaves its database behind
on purpose so you can go and look; sqlx sweeps those on the next run, and they
show up as `_sqlx_test_*` in `\l` meanwhile.

The isolation is at the database level, not the server: one container holds
both, so `docker compose down -v` still takes everything.

What this costs is that **`cargo test` needs Postgres running**. That is worth
separating from two properties it does not touch:

| | needs a database |
| --- | --- |
| Running the app | **yes** — the access tokens are in it |
| `cargo build` | no — `.sqlx` holds every query's checked metadata |
| `cargo test` | yes, one `./scripts/db-init.sh` away |

An *app* that cannot start without a database would be a problem. A *test
suite* that needs one is ordinary, and is the price of tests that run.

## Checked queries

Every statement goes through `sqlx::query!`, which hands the SQL to Postgres at
compile time and asks it to describe it — so a typo, a column that does not
exist, or a migration that dropped one out from under a query is a build error
rather than something found at run time.

That needs a database at build time, or the `.sqlx` cache at the repo root,
which is committed. Regenerate it whenever a query changes:

```sh
cargo sqlx prepare --workspace
```

It is on the pre-commit list in [STYLE.md](STYLE.md). Forgetting it gives a
build that works on your machine and nowhere else.

`DATABASE_URL` comes from `.env`, which sqlx reads for all three of the build
macros, `#[sqlx::test]` and `cargo sqlx prepare`. So `cp .env.example .env` is
part of setting up, and it is the only place that URL is written down.

`./scripts/db-shell.sh` opens `psql`. `\dt` lists the tables and
`select * from _sqlx_migrations;` says which migrations this database has
actually run — the first thing worth checking when the schema is not what you
expected.

## Migrations

Files in `crates/src-postgres/migrations`, numbered, applied in order. They are
compiled into the binary and run on boot, which is what makes bringing the
whole thing up one command rather than a sequence somebody has to remember.

**A migration that has been applied anywhere is immutable.** This is the only
rule that matters, and everything else follows from it:

- Never edit a migration that has run. Not to fix a typo, not to add a column.
- Fix a mistake with the *next* migration. Being wrong repeatedly, in order, on
  the record, is what the numbered directory is for.
- Forward-only. Down migrations sound prudent and are almost never used in
  anger; when something has to be undone in production you write a migration
  that undoes it, and then you can see exactly what it did.

sqlx enforces the rule rather than trusting anyone to follow it: it records
every applied file in a `_sqlx_migrations` table with a checksum, and refuses
to start against a database where one has changed. That failure is the feature.
There is no way to know which databases already ran the old version of a file,
so the only safe move is to refuse.

The schema will be wrong. That is expected and is not a problem — it is a
problem only if it gets edited in place.

## The one column that is not what it looks like

`items.access_token` is `BYTEA`, and what is in it is not the token. Plaid's
access token never expires and reads somebody's bank account, so it is sealed
with AES-256-GCM before it goes in and opened on the way out. The blob is
`version || nonce || ciphertext || tag`, and the version byte is there so a
future key or algorithm can tell what it is holding instead of guessing.

`MERIDIAN_DB_KEY` is the key: 32 random bytes, base64, required whenever
`DATABASE_URL` is, and with no default — a default key is a key that ships.
`scripts/db-init.sh` writes one into `.env` if there is not one already, and
never replaces an existing one.

Only `save_item` and `load_items` know about any of this. `StoredItem` carries
a plain `String` and every caller above the store works with the token as it
is, because a caller that had to seal and open it itself is a caller that can
forget to.

**What it buys:** a stolen volume, a leaked backup, a `pg_dump` pasted into a
ticket, a replica, anyone with database credentials but not the server. That is
how this data actually escapes. It is no protection at all against someone who
has the server, because the server has the key.

**What it costs:** lose the key and every access token is scrap. Every bank has
to be relinked, and in Production relinking spends Item slots that do not come
back. Back the key up as carefully as the database, and not in the same place —
one theft should not get both.

Not the ledger. Encrypting transactions at the column level would make them
permanently unqueryable in SQL — no date ranges, no aggregation, no indexes —
to duplicate what an encrypted volume already does.

## Resetting

`./scripts/db-reset.sh` destroys the database and builds an empty one. **Every
row, permanently, with no backup.** It exists because a database is the one
part of this project that remembers what you did to it last week, and a
migration you cannot get out of is a worse place to be than an empty database.

It has two guards, in the spirit of `PlaidEnv` defaulting to Sandbox — the
harmless thing happens when you are not paying attention, and the destructive
one takes saying so out loud:

1. It refuses to run against any host but this machine.
2. It makes you type the database name back.

Run it yourself, from a terminal, having decided to. Not from a script, a hook,
or an agent.

## Why Postgres, and why the version is pinned

SQLite would be simpler for one user on one laptop, and it is the wrong answer
the moment the destination is a container on a server: one writer, no network,
and a volume to keep track of. Running the same engine in development and in
production is worth more than the simplicity.

`postgres:17-alpine` is pinned to a major version on purpose. Postgres does not
upgrade its data directory across majors, so changing that tag gives a
container that refuses to start against the existing volume. Moving to 18 is a
dump and a restore, deliberately, not an edit to one line.

## Where a query sits in the event loop

A query is IO, so it is an action out and an event back, the same as a trip to
Plaid — see [ARCHITECTURE.md](ARCHITECTURE.md).

What it is *not* is a lookup on the request path. The server holds its ledger in
memory and answers `/snapshot` from what it holds, synchronously. Postgres is
where that survives a restart:

- **Writes are actions.** A sync lands, and the machine asks for it to be saved.
- **Reads are one action at boot**, answered by an event that fills the machine.

If every request had to go and ask the database, the machine would stop being a
synchronous function of what it holds and become an async orchestrator — and
"replay the events, get the same actions" would go with it. That property is
the reason the whole architecture is shaped the way it is.

This holds until one process cannot hold every user, which is a long way off.
The answer then is loading a user's ledger on demand and evicting it, not
querying per request.
