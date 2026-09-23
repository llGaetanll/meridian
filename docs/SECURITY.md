# Security

The written security program, the risk assessment, and the incident response
plan, in that order — the three documents GLBA's Safeguards Rule requires
(#16) and Plaid's security questionnaire asks for (#43). Written 2026-09-19.

Everything here is true as written or names the issue where it is not yet.
**A claim in this document that the system does not honour is worse than no
document**, so when the system changes, this changes in the same commit.

## The security program

Meridian is built and operated by one person, Gaetan Almela, under Meridian
Studios LLC. There is no one else: the developer, the operator, and the
security officer are the same person, and this program is written for a
company of that size rather than pretending otherwise.

**What is held.** Bank and brokerage balances, transactions, merchant names,
email addresses, and Plaid access tokens, for US users only. One Postgres
database on one DigitalOcean droplet. Subprocessors are Plaid, Stripe, Resend,
and DigitalOcean, named in the privacy policy at `meridiancash.io/privacy`.

**In transit.** TLS on every hop. The public surface terminates at Caddy with
certificates it manages; the server reaches Postgres with
`sslmode=verify-full` against a CA of our own (`docs/DEPLOY.md`), so a
downgrade is a refusal rather than a fallback.

**At rest.** Plaid access tokens — the credential that reads a bank — are
sealed at the application level with AES-256-GCM before they reach Postgres
(`crates/src-postgres/src/postgres_secret.rs`). The database itself lives on
a DigitalOcean Block Storage Volume, LUKS-encrypted by DigitalOcean with a
key they hold (#33, closed 2026-09-21); droplet snapshots taken before that
date hold it in plain text until they age out, by 2026-10-19.

**Access.** ssh to the droplet is by key, from one authorized machine. There
is no admin panel, no support tooling, and no read of production data to
debug — a rule from #16, not an accident. Linking a bank requires a paid
subscription and the Items held are capped (#77), so exposure grows only
with paying customers. MFA on every console that can reach the data — DigitalOcean,
Plaid, Stripe, GitHub, Apple, email — is being verified door by door in #125.

**Secrets.** The box's secrets live in `/srv/meridian/.env`, readable only on
the box. The sealing key `MERIDIAN_DB_KEY`, the age backup key, the Plaid
secret, and the Postgres password are all escrowed in a password manager
(#54, closed 2026-09-23). Keys in a KMS rather than an env var is a known
step up, tracked in #124.

**Logging.** Every call crossing the network is logged, both halves, with
credentials stripped — a `secret` or `access_token` is logged as its length,
never its value (`docs/LOGGING.md`). Lines carry the commit that produced
them.

**Audit log.** Separate from the app logs, an append-only `audit_log` table
records every answer that serves customer data — whose data, what of it, from
what address, when — and every backup, which reads all of it. The table
refuses `UPDATE` and `DELETE` outright, rows survive account deletion, and the
nightly dump carries the log off the box, which is what makes a quiet rewrite
visible. Direct database access is not audited; the control there is that
production reads do not happen, plus MFA on the box (#125).

**Deletion.** A user can delete their account end to end — the ledger, the
Plaid Items, the Stripe customer — behind a confirmation and an emailed
six-digit code. Tested server-side (`crates/server-main/tests/flow_h_leaving.rs`).

**Backups.** A daily `pg_dump`, age-encrypted on the box with a public key it
cannot decrypt with, uploaded to a Spaces bucket with 30-day retention; the
restore has been run end to end from the bucket with only what the password
manager holds (`docs/DEPLOY.md`, Backups). Weekly droplet snapshots and a
by-hand laptop dump are the second and third copies. The bucket shares the
DigitalOcean account with the droplet — accepted 2026-09-21, revisited when
there are real users at scale.

**Review.** This program is re-read at every Plaid milestone — the
questionnaire, the application, the first real customer — and at least
yearly. Each pass gets a date at the top.

## The risk assessment

What can go wrong, ordered by how much it would matter. Each risk names its
control or its accepted gap; an accepted risk carries the date it was
accepted.

1. **The droplet is compromised.** The worst case, because #124 is real: the
   sealed tokens and the key that opens them share the box, so a root
   compromise reads banks. Contained by the small surface (one binary, Caddy,
   Postgres, nothing else), key-only ssh from one machine, and the Item cap;
   answered after the fact by the incident plan below. The KMS move (#124) is
   what shrinks this risk rather than containing it.
2. **A copy of the disk leaks** — a snapshot, a decommissioned drive, a
   backup somewhere it should not be. The database is on an encrypted Volume
   (#33) and every dump in the bucket is age-encrypted (#38), so this is
   reduced to key material leaking, which the password manager holds — plus
   the pre-move droplet snapshots that hold plaintext until 2026-10-19.
3. **A console account is taken over.** DigitalOcean is the box and its
   backups in one account; Plaid and Stripe are the data and the money.
   MFA everywhere is the control, verified in #125.
4. **The box is lost** — deleted, corrupted, region gone. Risk is
   availability alone, not data exposure: every secret a rebuild needs is in
   the password manager (#54), and #38's off-site dumps with a restore that
   has actually been run are the control.
5. **Data is misused rather than stolen.** The standing rules from #16: no
   selling or sharing, no transactions to third-party LLMs without zero
   retention, no tokens in logs, no data kept after deletion, no scopes
   beyond what is used. One person means no insider to guard against, but
   also no second pair of eyes — which is why every served read and every
   backup lands in the append-only audit log, and why production is not read
   directly at all.

## The incident response plan

One person, so the plan is a checklist rather than an org chart. It exists to
be followed on a bad day, in order.

1. **Contain first.** `./scripts/deploy.sh down` stops the server and closes
   the public surface in one command. Nothing about a deploy needs the box to
   be trusted: images rebuild from git on a laptop.
2. **Revoke what reads banks.** Rotate the Plaid secret in the dashboard; if
   tokens may have leaked, remove every Item (`/item/remove`), which kills
   the tokens at Plaid regardless of who holds them. Users relink; that is
   the acceptable cost.
3. **Rotate the rest.** The Postgres password, `MERIDIAN_DB_KEY` (a reseal,
   possible while the old key is still held), ssh keys, and any console
   password involved.
4. **Establish scope from the logs.** Every network call is logged with its
   commit; the question "what was read, by whom, since when" is answered
   there and in Postgres, not from memory.
5. **Notify.** Plaid, per its contract, promptly. Users whose data was
   exposed, plainly and quickly. The FTC within 30 days if 500 or more users
   are affected (Safeguards Rule); state breach-notification laws by the
   user's state. Under 500 users total today, the practical duty is Plaid
   and the users themselves.
6. **Recover.** Redeploy from git (`./scripts/deploy.sh ship`), restore the
   database from the most recent dump, verify with
   `./scripts/deploy.sh health`.
7. **Write it down.** What happened, when it was noticed, what was done, and
   what changes — appended to this document. An incident that leaves no
   written trace will be repeated.
