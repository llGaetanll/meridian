-- A Link session can repair an existing Item instead of attaching a new one —
-- Plaid's update mode. This is which Item, and NULL is the ordinary session
-- that attaches.
--
-- It has to be written down rather than remembered: a repair session's finish
-- must not be exchanged for a new Item — the exchange is the call that spends
-- a Production slot — and the process deciding that may not be the one that
-- opened the session.

ALTER TABLE link_sessions
    ADD COLUMN item_id TEXT REFERENCES items(item_id) ON DELETE CASCADE;
