-- A code that confirms an account deletion, sent and not yet used.
--
-- Its own table rather than a row in `login_codes`: that one is keyed by
-- address with one live code per address, so a deletion code there would
-- overwrite a sign-in half way through. Keyed by the user because the route
-- that asks is authenticated, and cascaded so the code goes with the account
-- it was for.
CREATE TABLE delete_codes (
    user_id     UUID PRIMARY KEY REFERENCES users(user_id) ON DELETE CASCADE,

    -- The SHA-256, never the code, for the reason `login_codes` gives.
    code_sha256 BYTEA NOT NULL,

    expires_at  TIMESTAMPTZ NOT NULL,

    -- Wrong guesses so far. Capped in code, because the cap is a rule and this
    -- is only where the count is kept.
    attempts    INTEGER NOT NULL DEFAULT 0,

    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
