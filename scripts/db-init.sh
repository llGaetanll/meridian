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
echo "postgres is up on 127.0.0.1:5432, database '$DB_NAME'"
echo "migrations apply when the server starts: cargo run -p server-main"
