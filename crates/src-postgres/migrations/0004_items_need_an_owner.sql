-- 0003 left `items.user_id` nullable for rows that predated users. There is no
-- such data worth keeping, and a nullable owner on the column that decides who
-- sees what is a hole that only gets harder to close.
--
-- The delete cascades to accounts, entries and streams, which is the point:
-- an item nobody owns takes its history with it.

DELETE FROM items WHERE user_id IS NULL;

ALTER TABLE items ALTER COLUMN user_id SET NOT NULL;
