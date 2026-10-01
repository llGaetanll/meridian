# Deploying

The server runs on one DigitalOcean droplet at `api.meridiancash.io`, in the
same containers it runs in on a laptop, with a proxy in front of it holding the
only open port. The web app and the site are on the same box, behind the same
proxy, each in a container of its own.

```
                      internet
                         │  443
                         ▼
                    ┌─────────┐
                    │  caddy  │  TLS, and nothing else. Certificates from Let's Encrypt.
                    └────┬────┘
         ┌───────────────┼───────────────┐   over the compose network
         │ server:7120   │ web:3000      │ site:3000
    ┌────▼────┐     ┌────▼────┐     ┌────▼────┐
    │ server  │     │   web   │     │  site   │   web and site: Caddy handing out files
    └────┬────┘     └─────────┘     └─────────┘
         │  db:5432
    ┌────▼────┐
    │   db    │  postgres:17-alpine, on a named volume.
    └─────────┘
```

The server publishes 7121 on loopback and nothing else does: the prod file
states that with `!override`, which replaces the dev file's ports rather than
appending to them, so the box is safe without anyone remembering a variable
(#145). The web app and the site publish nothing. `docs/PORTS.md` is where the
numbers are allocated.

Everything is driven from a laptop with `scripts/deploy.sh`, which is ssh and
nothing else — no registry, no CI, and a deploy that still works on the day
GitHub is down.

**The images are built on the laptop and shipped over ssh**, with
`docker save | ssh docker load`. The box holds no source, no Rust toolchain, no
Node and no git; it runs Postgres, Caddy, a ten-megabyte binary and two
directories of files. Compiling 427 crates
is a fixed cost, and a machine sized to do it is that cost charged monthly
rather than once — the running stack measures 11 MiB for the server and 98 MiB
for Postgres, which is a different machine entirely from one that can link
`server-main`.

The smallest droplet is therefore the right one, and it is the reason the
laptop's `docker build` is the only compiler in this document.

## The names

| Name | Points at | Used for |
| --- | --- | --- |
| `api.meridiancash.io` | the box | The server. |
| `app.meridiancash.io` | the box | The web app — `crates/web-main`, compiled to wasm. |
| `meridiancash.io` | the box | The site — `site/`, exported to static files. |
| `www.meridiancash.io` | the box | A redirect to `meridiancash.io`. |

The server is on its own subdomain so that putting the site or the web app on
the apex is a DNS record rather than a migration.

All three are shipped together, under one commit, by the same command — see
[the web app and the site](#the-web-app-and-the-site) for how the two that are
files get built.

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
but without it neither SPF nor DKIM is enforceable by a receiver. It is in
place: `v=DMARC1; p=none; rua=mailto:support@meridiancash.io` at `_dmarc`,
which covers the `mail.` subdomain too, since a receiver falls back to the
organizational domain.

The host is `_dmarc`, not `_dmarc.meridiancash.io`. Namecheap appends the
domain to whatever is typed, so the longer form answers at
`_dmarc.meridiancash.io.meridiancash.io` and no receiver ever finds it. The
same goes for every host in the tables below.

**The support mailbox.** Resend only sends. Mail *to* the domain is a separate
service: Namecheap Private Email, Launch plan, one mailbox — `support@` — with
`privacy@` as an alias on it rather than a second mailbox. The plan carries one
mailbox and ten aliases, so a second published address is free and a second
mailbox is not. Both are published by `site/lib/site.ts`, and
`docs/COMPLIANCE.md` needs an address at the domain for an App Store release,
so neither may bounce.

Its records sit at the apex and do not touch Resend's, which live under
`mail.`:

| Type | Host | Value | Priority |
| --- | --- | --- | --- |
| `MX` | `@` | `mx1.privateemail.com.` | 10 |
| `MX` | `@` | `mx2.privateemail.com.` | 10 |
| `TXT` | `@` | `v=spf1 include:spf.privateemail.com ~all` | |
| `TXT` | `privateemail._domainkey` | the DKIM key, from the subscription page | |

Buying the subscription and creating the mailbox are two steps, and the
subscription page reports itself active after the first. `dig` cannot tell them
apart either, since the MX answers either way. What does:

```sh
python3 -c 'import smtplib;s=smtplib.SMTP("mx1.privateemail.com",25);s.ehlo("meridiancash.io");s.mail("postmaster@meridiancash.io");print(s.rcpt("support@meridiancash.io"))'
```

`250` is a mailbox. `450 unverified address` is the subscription without one.
Ask after a name that cannot exist too — a `250` for that is a catch-all, which
this domain does not have and should not.

Aliases are added from the subscription page, in the dropdown beside the
mailbox's `LOGIN` button — not in the webmail's own settings, which is the
mailbox user's view and has no alias section at all.

That page also shows a standing banner asking for the MX and SPF records it
already has. It is stale, not a check; believe `dig`.

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

**It is now `s-1vcpu-2gb`, with the disk grown to 50 GB**, because the box
hosts other services besides Meridian. Growing the disk is the irreversible
half, so the box cannot go back below $12/mo.

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

**Swap, because a small box has no headroom for a surprise.** Nothing here
needs it in steady state; it is what turns an unexpected spike into a slow minute rather
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

The web app and the site need their names too, and for the same reason Caddy
asks for all four certificates the first time it starts with them. At
Namecheap, under Advanced DNS for `meridiancash.io`:

| Type | Host | Value |
| --- | --- | --- |
| `A` | `@` | `138.197.20.171` |
| `AAAA` | `@` | `2604:a880:800:14:0:3:7423:5000` |
| `CNAME` | `app` | `api.meridiancash.io.` |
| `CNAME` | `www` | `meridiancash.io.` |

The two addresses are the box's, as `api.meridiancash.io` answers them. The
apex has to carry them itself, since a CNAME is not allowed there; `app` and
`www` name another name instead, so moving the box changes the records that
carry addresses and leaves these two alone. `dig +short app.meridiancash.io`,
`www.` and the apex answering the box is the check, before the `.env` lines
below go in.

## The box's `.env`

Not the laptop's. It lives at `/srv/meridian/.env`, never enters an image, and
is never committed — the same rules as the development one, with different
answers:

```sh
# The proxy. No defaults anywhere: a box that has not said which name it
# answers to would ask for a certificate for nothing. The site's name also
# answers as `www.`, which redirects to it.
MERIDIAN_DOMAIN=api.meridiancash.io
MERIDIAN_APP_DOMAIN=app.meridiancash.io
MERIDIAN_SITE_DOMAIN=meridiancash.io
TLS_EMAIL=you@example.com

# Nothing here pins the published ports any more: docker-compose.prod.yml
# states them with `!override`, so the box gets 127.0.0.1:7121 for the server
# and no publish at all for the web app, whatever a `.env` says. SERVER_BIND
# and WEB_BIND remain for laptops, which want 0.0.0.0 for a phone on the LAN.

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

# These two lines are the whole switch to real banks, and nothing in the server
# second-guesses them any more — see docs/PLAID.md. The ten Production Item
# slots do not come back, so `production` here is a decision, not a setting.
PLAID_CLIENT_ID=…
PLAID_SECRET=…            # the one for this environment; the two differ
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
# else — a laptop's `http://127.0.0.1:7130` here would be a value that matches
# no browser on the internet and blocks the real one.
WEB_ORIGINS=https://app.meridiancash.io

# The switch to real cards is these five lines, changed together. A price and
# a webhook endpoint each exist in one mode only, so a half switch is a server
# that talks to live Stripe about a test price. STRIPE_ENV names the mode,
# and a key whose prefix disagrees with it stops the server on the way up.
STRIPE_ENV=test
STRIPE_SECRET_KEY=…       # `sk_test_` or `sk_live_`, matching STRIPE_ENV
STRIPE_PUBLISHABLE_KEY=…  # `pk_test_` or `pk_live_`, the same
STRIPE_PRICE_ID=…         # the price in this mode
STRIPE_WEBHOOK_SECRET=…   # the endpoint registered in this mode
```

`chmod 600 .env`. It holds the Plaid secret, which grants access to every Item
on the account.

The `DATABASE_URL` here says `db`, not `localhost`: docker-compose.yml
overrides it for the server container anyway, but a copy that disagrees is a
copy somebody will read and believe.

**A change to this file needs `up`, not `restart`.** Compose reads `.env` when
it *creates* a container and bakes the values into it; `restart` starts the
same container again, with the same values, and the edit is never read. The
failure has no error in it — the server comes up cleanly, logs the old value,
and the only way to notice is to read the startup line. `up` recreates the
container when the config has changed, which is what makes the edit take.

The same is true of `certs --force`, for the same reason, further down.

## The first deploy

From the laptop, with `MERIDIAN_HOST=meridian@api.meridiancash.io` in `.env`:

```sh
./scripts/deploy.sh ship       # compiles here; the first one is slow
./scripts/deploy.sh ps
./scripts/deploy.sh logs
./scripts/deploy.sh health     # through DNS, TLS and Caddy, to all three
```

`health` answering `ok store=postgres` over https means all four of DNS, the
certificate, the proxy and the database are right, and an `ok` for each of the
other two names means the same of theirs, as far as a page answering 200.
Migrations run at boot, so there is no separate step for the schema.

It asks by presenting a token that is deliberately not a session, and expects a
401. There is no `/health` route: an unauthenticated one is a thing anyone on
the internet can make the server do, and rate limiting it would have meant
counting Caddy's own address. The 401 proves more anyway — `authenticate` reads
the token out of Postgres before it can reject it, so a database that is down
answers 500 there rather than 401.

After that, every deploy is that same command: build here, load it there, swap.

What it builds is `HEAD`, not the working tree, and it says which files it is
leaving behind when the two differ. The commit stamped into every log line is
decided on the laptop now, and an image labelled with a commit that does not
contain it sends the next reader to the wrong source — so `ship` exports the
commit with `git archive` into a temporary directory and builds from that. The
compose files, the Caddyfile and the two scripts it pipes over ssh come from the
same export: taking half a deploy from the tree is the same lie in a different
place. Nothing about this touches the tree or the index, so a deploy can go out
while the laptop is being worked in — which is the point, since the alternative
was stashing to ship.

Going back does not rebuild anything. The last five commits' images are on the
box under their own tags:

```sh
./scripts/deploy.sh rollback 6f31686
```

which is worth the disk precisely because there is no source on the box to
rebuild an old one from. Five is also how far back `rollback` reaches, and
`MERIDIAN_KEEP` moves it.

Every `ship` trims the rest, by piping `scripts/prune-images.sh` to the box
after the swap — a 25 GB droplet that also holds Postgres does not want every
commit it has ever run, and a trim somebody has to remember is one that stops
happening. The commit in `.sha` is kept whatever its age, so a box left on an
old image by a `rollback` does not have it removed under it. `./scripts/deploy.sh
prune` runs the same trim on its own, for a box that got ahead of this.

The three images go back together. A commit shipped before the web app and the
site had images of their own has only the server's on the box, and `rollback`
refuses it rather than bringing the stack half up.

## Reading the box's logs

Two ways, and the second is the one to leave open:

```sh
./scripts/deploy.sh logs               the last 5m of everything, and it returns
./scripts/deploy.sh logs server        the last 5m of the server alone
MERIDIAN_SINCE=30m ./scripts/deploy.sh logs server

./scripts/deploy.sh watch              follow everything
./scripts/deploy.sh watch server       follow the server alone
```

`logs` is one shot and prints a slice; `MERIDIAN_SINCE` widens it, and anything
after the subcommand is handed to `docker compose logs`, so a service name
narrows it. `watch` follows, in a tmux window of its own, the way
`scripts/dev.sh` with no arguments is the laptop's.

`watch` reconnects in a loop, which is what makes both the connection and the
service argument safe. ssh dies with the wifi, with a lid closing, with a
sleep, and `logs -f` given a service dies with that container — so the next
`ship` ends it too. Neither is anything having gone wrong, and both are the
same two-second gap: the window comes back on the new container having missed
only the swap. It does not bring the stack up first, on purpose: a window
opened to read logs should not be the thing that decides the box is running.

## The web app and the site

Two containers, `web` and `site`, each a `caddy:2-alpine` with files in it and
nothing else. `deploy/web/` and `deploy/site/` hold each one's Dockerfile, its
ignore file and its Caddyfile. Both are built from the root of the repository,
because both import the design language from `sketches/shared`, and each has an
ignore file of its own because the root `.dockerignore` is the server's.

**The web app** is `trunk build --release`. Which server it calls is decided
when it compiles — `MERIDIAN_SERVER`, read by `option_env!` in
`crates/web-main/src/io/meridian.rs` — and the Dockerfile sets it to
`https://api.meridiancash.io`. The box's `.env` cannot change it.

trunk's own tools — wasm-bindgen, wasm-opt and tailwind — are pinned in the
Dockerfile and installed in layers of their own, and the build runs
`--offline`, so a deploy asks GitHub for nothing until one of those versions
changes. Two of them have to agree with the tree: wasm-bindgen with
`Cargo.lock`, tailwind with `crates/web-main/Trunk.toml`. Bumping either there
without the Dockerfile fails the build, naming the tool.

**The site** is `npm ci` and `npm run build`, which writes `out/`. The build
fetches Inter from Google Fonts, since `next/font` serves it from the site
itself.

Files named for their content — trunk's `-<hash>` files and `snippets/`, and
everything under Next's `/_next/static/` — are kept for a year as `immutable`.
Everything else, `index.html` and the site's pages among them, is `no-cache`,
which a browser checks on every load. `/linked.html`, where Plaid sends the
bank's window back on the web, is served as it is and never kept. Everything
that compresses is brotli'd when the image is built, so the box never
compresses on a request.

The proxy adds HSTS and `nosniff` to both names, and `X-Frame-Options: DENY`
to the app. The server's `WEB_ORIGINS` has to be `https://app.meridiancash.io`
exactly, or the browser refuses every call the app makes.

**Markets is not ready for this.** Its chart asks Yahoo through the page's own
origin, at `/v8/*` and `/v1/*`, which `trunk serve` forwards in development and
nothing forwards here. It is locked in the web app's nav for now; when it
unlocks, `deploy/web/Caddyfile` needs those two paths proxied to
`query1.finance.yahoo.com`.

**The legal pages are not finished.** `site/lib/legal.ts` has six blanks only a
person can fill (#56), and the pages render them as written.

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

Both are live right now and both are inert, because `MERIDIAN_TEAM_ID` is
unset — the file then names the literal `TEAMID`, which owns nothing — and
`PLAID_REDIRECT_URI` is empty, so no link token points a bank here.
That is the correct state: telling Plaid a native app opened the session
without giving the bank a reachable address strands the user in their bank's
app *after* they have signed in.

Turning it on needs a paid Apple Developer account first, since the entitlement
that lets the app claim the link is not one a free profile grants — the failure
is explicit, and it is worth knowing what it looks like because it is the check
that the account is really active:

```
error: Cannot create a iOS App Development provisioning profile for
"com.meridian.phone". Personal development teams do not support the
Associated Domains capability.
```

`swift/Support/Meridian.entitlements` is what asks for it. Nothing signs with
that file — `ios-device.sh` still takes the app's entitlements out of the
profile, so the two cannot disagree — but `ios-provision.sh` builds its stub
against it, and a capability Apple is never asked for is one the profile does
not carry.

Both scripts read `MERIDIAN_TEAM_ID` for the same reason the box does. Joining
the Developer Program does not remove the free personal team, so from then on
there are two of everything — two certificates both called "Apple Development",
two profiles for this same bundle id — and the personal one grants nothing.
Naming the team is what stops a build picking it.

1. `export MERIDIAN_TEAM_ID=…` — the ten characters under Membership at
   developer.apple.com/account — then `./scripts/ios-provision.sh`. It asks
   Apple to enable **Associated Domains** and **Keychain Sharing** on the App
   ID and comes back with a profile that grants both
2. Set the same `MERIDIAN_TEAM_ID` on the box and redeploy — `up`, not
   `restart` — then read the file, body and all:
   `curl -s https://api.meridiancash.io/.well-known/apple-app-site-association`
   must be `content-type: application/json` with no redirect, and its `appIDs`
   must read `TEAMID.com.meridian.phone` with the real ten characters. A 200 is
   not on its own evidence the team took: the file is served whether or not the
   variable reached Caddy, and `TEAMID.com.meridian.phone` is what it says when
   it did not
3. Register `https://api.meridiancash.io/link/oauth` in the Plaid dashboard
4. Set `PLAID_REDIRECT_URI` to that same address and bring the server up. The
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

The timer is the box's own — `deploy/box/backup.sh`, run daily at 06:17 UTC
by `meridian-backup.timer` (since 2026-09-21). It takes the same `pg_dump
-Fc`, encrypts it with `age`, and uploads it to the `meridiancash-backups`
Space in nyc3, where a lifecycle rule expires dumps after 30 days.
`systemctl status meridian-backup` on the box says whether last night's ran;
the timer is `Persistent`, so a box that was down at 06:17 takes the dump
when it comes back.

**The box can write backups it cannot read.** `.env` carries only the age
*public* key (`BACKUP_AGE_RECIPIENT`); the private key is in the password
manager. The Spaces key beside it (`SPACES_ACCESS_KEY`/`SPACES_SECRET_KEY`,
named `meridian-droplet-backups` in DO) is scoped to this one bucket, so it
opens nothing else.

**The bucket shares the DO account with the droplet, and that is accepted.**
A second vendor would survive the account itself being lost, and a Spaces key
that can write can also delete, so someone on the box could wipe the bucket.
Accepted 2026-09-21 at this size; the by-hand laptop dump above is the copy
that lives outside the account, and the `age` layer means a leaked bucket is
ciphertext either way.

**Restoring needs two keys from the password manager** — the age private key
to open the archive, and `MERIDIAN_DB_KEY` to make the sealed access tokens
inside it mean anything. Neither is *in* a backup, deliberately: a dump that
carried its own keys would undo both layers. The restore has actually been
run, 2026-09-21, from the bucket into a throwaway Postgres as if the box were
gone:

```sh
aws s3api get-object --bucket meridiancash-backups --key meridian-STAMP.dump.age \
    --endpoint-url https://nyc3.digitaloceanspaces.com out.dump.age
age -d -i meridian-backup-age.key out.dump.age > out.dump
docker run -d --rm --name restore -e POSTGRES_PASSWORD=throwaway postgres:17-alpine
docker exec -i restore pg_restore -U postgres -d postgres --create --no-owner < out.dump
```

**Every secret a restore needs is in the password manager** — the age private
key, `MERIDIAN_DB_KEY`, the Plaid secret and the Postgres password, escrowed
2026-09-23 (#54). Losing the box loses no key.

## Encryption at rest

Postgres writes to the `meridian-db` Block Storage Volume, mounted at
`/mnt/meridian-db` and bound into the container by `docker-compose.prod.yml`
— the second of that file's two exceptions to "this file only adds". Block
Storage Volumes are LUKS-encrypted by DigitalOcean with a key they hold, and
that is the one place their two storage products differ: a droplet's local
disk is not encrypted at rest, which is where the database lived until
2026-09-21 as a risk accepted while everything was Sandbox.

Three residues of the move, none urgent and none invisible:

- The old named volume's blocks sit freed on the droplet's disk until
  something overwrites them, and the weekly droplet snapshots taken before
  the move hold the plaintext database until they age out of DO's four-week
  retention — gone by 2026-10-19.
- Droplet backups snapshot the *droplet's* disk, so they no longer cover the
  database at all. The nightly encrypted dump to Spaces (above) is what
  does, and it landed first on purpose. Volume snapshots exist but are not
  scheduled by DO; nothing schedules them here either.
- A LUKS layer of our own on top of the Volume would buy protection from
  DigitalOcean themselves and cost a passphrase at every unattended reboot.
  Decided against, for that reason rather than for the effort.

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

- **Pre-2026-09-21 droplet snapshots hold the database in plain text** until
  they age out, by 2026-10-19 (#33 — the database itself is on an encrypted
  Volume now, and the server does not talk to Postgres in plain text either,
  #34; the sections above have both).
- **An OAuth bank has nowhere to hand control back to.** The server half is
  deployed and switched off; what is missing is the Apple Developer account
  that lets the app claim the link. The section above has the four steps.
