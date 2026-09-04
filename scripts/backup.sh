#!/bin/sh
# A dump of the box's database, on this machine.
#
#     ./scripts/backup.sh          take one now
#     ./scripts/backup.sh list     what is already here
#
# This is the first half of #38 and deliberately not the whole of it. It is a
# person deciding to take a backup, not a timer, and the copy lands on whatever
# laptop typed it rather than off-site. What it does buy today is the thing
# weekly droplet snapshots cannot: a dump of the rows, taken at a moment you
# chose, that `pg_restore` can pull a single table out of.
#
# The dump is taken *inside* the db container and streamed back over the same
# ssh connection every other box command uses, so nothing is written on the
# droplet and no port has to be open. `-Fc` is Postgres's own compressed
# format: smaller than SQL text, and `pg_restore --list` reads its table of
# contents without restoring anything.
#
# **A dump is not a restore.** The access tokens in `items` are sealed with
# `MERIDIAN_DB_KEY`, which lives in the box's `/srv/meridian/.env` and nowhere
# else — restore this file without that key and every bank has to be relinked.
# Backing up that key is the other half of #38 and is not this script.
#
# Nothing here deletes anything. A dump of this database is kilobytes, and a
# backup script that removes backups on its own is a bad first version.

set -e

cd "$(dirname "$0")/.."

# The same box, found the same way as scripts/deploy.sh: `.env`, because it
# names a machine and this repo is public.
if [ -z "${MERIDIAN_HOST:-}" ] && [ -f .env ]; then
    MERIDIAN_HOST=$(sed -n 's/^MERIDIAN_HOST=//p' .env | tail -1)
fi

if [ -z "${MERIDIAN_HOST:-}" ]; then
    echo "MERIDIAN_HOST is not set — put it in .env as user@host, or export it." >&2
    echo "  MERIDIAN_HOST=meridian@api.meridiancash.io" >&2
    exit 1
fi

MERIDIAN_PATH="${MERIDIAN_PATH:-/srv/meridian}"

# Where the dumps land. In the tree, gitignored, next to `.env` — which holds
# the Plaid secret and lives here for the same reason: local, private, and in
# the one directory anyone working on this project already knows about.
DIR="${MERIDIAN_BACKUPS:-backups}"

# Identical to the line in scripts/deploy.sh, and single-quoted for the same
# reason: `$(cat .sha)` has to survive to the box rather than being answered
# here. `exec` needs the file to parse, and it does not parse without the tag.
COMPOSE='COMPOSE_PROFILES=server MERIDIAN_SHA=$(cat .sha) docker compose -f docker-compose.yml -f docker-compose.prod.yml'

# `$POSTGRES_USER` and `$POSTGRES_DB` are the container's own environment, so
# the credentials are never on this machine and never on the wire. They are
# escaped twice on the way: once so this shell leaves them alone, once so the
# box's shell hands them to the container's.
DUMP='exec -T db sh -c "pg_dump --format=custom -U \"\$POSTGRES_USER\" \"\$POSTGRES_DB\""'

case "${1:-dump}" in
    dump)
        mkdir -p "$DIR"

        # UTC, and in a format that sorts. A backup named in local time is a
        # backup that lies twice a year.
        stamp=$(date -u +%Y%m%dT%H%M%SZ)
        out="$DIR/meridian-$stamp.dump"
        tmp="$DIR/.partial-$stamp.dump"

        trap 'rm -f "$tmp"' EXIT

        echo "dumping $MERIDIAN_HOST → $out"
        ssh "$MERIDIAN_HOST" "cd '$MERIDIAN_PATH' && $COMPOSE $DUMP" > "$tmp"

        # Three checks before this counts as a backup, because the failure
        # this guards against is not a dump that errors — `set -e` catches
        # that — but one that half-succeeds and leaves a file that looks like
        # a backup until the day it is needed. A custom-format archive opens
        # with `PGDMP`; anything else here is a shell error caught in a pipe.
        if [ ! -s "$tmp" ]; then
            echo "the dump came back empty — nothing kept." >&2
            exit 1
        fi

        if [ "$(head -c 5 "$tmp")" != "PGDMP" ]; then
            echo "the dump is not a Postgres archive — nothing kept." >&2
            head -c 200 "$tmp" >&2
            exit 1
        fi

        mv "$tmp" "$out"
        echo "kept $(du -h "$out" | cut -f1)  $out"
        ;;

    list)
        set -- "$DIR"/meridian-*.dump

        if [ ! -e "$1" ]; then
            echo "no backups in $DIR yet."
            exit 0
        fi

        # Oldest first, which the glob gives for free: the stamps are UTC and
        # fixed width, so lexical order is chronological order.
        for f in "$@"; do
            printf '%s\t%s\n' "$(du -h "$f" | cut -f1)" "$f"
        done
        ;;

    *)
        echo "usage: $0 [dump|list]" >&2
        exit 1
        ;;
esac
