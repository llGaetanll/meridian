-- The ledger itself: the accounts, the transactions and the recurring streams
-- an Item's history is made of.
--
-- These arrive together with the cursor being used for the first time, and
-- that is not a coincidence. /transactions/sync answers with what changed
-- *since* the cursor, so a saved cursor and no saved transactions gives, on
-- the next boot, an empty ledger and a cursor that correctly returns nothing.
-- Every log line would still say the sync succeeded. docs/PLAID.md calls this
-- one of the ways to be quietly wrong.

-- 0001 said the institution's name always arrives with the balances, which is
-- true right up until a cold start that has rows but has not called Plaid yet
-- — and then the app draws "Bank".
ALTER TABLE items ADD COLUMN institution_name TEXT;

-- Money is stored in minor units, as whole numbers. Doubles accumulate error
-- across arithmetic, and a durable record should not be a second source of it.
-- The app still computes in f64, so this does not make the *app* exact; it
-- makes what is written down exact and round-trip it without loss.
--
-- "Minor" rather than "cents" because not every currency has two decimal
-- places — JPY has none and KWD has three. Everything here is USD today, and a
-- non-USD holding needs a currency column on accounts, which is a later
-- migration rather than a guess now.

CREATE TABLE accounts (
    -- The source's own id, stable across syncs.
    account_id      TEXT PRIMARY KEY,

    item_id         TEXT NOT NULL REFERENCES items(item_id) ON DELETE CASCADE,

    -- The bank's name for it, e.g. "Plaid Checking".
    name            TEXT NOT NULL,

    -- The source's raw subtype: "checking", "401k", "home equity". Not worded
    -- — "401(k)" is a decision about what a screen says, and phone-state makes
    -- those. Keeping it raw means changing that wording changes what a row
    -- written months ago says, with no migration.
    subtype         TEXT,

    -- "depository", "investment", "credit", "loan", "other". This is what says
    -- whether a balance is money held or money owed; the sign never does, and
    -- getting it wrong adds a mortgage to somebody's net worth.
    account_group   TEXT NOT NULL,

    -- The last four digits, absent on some accounts.
    mask            TEXT,

    -- Always positive, as the bank reports it. See account_group.
    balance_minor   BIGINT NOT NULL,

    -- Spendable now. Stored raw: dropping it when it merely equals the balance
    -- is a decision about the row, made on the way out.
    available_minor BIGINT,

    -- A credit line's ceiling.
    limit_minor     BIGINT,

    -- A rate, not money, so not minor units.
    apy             DOUBLE PRECISION
);

CREATE TABLE entries (
    -- The source's transaction id, so a modified transaction replaces its old
    -- self rather than joining it.
    id           TEXT PRIMARY KEY,

    -- Which bank it came through. Not a field on Entry, and the one column
    -- here that is ours: account_id is optional, so without this an entry with
    -- no account could not be attributed to a bank — and removing a bank would
    -- not take its history with it, which the app promises in as many words.
    item_id      TEXT NOT NULL REFERENCES items(item_id) ON DELETE CASCADE,

    -- Not a foreign key: Entry.account_id is optional, and a source may name
    -- an account we were never told about.
    account_id   TEXT,

    ts           TIMESTAMPTZ NOT NULL,

    -- What amount_minor is minor units of.
    currency     TEXT NOT NULL,

    -- Ledger convention, already flipped: negative left the account. Stored
    -- converted rather than raw because negating is not idempotent — doing it
    -- on the way in and again on the way out cancels, and every expense
    -- becomes income. docs/PLAID.md, trap one.
    amount_minor BIGINT NOT NULL,

    -- The merchant where the source knew one, the raw bank description
    -- otherwise.
    label        TEXT NOT NULL,

    -- The source's own primary category, e.g. "FOOD_AND_DRINK". "UNKNOWN" is
    -- ours, for anything we did not recognise.
    category     TEXT NOT NULL,

    logo_url     TEXT,

    -- Authorised but not settled. Provisional: the amount can still change and
    -- the whole row can be replaced.
    pending      BOOLEAN NOT NULL,

    -- The pending entry this posted one replaces. Deliberately not a foreign
    -- key: it points at a row that is about to be deleted, which is the point
    -- of it.
    supersedes   TEXT
);

-- The ledger is only ever read newest-first.
CREATE INDEX entries_ts_desc ON entries (ts DESC);

-- And swept per bank, when one is removed or resynced.
CREATE INDEX entries_item ON entries (item_id);

CREATE TABLE streams (
    id           TEXT PRIMARY KEY,

    item_id      TEXT NOT NULL REFERENCES items(item_id) ON DELETE CASCADE,

    label        TEXT NOT NULL,

    category     TEXT NOT NULL,

    -- Already flipped, as on entries. Negative is money out.
    amount_minor BIGINT NOT NULL,

    -- The source's raw cadence, e.g. "MONTHLY". Worded on the way out.
    frequency    TEXT NOT NULL,

    next_date    TIMESTAMPTZ
);

CREATE INDEX streams_item ON streams (item_id);

-- No logo_url and no is_active. Plaid's stream response carries no logo, and
-- an inactive stream is dropped before it ever reaches us — a column that can
-- only ever hold one value is a column that misleads whoever reads the schema
-- next.
