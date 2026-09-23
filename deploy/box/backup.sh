#!/bin/bash
# A dump of the database, encrypted, into the meridiancash-backups Space.
#
# This is the on-box half of #38, run daily by meridian-backup.timer; the
# by-hand half is scripts/backup.sh, which pulls a dump to a laptop and stays
# the copy that lives outside the DigitalOcean account. The dump goes through
# `age` before it exists anywhere: the only file this writes is ciphertext,
# and it is deleted once the upload is confirmed.
#
# `age` holds only the *public* key, so the box can write backups it cannot
# read. The private key is in the password manager, next to MERIDIAN_DB_KEY,
# and restoring needs both: `age` to open the archive, MERIDIAN_DB_KEY to make
# the sealed access tokens inside it mean anything.
#
# curl does the upload because it is the one S3 client on the box that DO's
# per-bucket keys accept: Debian 13's rclone signs with a 2022 aws-sdk-go that
# those keys answer 403, while curl's own --aws-sigv4 works. A file is the
# price — curl cannot stream stdin to S3 without chunked signing — and it is
# paid in ciphertext.
#
# Nothing here deletes anything in the bucket. Its lifecycle rule expires
# dumps at 30 days, which is why the key in .env only ever needs to write.

set -euo pipefail

cd /srv/meridian

# The three values this needs from .env, read one at a time rather than
# sourcing the file: .env belongs to docker compose, and a shell sourcing it
# would trip on anything compose accepts that bash does not.
key() { sed -n "s/^$1=//p" .env | tail -1; }

access_key=$(key SPACES_ACCESS_KEY)
secret_key=$(key SPACES_SECRET_KEY)
recipient=$(key BACKUP_AGE_RECIPIENT)

if [ -z "$recipient" ] || [ -z "$access_key" ] || [ -z "$secret_key" ]; then
    echo "SPACES_ACCESS_KEY, SPACES_SECRET_KEY and BACKUP_AGE_RECIPIENT belong in .env" >&2
    exit 1
fi

# UTC, in a format that sorts — scripts/backup.sh's reasoning, and its name
# shape too, so a listing of either place reads the same way.
stamp=$(date -u +%Y%m%dT%H%M%SZ)
out="meridian-$stamp.dump.age"
tmp=$(mktemp /srv/meridian/.backup-XXXXXX)

trap 'rm -f "$tmp"' EXIT

# A backup reads every row there is, so the audit log gets its line first —
# before the dump, so the record of the backup rides inside the backup. NULL
# is the log's word for every user at once.
COMPOSE_PROFILES=server MERIDIAN_SHA=$(cat .sha) \
    docker compose -f docker-compose.yml -f docker-compose.prod.yml \
    exec -T db sh -c 'psql -qc "INSERT INTO audit_log (at, user_id, scope, client) VALUES (now(), NULL, '\''backup'\'', '\''box'\'')" -U "$POSTGRES_USER" "$POSTGRES_DB"'

# The same exec scripts/backup.sh uses: credentials stay the container's own.
COMPOSE_PROFILES=server MERIDIAN_SHA=$(cat .sha) \
    docker compose -f docker-compose.yml -f docker-compose.prod.yml \
    exec -T db sh -c 'pg_dump --format=custom -U "$POSTGRES_USER" "$POSTGRES_DB"' \
    | age -r "$recipient" > "$tmp"

# pipefail catches a pg_dump that dies, but not one that half-succeeds into a
# short file. An empty database dumps to a few kilobytes, so anything under
# one is not a backup, and a real one opens with age's own magic.
size=$(stat -c %s "$tmp")

if [ "$size" -lt 1024 ] || [ "$(head -c 22 "$tmp")" != "age-encryption.org/v1" ]; then
    echo "the dump came out $size bytes and/or not age-encrypted — not uploaded." >&2
    exit 1
fi

code=$(curl -sS -o /dev/null -w '%{http_code}' -X PUT -T "$tmp" \
    --aws-sigv4 "aws:amz:nyc3:s3" --user "$access_key:$secret_key" \
    "https://meridiancash-backups.nyc3.digitaloceanspaces.com/$out")

if [ "$code" != "200" ]; then
    echo "the upload answered $code — $out is not in the bucket." >&2
    exit 1
fi

echo "kept $size bytes  $out"
