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
| `meridiancash.io` | nowhere yet | The site — `site/`, built to static files. |
| `app.meridiancash.io` | nothing yet | The web app, when it ships. |

The server is on its own subdomain so that putting the site or the web app on
the apex is a DNS record rather than a migration.

The site is three static pages and nothing else, so it does not belong on this
box: `npm run build` in `site/` writes `out/`, and any static host takes it
from there. The apex and the API are independent, which is the point — a deploy
of one cannot take down the other.

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
name `api.meridiancash.io`, which is fine to register before it resolves. The
webhook route exists now (#36), so `PLAID_COLLECT=webhook` is available once
the dashboard names it and the box's `.env` carries a `PUBLIC_URL`; Sandbox and
`PLAID_COLLECT=poll` is what the first deploy ran.

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
PLAID_COLLECT=poll        # `webhook` works now (#36); poll until it is chosen

# Where an OAuth bank's own app hands the user back. Empty, and it stays empty
# until the phone can claim a Universal Link — see below.
PLAID_REDIRECT_URI=
MERIDIAN_TEAM_ID=

# Real mail, from the verified domain. `log` writes the code to the server log
# and sends nothing, which is the answer before Resend is set up.
EMAIL_SEND=resend
RESEND_API_KEY=…          # the box's own key, not a copy of the laptop's
RESEND_FROM=noreply@mail.meridiancash.io

# The origins a browser may call this server from. The web app, and nothing
# else — a laptop's `http://127.0.0.1:8080` here would be a value that matches
# no browser on the internet and blocks the real one.
WEB_ORIGINS=https://app.meridiancash.io
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

`swift/Support/Info.plist` names `https://api.meridiancash.io` and asks for no
App Transport Security exemption, because https to a real name is exactly what
ATS wants by default. A build off this tree points at the box with nothing set.

A build pointed at a laptop is the exception, and it is the build scripts that
make it: `ios-run.sh` writes the Mac's loopback into the copy under `target/`
and `ios-device.sh` the Mac's address on the LAN, each adding the exemption
plain http then needs. `MERIDIAN_SERVER` names the address yourself, and
setting it to an `https://` one is how either script points back at the box.

## The handback an OAuth bank needs

An OAuth bank — Citi and most of the large ones — does not take the sign-in on
Plaid's page. It hands off to its own app, and the only way that app has of
returning control is to open a URL. That URL is the link token's top-level
`redirect_uri`, and it is not the `meridian://linked` that closes the sheet;
[PLAID.md](PLAID.md) has why the two cannot be the same address.

Caddy already serves both halves of it on this box:

| | What it is |
| --- | --- |
| `/.well-known/apple-app-site-association` | What tells iOS this domain vouches for the app. Names `$MERIDIAN_TEAM_ID.com.meridian.phone` |
| `/link/oauth` | The address itself. Only ever *rendered* when the association failed — in the working case iOS opens the app instead |

Both are live right now and both are inert, because `MERIDIAN_TEAM_ID` names no
team and `PLAID_REDIRECT_URI` is empty, so no link token points a bank here.
That is the correct state: telling Plaid a native app opened the session
without giving the bank a reachable address strands the user in their bank's
app *after* they have signed in.

Turning it on is four steps and needs a paid Apple Developer account first,
since the entitlement that lets the app claim the link is not one a free
profile grants:

1. Tick **Associated Domains** on the `com.meridian.phone` App ID, and add
   `applinks:api.meridiancash.io` to the app's entitlements
2. Set `MERIDIAN_TEAM_ID` on the box and redeploy, then check the file:
   `curl -sI https://api.meridiancash.io/.well-known/apple-app-site-association`
   must answer 200 with `content-type: application/json` and no redirect
3. Register `https://api.meridiancash.io/link/oauth` in the Plaid dashboard
4. Set `PLAID_REDIRECT_URI` to that same address and restart the server. The
   startup line's `handback=` says whether it took

Then link `ins_132241` in Sandbox, which is Plaid's app-to-app test bank. It
spends no Item, and it is the only way to see the hop actually happen before a
real bank does it.

## Backups

Droplet backups, enabled at create time above, are weekly snapshots of the
whole disk and restore to a working box rather than to a directory of files.
That is the floor, and it is 20% of the droplet.

It is not enough on its own. Weekly means up to a week of transactions lost,
and a whole-disk snapshot is a clumsy way to recover one table.

So there is a dump, taken by hand from a laptop:

```sh
./scripts/backup.sh          a dump of the box, into ./backups
./scripts/backup.sh list     what is already there
```

It runs `pg_dump -Fc` inside the db container and streams the result back over
the same ssh connection every other box command uses — nothing is written on
the droplet, and no port has to be open for it. The archive is Postgres's own
compressed format, which `pg_restore` can read a table out of without restoring
the rest. `backups/` is gitignored: a dump holds every transaction, every email
address and the sealed access tokens, and is as sensitive as the box.

What is still missing is most of #38.

**It is a person, not a timer.** A backup nobody remembers to take is a weekly
snapshot with extra steps.

**It is on a laptop, which is not off-site from a laptop.** `pg_dump` on a
timer to Spaces is still the shape.

**`MERIDIAN_DB_KEY` is escrowed, the backups are not.** The key that opens the
access token column is in `/srv/meridian/.env` on the box and in a password
manager, so a dump can actually be restored — without it the restore is a
ledger with every bank unlinked. It is deliberately not *in* a backup: a dump
that carried its own key would undo the sealing.

**The other two secrets in that file are not escrowed.** The Plaid secret and
the Postgres password exist on the box and nowhere else, so losing it loses
both — #54.

**No restore has been run.** A backup that has never been restored is a belief,
not a backup.

## Encryption at rest, and why there is none

Postgres writes to a docker named volume on the droplet's own disk, and a
droplet's disk is not encrypted at rest. DigitalOcean is explicit about this
and it is the one place their two storage products differ: Block Storage
Volumes are LUKS-encrypted by DigitalOcean, with snapshots inheriting it, and
a droplet's local disk is not.

So balances, transactions, merchant names and email addresses sit in the clear
on the box and in every weekly droplet backup. **This is accepted, as of
2026-09-04, not overlooked.** Everything here is Plaid Sandbox: invented
transactions belonging to nobody, on a box no one else can reach.

Moving the data onto a Volume is the fix and it is cheap — about a dollar a
month at any size this database will be for a long time, and no key management,
because DigitalOcean holds that key rather than us. What makes it not worth
doing today is that it is not urgent, and what makes it worth doing early is
that the migration only gets more expensive: right now `0007` has just emptied
`items` and cascaded the ledger with it, so there is nearly nothing to move.

**Revisit before the first real customer**, which is the same trigger as
applying for Plaid Production. Two things to know when that day comes:

- Droplet backups snapshot the *droplet's* disk. The moment Postgres lives on
  a Volume they stop covering the database, and Volume snapshots are not
  scheduled for you. The move and a working `pg_dump` have to land together or
  there is a window with neither.
- A LUKS layer of our own on top of the Volume buys protection from
  DigitalOcean themselves and costs a passphrase at every unattended reboot.
  Decided against, for now, for that reason rather than for the effort.

Worth being clear what any of this buys, because it is easy to overstate. Disk
encryption protects a volume that is **not mounted**: a stolen drive, a
decommissioned host, a snapshot copied somewhere it should not be. It does
nothing against someone who gets onto a running box, because the volume is
decrypted the whole time the box is up. It is not a substitute for sealing the
access token, and sealing the access token is not a substitute for it.

## The server's connection to Postgres

The server reaches the database over TLS and checks the certificate:
`sslmode=verify-full`, set in `docker-compose.yml` on the `DATABASE_URL` that
compose builds. Postgres serves it with a certificate from a CA of our own,
made by `scripts/db-certs.sh`.

**The mode is stated rather than defaulted, and that is most of the point.**
sqlx defaults to `prefer` — try TLS, fall back to plain text, say nothing
either way — which is how this connection was in the clear with nothing in the
repo admitting it. It is worse than it sounds: `prefer` against a server that
does offer TLS takes it *without checking the certificate*, which looks secure
and is not. `require` has the same hole on purpose — it encrypts and accepts
any certificate at all, so it stops a listener and not the machine that
answers. `verify-full` is the only one of the three that authenticates, and it
also checks the name, which is why the certificate carries `db`.

The certificates are made **on the machine that uses them** and never travel:
the laptop has its own, the box has its own, and the two are unrelated. That is
why `deploy.sh ship` pipes the script over ssh rather than copying a key — a
private key that has been sent somewhere has been on two disks and in whatever
carried it. `certs/` is gitignored.

Two things worth knowing before this surprises somebody:

- **Postgres will not start without them.** `ship` and `up` both run the script
  before bringing anything up, and it is a no-op when the files are there, so
  a box that has certificates keeps them rather than getting a new key under a
  running database on every deploy. `./scripts/deploy.sh certs --force`
  replaces them deliberately, and wants an `up` after it, not a `restart` —
  Postgres reads the certificate when it starts.
- **Ten years, on purpose.** A certificate that expires takes the database down
  with no code having changed, at whatever hour it lands, and nobody here is
  set up to rotate one. The key is worth what the machine it sits on is worth,
  and that machine already holds the database it protects.

`ssl = on` offers TLS; it does not require it. What requires it is the client
saying so, which the server does and host tooling on loopback deliberately does
not — `.env.example` has that reasoning. The day the database leaves this box,
that changes with it: a managed Postgres, a replica or a second machine all
mean `verify-full` everywhere, and it is the same trigger as the volume
question above.

## What has to be true before this holds real money

The box being up is not the same as it being ready, and #20 tracks the rest.
What is specifically about facing the internet:

- **The volume is not encrypted at rest** (#33) — accepted, with a trigger. The
  server no longer talks to Postgres in plain text (#34); the section above has
  both.
- **Backups are weekly whole-disk snapshots, plus a dump somebody remembers to
  take** (#38). Neither is off-site. `MERIDIAN_DB_KEY` is escrowed separately,
  which is the right way round — see the backups section.
- **An OAuth bank has nowhere to hand control back to.** The server half is
  deployed and switched off; what is missing is the Apple Developer account
  that lets the app claim the link. The section above has the four steps.
