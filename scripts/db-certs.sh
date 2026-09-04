#!/bin/sh
# The certificates Postgres serves TLS with, and the CA the server checks it
# against.
#
#     ./scripts/db-certs.sh          make them if they are not there
#     ./scripts/db-certs.sh --force  make them again, whatever is there
#
# Run once per machine that runs a database. The laptop makes its own and the
# box makes its own, because a private key that travels is a private key that
# has been on two disks and in whatever copied it. `scripts/deploy.sh ship`
# runs this on the box for that reason, rather than shipping what is here.
#
# The CA is our own. There is no third party to convince: exactly one client
# connects to this database and it is ours, so the only question the
# certificate has to answer is "is this the Postgres I was configured for",
# which a private CA answers as well as a public one and without the internet
# being involved.
#
# **Ten years, on purpose.** A certificate that expires takes the database down
# with no code having changed, at whatever hour it happens to be, and the
# rotation that would justify a short life is not a thing anybody here is set
# up to do. The trade is deliberate: this key is worth exactly as much as the
# machine it sits on, which already holds the database it protects.
#
# The names matter more than they look. The server reaches Postgres as `db` on
# the compose network, and `sslmode=verify-full` checks the name it dialled
# against the certificate. `localhost` and `127.0.0.1` are there so host
# tooling — psql, scripts/db-shell.sh — can verify too if it asks to.

set -e

# Run as a file, this walks up to the repo root the way every other script
# here does. Piped — `ssh box 'cd /srv/meridian && sh -s' < scripts/db-certs.sh`,
# which is how the box gets its own, since there is no checkout there — `$0` is
# just `sh` and there is no path to resolve, so the caller's directory stands.
case "${0:-}" in
    */*) cd "$(dirname "$0")/.." ;;
esac

DIR="${MERIDIAN_CERTS:-certs}"

if [ "${1:-}" = "--force" ]; then
    rm -f "$DIR/ca.crt" "$DIR/ca.key" "$DIR/server.crt" "$DIR/server.key"
fi

if [ -f "$DIR/server.crt" ] && [ -f "$DIR/server.key" ] && [ -f "$DIR/ca.crt" ]; then
    echo "certificates are already in $DIR — pass --force to replace them"
    exit 0
fi

mkdir -p "$DIR"

# A config file rather than `-addext`, which LibreSSL — what `openssl` is on a
# Mac — does not reliably have. This is the one form that works on both.
CONF=$(mktemp)
trap 'rm -f "$CONF"' EXIT

cat > "$CONF" <<'EOF'
[req]
distinguished_name = dn
x509_extensions    = v3_ca
prompt             = no

[dn]
CN = Meridian Postgres CA

[v3_ca]
basicConstraints       = critical, CA:TRUE, pathlen:0
keyUsage               = critical, keyCertSign, cRLSign
subjectKeyIdentifier   = hash

[server]
distinguished_name = server_dn
prompt             = no

[server_dn]
CN = db

[v3_server]
basicConstraints = critical, CA:FALSE
keyUsage         = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth
subjectAltName   = DNS:db, DNS:localhost, IP:127.0.0.1
EOF

echo "making a CA and a server certificate in $DIR/ ..."

openssl req -x509 -newkey rsa:2048 -nodes -days 3650 \
    -config "$CONF" -extensions v3_ca \
    -keyout "$DIR/ca.key" -out "$DIR/ca.crt" 2>/dev/null

openssl req -new -newkey rsa:2048 -nodes \
    -config "$CONF" -section server \
    -keyout "$DIR/server.key" -out "$DIR/server.csr" 2>/dev/null

openssl x509 -req -in "$DIR/server.csr" -days 3650 \
    -CA "$DIR/ca.crt" -CAkey "$DIR/ca.key" -CAcreateserial \
    -extfile "$CONF" -extensions v3_server \
    -out "$DIR/server.crt" 2>/dev/null

rm -f "$DIR/server.csr" "$DIR/ca.srl" "$DIR/server.srl"

# The mode here is not what Postgres reads. The container copies these to a
# private directory owned by `postgres` before it starts, because a bind mount
# carries whatever ownership the host gave it and that differs between a Mac
# and the box — which is exactly the kind of difference that works in
# development and fails on a deploy. This is the laptop's own hygiene.
chmod 600 "$DIR/ca.key" "$DIR/server.key"
chmod 644 "$DIR/ca.crt" "$DIR/server.crt"

echo "done. $DIR/ is gitignored and stays on this machine."
