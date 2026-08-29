-- A Link session that has been opened and not yet collected.
--
-- It exists to bind the `link_token` to the person who asked for it. The token
-- is not a credential — anyone holding one can open Plaid's page, and whatever
-- they sign into becomes *their* Item — but the `public_token` it later yields
-- is, and without this row there would be nothing stopping one account
-- claiming the bank another account just linked.
--
-- Rows are short-lived. A link token expires in about thirty minutes and Plaid
-- holds the finished session's result for six hours, so nothing here is worth
-- keeping past a day.

CREATE TABLE link_sessions (
    link_token TEXT PRIMARY KEY,

    user_id    UUID NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,

    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- Set once the session has been collected, successfully or not. Kept
    -- rather than deleted so that a second collection of the same session is
    -- a no-op instead of a second Item.
    collected_at TIMESTAMPTZ
);

CREATE INDEX link_sessions_user ON link_sessions (user_id);
