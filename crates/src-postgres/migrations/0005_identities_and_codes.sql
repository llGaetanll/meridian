-- How somebody proves who they are, and the codes they do it with.
--
-- 0003 put the address on `users` and made it unique there, which is right up
-- until the same person signs in with Apple. Then "which user is this?" has
-- two answers in two shapes, and account linking has nowhere to live.
--
-- So a credential is a row. Email today; ('apple', <sub>) and ('google', <sub>)
-- later are the same shape, and adding one is an INSERT rather than a
-- migration that reaches into `items`.

CREATE TABLE identities (
    -- 'email', and later 'apple' and 'google'.
    provider TEXT NOT NULL,

    -- Who the provider says this is. A lowercased address for 'email', the
    -- `sub` claim for the others — never the display address, which Apple can
    -- change out from under us with private relay.
    subject  TEXT NOT NULL,

    user_id  UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (provider, subject)
);

CREATE INDEX identities_user ON identities (user_id);

-- Every user so far arrived by email, and this is that credential said the new
-- way. `users.email` stays as the address we *write* to; this is the one we
-- authenticate against.
INSERT INTO identities (provider, subject, user_id)
SELECT 'email', lower(email), user_id FROM users;

-- Uniqueness of the credential now belongs to the primary key above. Two
-- accounts sharing a contact address is strange rather than wrong, and it is
-- no longer the thing that keeps logins apart.
DROP INDEX users_email_lower;

-- A code that has been sent and not yet used.
--
-- One live code per address, so asking again replaces rather than accumulates:
-- a table of every code ever sent is a table of guesses somebody can work
-- through.
CREATE TABLE login_codes (
    email       TEXT PRIMARY KEY,

    -- The SHA-256, never the code. A leaked database should not be a set of
    -- live logins, which is the same reason `sessions` stores a hash.
    code_sha256 BYTEA NOT NULL,

    expires_at  TIMESTAMPTZ NOT NULL,

    -- Wrong guesses so far. Capped in code, because the cap is a rule and this
    -- is only where the count is kept.
    attempts    INTEGER NOT NULL DEFAULT 0,

    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
