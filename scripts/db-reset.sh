#!/bin/sh
# Destroys the database and builds an empty one.
#
# **This deletes every row, permanently, with no backup.** It exists because a
# database is the one part of this project that remembers what you did to it
# last week, and a wrong migration you cannot get out of is a worse place to be
# than an empty database.
#
# Two guards, in the spirit of `PlaidEnv` defaulting to Sandbox: the harmless
# thing happens when you are not paying attention, and the destructive one
# takes saying so out loud.
#
#   1. It refuses to run against anything but a database on this machine.
#   2. It makes you type the database name back.
#
# Never run this from a script, a hook, or an agent. It is for a person who has
# decided.

set -e

cd "$(dirname "$0")/.."

[ -f .env ] && . ./.env
DB_URL=${DATABASE_URL:-postgres://meridian:meridian@localhost:5432/meridian}
DB_NAME=${POSTGRES_DB:-meridian}

# Guard one. The host out of a postgres:// URL: everything between the last `@`
# and the following `:` or `/`.
HOST=$(printf '%s' "$DB_URL" | sed -e 's|.*@||' -e 's|[:/].*||')

case "$HOST" in
    localhost|127.0.0.1|::1|"") ;;
    *)
        echo "refusing to reset a database on '$HOST'." >&2
        echo "this script only ever touches one on this machine." >&2
        exit 1
        ;;
esac

# Guard two. Not a y/n: the point is to make you read which database it is.
echo "This deletes everything in '$DB_NAME' on $HOST. There is no backup."
printf "Type the database name to confirm: "
read -r CONFIRM

if [ "$CONFIRM" != "$DB_NAME" ]; then
    echo "not confirmed, nothing done." >&2
    exit 1
fi

# `-v` is the whole point: without it the volume survives and the new container
# comes up holding the old rows.
docker compose down -v

exec "$(dirname "$0")/db-init.sh"
