-- Who is paying, and until when.
--
-- One row per user, because the product is one subscription. A user with no
-- row has never paid; a user whose `current_period_end` is in the past paid
-- once and does not now. Both are the same answer to the only question this
-- table is asked, which is whether to serve somebody their ledger.
--
-- The row is written from what the provider says, and nothing else writes it.
-- A checkout that appears to succeed is not a payment: the card can fail after
-- the form closes, the renewal can fail a month later, and a chargeback can
-- undo one that worked. Stripe says so in a webhook and Apple in a server
-- notification, and those are the only things that move `current_period_end`.

-- Which rail proved the payment.
--
-- Here from the first migration rather than added when StoreKit lands: iOS
-- sells through Apple and cannot sell through Stripe, so a second provider is
-- known to be coming. Adding it later means a migration that reaches into rows
-- somebody is being billed against, which is the wrong place to be careful.
CREATE TYPE billing_provider AS ENUM ('stripe', 'apple');

CREATE TABLE subscriptions (
    user_id UUID PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,

    provider billing_provider NOT NULL,

    -- The provider's name for this person. `cus_…` for Stripe; for Apple the
    -- `originalTransactionId`, which is the identifier that survives a
    -- renewal, a resubscribe and a refund.
    --
    -- Unique per provider so that two accounts cannot end up sharing one
    -- payer. On Apple that is not a nicety: one Apple ID resubscribing must
    -- land on the account that already had it, or a person pays twice and sees
    -- an empty app.
    customer_id TEXT NOT NULL,

    -- The subscription itself. `sub_…` for Stripe. Null for Apple, where the
    -- transaction id above is the whole of the identity.
    subscription_id TEXT,

    -- The provider's word, stored as it arrived: 'active', 'past_due',
    -- 'canceled', and whatever they add next.
    --
    -- TEXT rather than an enum, unlike `provider`. The set is theirs and they
    -- extend it without asking; a status arriving that Postgres refuses to
    -- store would fail the webhook that carried it, and a webhook that cannot
    -- be stored is retried until it is dropped. What the app *does* about a
    -- status is decided in the state machine, which is where a value nobody
    -- has seen before can be treated as "not paid" without losing the record
    -- of what was actually said.
    status TEXT NOT NULL,

    -- The end of the period that has been paid for. Entitlement is this
    -- against the clock, and nothing else: a status of 'active' with a date in
    -- the past is not entitlement, and 'canceled' with a date in the future is
    -- — that is what cancelling at period end means, and the terms promise it.
    --
    -- Nullable because a subscription exists before its first payment lands.
    -- Null is "has never been paid for", which is not entitled.
    current_period_end TIMESTAMPTZ,

    -- Whether it stops at the end of that period. Not what decides access —
    -- the date above does — but what the app says on the account screen, so
    -- somebody who cancelled is not told they will be billed again.
    cancel_at_period_end BOOLEAN NOT NULL DEFAULT false,

    -- The last event this row was written from, and when the provider made it.
    --
    -- Both exist for the same reason: webhooks arrive more than once and out
    -- of order. Stripe retries until it is answered, so the same event id can
    -- land twice; and a retry of an older event can arrive after a newer one,
    -- which without a comparison would write a stale status over a fresh one.
    -- So a write happens when the id differs and the timestamp is not older.
    last_event_id TEXT,
    last_event_at TIMESTAMPTZ,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- One payer, one account. See `customer_id` above.
CREATE UNIQUE INDEX subscriptions_provider_customer
    ON subscriptions (provider, customer_id);

-- A webhook arrives naming the subscription and not the user, so this is the
-- index from what Stripe says to whose row it is.
CREATE UNIQUE INDEX subscriptions_subscription
    ON subscriptions (subscription_id)
    WHERE subscription_id IS NOT NULL;
