# The database

Postgres, in a container, reached through one crate. `src-postgres` is the only
place in the workspace that speaks SQL — every statement is a function in
there, so a query cannot be written at a call site and nothing else has to know
what the schema looks like.

## Running it

```sh
cp .env.example .env         # once
./scripts/db-init.sh         # brings Postgres up and waits until it answers
cargo run -p server-main     # applies migrations, then serves
```

Or the whole thing as it is deployed, in one command:

```sh
docker compose --profile server up
```

The split is a development convenience. `cargo run` rebuilds in about three
seconds where an image rebuild is closer to twenty, so a database container and
a server on the host is what you want while writing code — but the server
container is the real target and is meant to stay working.

**Nothing here is required to run the app.** With no `DATABASE_URL` the server
holds its state in memory exactly as it always has, and with no Plaid
credentials it serves the captured fixtures. A clean checkout still runs with
no Docker at all. A `DATABASE_URL` that is set and *wrong* is fatal, though:
falling back would give a server that looks healthy while quietly keeping
nothing.

Its queries have tests that run against a real database, ignored by default so
that `cargo test` still passes with no Docker:

```sh
cargo test -p src-postgres -- --ignored
```

They share the one database and `load_entries` is deliberately global, so they
take a lock and start from empty. Note that cargo runs each *test binary* as
its own process in parallel, which the lock does not reach — so a test in one
file must not assert on how many rows are in a table another file writes to.

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
