#!/bin/sh
# Brings the database up and waits until it is actually answering.
#
# Safe to run any number of times: it starts the container if it is not
# running, does nothing if it is, and never touches what is in it. The one to
# run after a fresh clone, or when `cargo run -p server-main` says it cannot
# reach Postgres.
#
# It does not apply migrations. The server does that as it boots — see
# `src_postgres::migrate` — so there is one place that decides what the schema
# should be rather than two that can disagree.

set -e

cd "$(dirname "$0")/.."

# Defaults match .env.example, so this works before anyone has written a .env.
[ -f .env ] && . ./.env
DB_USER=${POSTGRES_USER:-meridian}
DB_NAME=${POSTGRES_DB:-meridian}

# The key the access token column is sealed with, if `.env` has not got one.
#
# Written rather than defaulted: a default key is a key that ships, and one
# invented per run would open nothing the last run wrote. Generated here
# because this is the script somebody runs once after a clone, which is exactly
# when a key should come into existence.
#
# Only ever added, never replaced. Overwriting it would turn every stored token
# into scrap, and the message says so rather than making that easy to do by
# accident.
if [ -f .env ] && ! grep -q '^MERIDIAN_DB_KEY=.' .env; then
    key=$(openssl rand -base64 32)

    # `.env.example` carries the empty line, so a clone has the key commented
    # into place already; this fills it in where it is rather than appending a
    # second one, which `.` would then read last and silently prefer.
    if grep -q '^MERIDIAN_DB_KEY=' .env; then
        # A literal `/` and `+` are base64's alphabet, so `|` is the delimiter.
        sed -i.bak "s|^MERIDIAN_DB_KEY=.*|MERIDIAN_DB_KEY=$key|" .env && rm -f .env.bak
    else
        echo "MERIDIAN_DB_KEY=$key" >> .env
    fi

    echo "wrote a new MERIDIAN_DB_KEY into .env — back it up with the database,"
    echo "and note that losing it means relinking every bank."
fi

if ! docker info >/dev/null 2>&1; then
    echo "docker is not running — start Docker Desktop and try again." >&2
    exit 1
fi

docker compose up -d db

# Polled rather than `up --wait`, which is newer than the Compose some people
# have. `pg_isready` is the same check the healthcheck in docker-compose.yml
# runs, asked from outside.
printf 'waiting for postgres'

waited=0
until docker compose exec -T db pg_isready -U "$DB_USER" -d "$DB_NAME" >/dev/null 2>&1; do
    if [ "$waited" -ge 60 ]; then
        echo
        echo "postgres did not come up in 60s. \`docker compose logs db\` will say why." >&2
        exit 1
    fi

    printf '.'
    sleep 1
    waited=$((waited + 1))
done

echo
echo "postgres is up on 127.0.0.1:7110, database '$DB_NAME'"
echo "migrations apply when the server starts: cargo run -p server-main"
