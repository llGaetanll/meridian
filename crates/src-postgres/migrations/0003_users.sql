-- Users, and the sessions that identify them on a request.
--
-- Everything before this migration was written as though the server had one
-- user, because it did. The tell was `load_entries` and `load_items` having no
-- WHERE clause at all: with one user that reads as "everything I know", and
-- with two it reads as "everyone's transactions, to whoever asked".

CREATE TABLE users (
    user_id    UUID PRIMARY KEY DEFAULT gen_random_uuid(),

    -- As typed. Uniqueness is case-insensitive below, because nobody thinks of
    -- Alex@example.com as a second account, but the capital is theirs to keep.
    email      TEXT NOT NULL,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX users_email_lower ON users (lower(email));

-- What the phone sends on every request once it has signed in.
--
-- A row rather than a signed token, so that signing out is a DELETE and not a
-- wait for an expiry. The token itself is never stored: only its SHA-256, so
-- that a leaked database is not a set of live logins.
CREATE TABLE sessions (
    token_sha256 BYTEA PRIMARY KEY,

    user_id      UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,

    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- NULL never expires. Nothing issues one of those today; the column exists
    -- so that a session with a lifetime does not need a migration.
    expires_at   TIMESTAMPTZ
);

CREATE INDEX sessions_user ON sessions (user_id);

-- Whose bank it is. Nullable, because rows exist from before there were users
-- and inventing an owner for them would be a lie in the one column that
-- decides who sees what. The NOT NULL follows once they are claimed or gone.
--
-- Only items carries this. accounts, entries and streams all hang off item_id,
-- so scoping flows through the join and there is one place that says who owns
-- what — a second copy is a second thing to get wrong.
ALTER TABLE items ADD COLUMN user_id UUID REFERENCES users(user_id) ON DELETE CASCADE;

CREATE INDEX items_user ON items (user_id);
