-- A bank connection, as Plaid calls it: one Item is one set of credentials at
-- one institution.
--
-- Nothing here is a design choice of ours. Plaid's shape forces all of it: the
-- access token never expires and is the credential every later call is made
-- with, and the cursor is the only record of how far the last sync got --
-- losing it means re-fetching an entire history.

CREATE TABLE items (
    -- Plaid's id for the Item, which is what its webhooks name it by. Ours as
    -- well, so a connection has one id rather than two that can disagree.
    item_id TEXT PRIMARY KEY,

    -- Permanent, and a live credential to somebody's bank account. Plain text
    -- for now; encrypted before the first Production Item is ever created.
    -- Tracked on issue #20.
    access_token TEXT NOT NULL,

    -- Plaid's id for the bank. The name people read comes back with the
    -- balances, so it is not copied here where it could go stale.
    institution_id TEXT,

    -- Where the last /transactions/sync reached. NULL means the next sync
    -- starts from the beginning, which is correct and merely slow.
    cursor TEXT,

    -- Plaid's error_code when the connection needs a person, most often
    -- ITEM_LOGIN_REQUIRED. NULL is healthy.
    error_code TEXT,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- No owner column yet, because there are no users yet. It arrives as its own
-- migration, which is the whole reason this directory is numbered.
