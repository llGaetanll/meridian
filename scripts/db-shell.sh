#!/bin/sh
# A psql prompt on the running database.
#
# `\dt` lists the tables, `\d items` describes one, and
# `select * from _sqlx_migrations;` is how you see which migrations this
# database has actually run — which is the first thing worth checking when the
# schema is not what you expected.

set -e

cd "$(dirname "$0")/.."

[ -f .env ] && . ./.env
DB_USER=${POSTGRES_USER:-meridian}
DB_NAME=${POSTGRES_DB:-meridian}

# Compose allocates a terminal by default and *panics* rather than explaining
# itself when there is not one — which is what a pipeline, or
# `db-shell.sh -c 'select ...'` from a script, looks like. `-T` is the answer,
# and only when it applies, since an interactive psql wants its terminal.
if [ -t 0 ]; then
    exec docker compose exec db psql -U "$DB_USER" -d "$DB_NAME" "$@"
fi

exec docker compose exec -T db psql -U "$DB_USER" -d "$DB_NAME" "$@"
