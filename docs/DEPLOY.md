# Deploying

The server runs on one DigitalOcean droplet at `api.meridiancash.io`, in the
same containers it runs in on a laptop, with a proxy in front of it holding the
only open port.

```
      internet
         │  443
         ▼
    ┌─────────┐
    │  caddy  │  TLS, and nothing else. Certificate from Let's Encrypt.
    └────┬────┘
         │  server:8082, over the compose network
    ┌────▼────┐
    │ server  │  server-main. Publishes 8082 on loopback only.
    └────┬────┘
         │  db:5432
    ┌────▼────┐
    │   db    │  postgres:17-alpine, on a named volume.
    └─────────┘
```

Everything is driven from a laptop with `scripts/deploy.sh`, which is ssh and
nothing else — no registry, no CI, and a deploy that still works on the day
GitHub is down.

**The image is built on the laptop and shipped over ssh**, with
`docker save | ssh docker load`. The box holds no source, no Rust toolchain and
no git; it runs Postgres, Caddy and a ten-megabyte binary. Compiling 427 crates
is a fixed cost, and a machine sized to do it is that cost charged monthly
rather than once — the running stack measures 11 MiB for the server and 98 MiB
for Postgres, which is a different machine entirely from one that can link
`server-main`.

The smallest droplet is therefore the right one, and it is the reason the
laptop's `docker build` is the only compiler in this document.

## The names

| Name | Points at | Used for |
| --- | --- | --- |
| `api.meridiancash.io` | the box | The server. The only name that has to exist. |
| `meridiancash.io` | nothing yet | The site, later. Kept free on purpose. |

The server is on its own subdomain so that putting a marketing page or the web
app on the apex later is a DNS record rather than a migration.

## Before the box exists

None of this needs a server, and each of them takes longer to propagate than to
do — which is the argument for doing them first.

**An ssh key, if there isn't one.** A droplet created with a key has no root
password at all; one created without gets a root password emailed in plain
text.

```sh
ssh-keygen -t ed25519 -C meridian
cat ~/.ssh/id_ed25519.pub    # this is what the droplet is created with
```

**Verify the domain in Resend.** Done: `mail.meridiancash.io` is verified, and
mail is sent from `noreply@mail.meridiancash.io`. It is three records at the
registrar — DKIM at `resend._domainkey.mail`, and an SPF `TXT` and an `MX` at
`send.mail` — and they go wherever the domain's nameservers actually are, which
for `meridiancash.io` is Namecheap and not DigitalOcean. `dig
@dns1.registrar-servers.com resend._domainkey.mail.meridiancash.io TXT` asks
the authority directly and so tells propagation apart from a record that was
never added.

It is the slowest thing here, so on a new domain it still wants starting first.
`EMAIL_SEND=log` remains a legitimate answer for a first deploy: it writes the
code to the log and sends nothing, which is enough to walk a sign-in.

DMARC is the one record not in the set above and Resend does not require it,
but without it neither SPF nor DKIM is enforceable by a receiver. A
`v=DMARC1; p=none; rua=…` at `_dmarc.meridiancash.io` covers the `mail.`
subdomain too, since a receiver falls back to the organizational domain.

**Nothing in the Plaid dashboard yet.** The redirect URI and the webhook both
name `api.meridiancash.io`, which is fine to register before it resolves — but
neither is read until the code that sends them exists, and the webhook route
does not (#20). Sandbox and `PLAID_COLLECT=poll` is what the first deploy runs.

## The box

The smallest Basic droplet — 1 vCPU, 1 GB, 25 GB — because nothing here
compiles. Debian, and the public key above.

Sizing is reversible in one direction only: CPU and RAM can be raised and
lowered again, but growing the disk is permanent. So start at the bottom.

```sh
doctl compute droplet create meridian \
    --region nyc3 --size s-1vcpu-1gb --image debian-13-x64 \
    --ssh-keys "$(doctl compute ssh-key list --no-header --format ID)" \
    --enable-backups --enable-ipv6 --wait
```

`--enable-backups` is 20% on top and is the whole of the backups story below.
`--enable-ipv6` is not the default and cannot be inferred later from the AAAA
record the DNS section asks for — a droplet created without it has no v6
address to put there.

**The first boot is already running `apt`.** Debian's unattended-upgrades takes
the lock within seconds of the machine answering ssh, and anything below that
installs a package fails with `Could not get lock /var/lib/apt/lists/lock`
rather than waiting. It clears on its own in a minute or two; the fix is to
wait for it, not to break the lock.

**Swap, because 1 GB has no headroom for a surprise.** Nothing here needs it in
steady state; it is what turns an unexpected spike into a slow minute rather
than the OOM killer choosing between Postgres and the server.

```sh
fallocate -l 2G /swapfile && chmod 600 /swapfile
mkswap /swapfile && swapon /swapfile
echo '/swapfile none swap sw 0 0' >> /etc/fstab
```

**Docker**, from Docker's own repository rather than Debian's, which ships a
version old enough to lack `docker compose` as a subcommand:

```sh
curl -fsSL https://get.docker.com | sh
```

This comes before the user below rather than after it, because `usermod` names
the `docker` group and that group does not exist until this has run.

**A user that is not root.**

```sh
adduser --disabled-password --gecos "" meridian
usermod -aG sudo,docker meridian
rsync --archive --chown=meridian:meridian ~/.ssh /home/meridian/

echo 'meridian ALL=(ALL) NOPASSWD:ALL' > /etc/sudoers.d/meridian
chmod 440 /etc/sudoers.d/meridian
```

The sudoers line is not a convenience. `--disabled-password` leaves the account
with no password to give `sudo`, so without it the box becomes unadministrable
the moment root login is refused below. It concedes nothing either: the same
user is in the `docker` group, and anything that can run a container can mount
`/` into it and is root already.

**Refuse passwords and refuse root.** In `/etc/ssh/sshd_config`:

```
PermitRootLogin no
PasswordAuthentication no
```

then `systemctl restart ssh` — from a second terminal that is already logged
in, so a mistake in that file is an inconvenience rather than a locked door.

**The firewall**, on the machine rather than in the cloud firewall so that it
travels with the box if the box ever moves.

What it is not is the thing keeping Postgres off the internet. Docker writes
its own iptables rules ahead of ufw's, so a published container port is
reachable whatever ufw says — the `127.0.0.1:` in front of every port in
docker-compose.yml is the only lock on that door, and it is the one to check if
this ever looks wrong. ufw covers what the *host* listens on, which here is ssh
and nothing else.

```sh
ufw default deny incoming
ufw allow 22/tcp
ufw allow 80,443/tcp
ufw allow 443/udp        # HTTP/3
ufw enable
```

**A directory to be run from**, in `/srv` because this is something the
machine runs rather than somebody's work:

```sh
sudo install -d -o meridian -g meridian /srv/meridian
```

That is all. It is not a checkout — `ship` copies the two compose files and the
Caddyfile into it, and the box's `.env` below is written once by hand. Nothing
on the box is ever `git pull`ed, so there is no branch here to be on and no
state here to conflict.

## DNS, and why it comes before the first deploy

Point `api.meridiancash.io` at the box — an `A` record for the IPv4 address and
an `AAAA` for the IPv6 — and wait for it to resolve.

Caddy asks Let's Encrypt for a certificate the first time it serves the name,
and the challenge is answered over port 80 at that name. If the record is not
live yet, the challenge fails, and Let's Encrypt's rate limit is five
certificates per name per week — so this is the one step where being early
costs a week of waiting. `dig api.meridiancash.io` before the first `up`.

## The box's `.env`

Not the laptop's. It lives at `/srv/meridian/.env`, never enters an image, and
is never committed — the same rules as the development one, with different
answers:

```sh
# The proxy. No defaults anywhere: a box that has not said which name it
# answers to would ask for a certificate for nothing.
MERIDIAN_DOMAIN=api.meridiancash.io
TLS_EMAIL=you@example.com

# Loopback, not every interface. Caddy reaches the server by service name over
# the compose network; the published port is only good for a `curl` from a
# shell on the box.
SERVER_BIND=127.0.0.1

# The database, inside the network. Generate the password — do not reuse the
# development one, which is `meridian` and is in the repo.
POSTGRES_USER=meridian
POSTGRES_PASSWORD=<openssl rand -hex 24>
POSTGRES_DB=meridian
DATABASE_URL=postgres://meridian:<the same password>@db:5432/meridian
# `-hex`, not `-base64`: this password is pasted into a URL, and base64's `/`
# and `+` have to be percent-encoded there or the connection string is wrong in
# a way that reads as a bad password.

# What the access token column is sealed with: 32 random bytes, base64.
# `openssl rand -base64 32`. Not the laptop's key — a box and a laptop share no
# rows, so sharing a key only widens what one leak costs.
#
# **Back this up somewhere other than the database.** Losing it means every
# bank on this box has to be relinked, which in Production spends Item slots
# that do not come back. Losing it *with* a backup of the database means the
# backup is scrap. docs/DATABASE.md has the reasoning.
MERIDIAN_DB_KEY=…

# Sandbox. Deliberately, and for as long as it takes — see docs/PLAID.md, and
# note that the ten Production Item slots do not come back.
PLAID_CLIENT_ID=…
PLAID_SECRET=…            # the Sandbox one; the two environments differ
PLAID_ENV=sandbox
PLAID_COLLECT=poll        # `webhook` needs a route that does not exist yet

# Real mail, from the verified domain. `log` writes the code to the server log
# and sends nothing, which is the answer before Resend is set up.
EMAIL_SEND=resend
RESEND_API_KEY=…          # the box's own key, not a copy of the laptop's
RESEND_FROM=noreply@mail.meridiancash.io
```

`chmod 600 .env`. It holds the Plaid secret, which grants access to every Item
on the account.

The `DATABASE_URL` here says `db`, not `localhost`: docker-compose.yml
overrides it for the server container anyway, but a copy that disagrees is a
copy somebody will read and believe.

## The first deploy

From the laptop, with `MERIDIAN_HOST=meridian@api.meridiancash.io` in `.env`:

```sh
./scripts/deploy.sh ship       # compiles here; the first one is slow
./scripts/deploy.sh ps
./scripts/deploy.sh logs
./scripts/deploy.sh health     # through DNS, TLS and Caddy, to the server
```

`health` answering `ok store=postgres` over https means all four of DNS, the
certificate, the proxy and the database are right. Migrations run at boot, so
there is no separate step for the schema.

It asks by presenting a token that is deliberately not a session, and expects a
401. There is no `/health` route: an unauthenticated one is a thing anyone on
the internet can make the server do, and rate limiting it would have meant
counting Caddy's own address. The 401 proves more anyway — `authenticate` reads
the token out of Postgres before it can reject it, so a database that is down
answers 500 there rather than 401.

After that, every deploy is that same command: build here, load it there, swap.
It refuses a dirty tree, because the commit it stamps into every log line is
decided on the laptop now and an image labelled with a commit that does not
contain it sends the next reader to the wrong source.

Going back does not rebuild anything. Every image the box has been shipped is
still on it under its own tag:

```sh
./scripts/deploy.sh rollback 6f31686
```

which is worth the disk precisely because there is no source on the box to
rebuild an old one from. `docker image prune` there when it matters; at ~98 MB
an image and 25 GB of disk, it will not be soon.

## Then the phone

`swift/Support/Info.plist` names `http://127.0.0.1:8082` and carries an
`NSAppTransportSecurity` exemption to permit plain http on the local network.
Against a hosted server both are wrong: the URL becomes
`https://api.meridiancash.io` and the exemption comes out, because https to a
real name is exactly what App Transport Security wants by default.

`scripts/ios-device.sh` writes the LAN address into the plist at build time for
a phone talking to a Mac. Pointing a build at the box is a separate switch, not
a change to that one.

## Backups

Droplet backups, enabled at create time above, are weekly snapshots of the
whole disk and restore to a working box rather than to a directory of files.
That is the floor, and it is 20% of the droplet.

It is not yet enough. Weekly means up to a week of transactions lost, and a
whole-disk snapshot is a clumsy way to recover one table. It is also not a
backup of `MERIDIAN_DB_KEY` in any useful sense: the key lives on the same disk
as the rows it opens, so a snapshot that loses one loses both. `pg_dump` on a timer
to Spaces is the shape, and the volume it protects holds the Plaid access
tokens and every transaction ever synced.

## What has to be true before this holds real money

The box being up is not the same as it being ready, and #20 tracks the rest.
The four that are specifically about facing the internet:

- **CORS is `Any` and there is no rate limiting** (#23). Fine on a laptop.
- **The Plaid access token is stored as plain text** (#21). A rented box
  holding Production tokens is a different risk from a laptop holding Sandbox
  ones.
- **`sweep_sessions` is never called** (#24), so expired sessions accumulate.
- **No webhook route**, so `PLAID_COLLECT=webhook` parses and then behaves as
  `poll`. Hosting is what makes a webhook possible, so the two land together.
