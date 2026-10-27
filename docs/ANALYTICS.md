# Analytics

PostHog measures how the site and the app are used. The project is
**Meridian**, id `630628`, on PostHog Cloud US —
[us.posthog.com](https://us.posthog.com/project/630628). This document is the
decisions; the privacy policy at `site/app/privacy/page.tsx` is the promise
made to users, and **the two change in the same pass** — the policy names what
is collected, who holds it, and for how long, so a change to any of those here
is a change to the policy.

## The one rule

**No part of the ledger ever enters analytics.** No balance, transaction,
merchant, holding, institution name, email address or typed text — not in an
event name, not in a property, not in a recording. Analytics carries *that*
something happened, never *what the money was*. The privacy policy states this
outright, so a violation is not a bug but a broken promise.

## Production only

The project API key is public by design — it can write events and read
nothing — and is baked into production images as a build `ARG` with a default,
the same way `MERIDIAN_SERVER` is in `deploy/web/Dockerfile`. Development
builds (`npm run dev`, a plain `npm run build`, trunk) carry no key and load
no analytics, which is what keeps development traffic out of the numbers.
There is no second "dev" PostHog project.

## The relay

Events travel first-party: the page sends to `/relay/*` on its own origin, and
the box's `Caddyfile` forwards that to PostHog's ingestion host with the `Host`
header rewritten and the visitor's cookies stripped. An ad blocker that names
`*.posthog.com` never sees that host here, which is worth roughly a third of
the attribution data. The `/relay/static/*` branch is PostHog's assets host,
which serves the lazily-loaded recorder script.

## What runs where

- **The site** (`site/`, meridiancash.io) — `posthog-js`, initialized in
  `site/components/analytics.tsx`. Autocapture on: pageviews, clicks, web
  vitals, and session recording. This is where ads land, so campaign
  attribution (UTM parameters, referrer) is captured here automatically; tag
  every ad link with `utm_source` / `utm_campaign`.
- **The web app** (`crates/web-main`, app.meridiancash.io) — not yet
  instrumented; the second pass. The plan is below.
- **The server** (`crates/server-main`) — not yet instrumented; also the
  second pass.

## Session recording

Recording is allowed for `https://meridiancash.io` **only**, enforced
server-side by the project's `recording_domains` setting rather than by
anything a client does — so the app can share the key without a recording ever
existing of somebody's finances. Typed input is masked by PostHog's default
before it leaves the browser. Retention is 30 days.

If `recording_domains` is ever widened, the privacy policy's recording
paragraph is wrong the moment it takes effect. Don't.

## Identity

Site visitors are anonymous, identified by a random id in a cookie set on
`.meridiancash.io` — shared across subdomains, which is what will let an ad
click on the site connect to a signup in the app. When the app is
instrumented, `identify()` is called with the **account UUID**, never the
email; the pre-signup journey merges onto the person automatically.

## Naming

Event names are `snake_case`. App events will map 1:1 onto
`client_state::EventUi` variants (`screen_opened`, `email_submitted`,
`bank_connect_requested`, …) so the dashboard cannot drift from the state
machine, with the onboarding funnel steps coming from
`state_onboarding::Step`. The variants that carry typed text
(`EmailTyped`, `CodeTyped`, `NameTyped`) are never captured — not even the
event name, since its cadence is keystrokes.

## The second pass

1. **The app**: one hook at `RuntimeApp::ui` (`crates/web-main`) capturing the
   allowed `EventUi` variants via the same `/relay` path on the app's vhost;
   no autocapture, no recording, no URL-based pageviews (the app has no
   router — `screen_opened` is the pageview).
2. **The server**: capture the funnel's ground truth from the handlers that
   own it — `code_verified`, `subscription_started` (from the Stripe
   webhook), `bank_linked`, `sync_finished` — keyed to the account UUID, so
   the funnel holds even for clients that block the relay.
3. **iOS/Android later**: the taxonomy is defined on `EventUi` in
   `client-state`, which the phones already share; only the transport is
   per-platform.

## Retention, and the numbers the policy states

The policy says: analytics events up to one year, recordings 30 days. One year
is PostHog's event retention on the free plan (paid plans keep seven years),
and 30 days is this project's recording retention setting. **Moving to a paid
PostHog plan or changing either setting means updating the policy first.**
