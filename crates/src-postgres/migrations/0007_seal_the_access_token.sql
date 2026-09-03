-- The access token stops being readable in the database.
--
-- It is permanent, it reads somebody's bank account, and until now it sat here
-- as text. Issue #21 is the whole argument; the short version is that a stolen
-- volume, a leaked backup or a pg_dump in a ticket is how this data actually
-- escapes, and against all of those a sealed column is noise.
--
-- BYTEA rather than TEXT holding base64, because what goes in is bytes: a
-- version byte, a nonce and AES-256-GCM's ciphertext and tag. Base64 would be
-- a third larger and would invite somebody to read it as a string.

-- Every existing row is deleted rather than re-encrypted in place.
--
-- Re-encrypting means a migration that rewrites every row and a window in which
-- both formats exist, and then code that can read both -- forever, because the
-- day it is safe to delete that path is a day nobody schedules. There is no
-- Production Item yet and Sandbox ones cost nothing, so the honest price is
-- relinking the test banks once.
--
-- accounts, entries and streams are ON DELETE CASCADE from items, so this
-- takes the ledger with it. That is correct: those rows were synced with a
-- token nothing can use any more.
DELETE FROM items;

ALTER TABLE items DROP COLUMN access_token;

-- NOT NULL, which the DELETE above is what makes possible. A nullable column
-- would mean "this Item has no token", and an Item without a token is not a
-- connection at all -- it is a row that every read has to remember to skip.
ALTER TABLE items ADD COLUMN access_token BYTEA NOT NULL;
