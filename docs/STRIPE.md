# Stripe

Stripe is how a subscription is paid for on the web and the desktop. The phone
does not use it: iOS sells through StoreKit, and what reaches the server from a
phone is Apple's word for a purchase rather than a card. `src-stripe` talks to
Stripe the same shape as the other sources: plain async functions over plain
data, one request each, no decisions.

## Test mode only

**Any agent working on Stripe uses test mode. Only the user switches to live,
and only by saying so.**

Stripe has one host for both modes. The key alone picks which one a call lands
in, and a live key works perfectly while charging a real card — so nothing in
the code can tell the two apart by behaviour. `STRIPE_ENV` is what tells them
apart: **required**, matched exactly, `test` or `live`, and a key whose prefix
disagrees with it fails the process on the way up.

A price and a webhook endpoint each exist in one mode only. The four Stripe
variables change together, or a server talks to live Stripe about a test
price.

## Credentials

```sh
export STRIPE_ENV=test               # required; `test` or `live`, exactly
export STRIPE_SECRET_KEY=sk_test_…   # its prefix has to agree with STRIPE_ENV
export STRIPE_PUBLISHABLE_KEY=pk_…   # the same; handed to the card form
export STRIPE_PRICE_ID=price_…       # the $5.99/month price, in this mode
export STRIPE_WEBHOOK_SECRET=whsec_… # the endpoint's, or what `stripe listen` prints
```

The secret key can start a charge against any customer on the account, so
every call that carries it is the server's. The publishable key is made to be
in a page; the server hands it out beside each subscription's secret so no
client is built with one.

The webhook secret is different for a registered endpoint and for `stripe
listen`, and a webhook signed by one does not verify against the other.
Development runs `stripe listen --forward-to localhost:PORT/stripe/webhook`
and uses what it prints.

## The API version is pinned

`src_stripe::API_VERSION` is sent on every call, and the webhook endpoint is
registered with the same version. An endpoint registered without one sends
whatever the account defaults to, so the objects this crate parses would come
from two versions at once. **An endpoint's version is set at creation and
never again**: raising the constant means registering a new endpoint, taking
its secret, and disabling the old one.

## The flow

```
client: POST /billing/subscribe          -> client_secret, publishable_key
server:   POST /v1/customers               one per user, reused on a second press
server:   POST /v1/subscriptions           default_incomplete, nothing charged yet
client: Stripe Elements confirms the card  the card never touches our server
stripe: POST /stripe/webhook             -> customer.subscription.updated
```

**A subscription's truth arrives by webhook and nowhere else.** The card form
closing says a card was accepted, which is not the same thing: the card can
fail after the form closes, the renewal can fail a month later, and a
chargeback can undo one that worked. So `/billing/subscribe` writes the
customer and subscription ids and no entitlement, and the webhook is what
opens the app.

## What entitlement is

One date: the end of the period paid for, read off the subscription's item.
`Entitlement::allows` compares it with the clock and reads nothing else — a
`canceled` subscription with a date in the future still opens the app, because
that is what cancelling at period end means and the terms promise it.

The status is kept beside the date for the account screen and the paywall's
sentence, and it changes the date in three cases: `unpaid` and
`incomplete_expired` are worth nothing from now, whatever the period says, and
`incomplete` has no date at all — Stripe reports its period a month out from
the moment it is created, before any card, and the `created` webhook carries
exactly that. `past_due` is deliberately not one of them — Stripe is still retrying, the
period was paid for, and the ordinary cause is an expired card that a smart
retry fixes within the hour.

`incomplete` is a subscription started and not paid for: a card form opened
and left, or a 3D Secure challenge closed before the bank answered. Stripe
holds its invoice open for 23 hours and then sends `incomplete_expired`. In
that window `/billing/subscribe` hands the same invoice's secret back rather
than starting a second subscription, so the paywall says the subscription is
unfinished and Subscribe picks the form back up. The invoice events Stripe
sends around a challenge (`invoice.payment_action_required`) say nothing the
client did not already learn from the card form, and the endpoint does not
subscribe to them.

## Webhooks

The endpoint is registered for three events and no others:

| Event | What happens |
| --- | --- |
| `customer.subscription.created` | Written. Status `incomplete` on a fresh one, so it entitles nobody |
| `customer.subscription.updated` | Written. This is a payment landing, a renewal, a cancellation, and a failed renewal |
| `customer.subscription.deleted` | Written. The app closes at once |

The machine reads the object in the body rather than the event name: a
subscription is written, and anything else is answered 2xx and dropped.
Refusing an event only asks Stripe to send it again.

### Why no invoice events

**The subscription event carries everything the machine acts on, and the
invoice events beside it carry nothing it does not.** This was observed on a
test clock rather than assumed, and it agrees with Stripe's documentation,
which says `customer.subscription.updated` is sent when a subscription
"starts or changes — for example, renewing a subscription".

A successful renewal, in the order it arrived:

```
invoice.upcoming
invoice.created                   the new month's invoice, draft
customer.subscription.updated     status active, items[0].current_period_end
                                  already the NEW period; the old one is in
                                  previous_attributes
charge.succeeded
invoice.paid                      a few seconds after the subscription event
```

A renewal whose card was declined:

```
invoice.upcoming
invoice.created
customer.subscription.updated     status active, the new period — before any
                                  charge is attempted
charge.failed
invoice.payment_failed
customer.subscription.updated     status past_due, the same new period, in
                                  the same second as the invoice event
```

Two things follow. The period rolls forward **before** the payment is
attempted, on the subscription event; the invoice says afterwards whether it
was paid, and a failed one arrives beside a subscription event saying
`past_due`. And a subscription that fails its renewal holds a period a month
out with status `past_due`, which by the rule above keeps the app open while
Stripe retries. When the retries run out, `unpaid` or `canceled` arrives the
same way and closes it.

That is the design this code chose. The other one — access tracks money
actually received, extended on `invoice.paid` — is what Stripe's "track active
subscriptions" guide describes, and it is not what is built. Choosing it would
mean subscribing to the invoice events again and reading the period off the
invoice's line.

### Order is not guaranteed

Stripe says so outright, and `created` is only accurate to the second. The
write takes an event only if its id is new and its timestamp is not older
than what is stored — `<=`, so two events sharing a second both land. The
route answers in the turn the signature checks out, before the write, because
Stripe retries anything not answered in about twenty seconds.

### Believing one

Every webhook is verified before anything is decided: the `Stripe-Signature`
header's `v1` is an HMAC over `t.body` with the endpoint secret, and `t` has to
be within `TOLERANCE_SECS` of now. The raw body bytes are what is signed, so
the body is not parsed until the signature has matched. Every failure answers
401 and the reason is in the log.

## Watching a renewal yourself

A test clock is a customer whose time you control. This is what produced the
orders above, and it costs nothing:

```sh
export STRIPE_API_KEY=$STRIPE_SECRET_KEY       # a test key
NOW=$(date +%s)
CLOCK=$(stripe test_helpers test_clocks create -d frozen_time=$NOW | jq -r .id)
CUS=$(stripe customers create -d test_clock=$CLOCK -d payment_method=pm_card_visa \
        -d "invoice_settings[default_payment_method]=pm_card_visa" | jq -r .id)
stripe subscriptions create -d customer=$CUS -d "items[0][price]=$STRIPE_PRICE_ID"
stripe test_helpers test_clocks advance $CLOCK -d frozen_time=$((NOW + 32*24*3600))
stripe events list --limit 100                  # once the clock reads `ready`
stripe test_helpers test_clocks delete $CLOCK --confirm
```

For a failed renewal, attach `pm_card_chargeCustomerFail` and make it the
customer's default *after* the subscription is created — that card fails on
the first charge too, so a subscription created against it never becomes
active. Deleting the clock deletes the customer and everything on it.

## Registering the endpoint

By the API, which returns the signing secret rather than making you reveal it
in the dashboard:

```sh
curl -X POST https://api.stripe.com/v1/webhook_endpoints \
  -u "$STRIPE_SECRET_KEY:" \
  -d "url=https://api.meridiancash.io/stripe/webhook" \
  -d "api_version=2026-08-26.dahlia" \
  -d "enabled_events[]=customer.subscription.created" \
  -d "enabled_events[]=customer.subscription.updated" \
  -d "enabled_events[]=customer.subscription.deleted"
```

`api_version` must match `src_stripe::API_VERSION`, and it cannot be changed
afterwards. The `secret` in the answer is `STRIPE_WEBHOOK_SECRET`. The
endpoint exists in the mode of the key that made it.
