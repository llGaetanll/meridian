-- A name is not optional.
--
-- 0013 made the column nullable because every account that existed had none and
-- there was nowhere to get one from. There were a handful. Carrying "might have
-- no name" through the store, the wire, the machine and every view for the rest
-- of the app's life is a worse price than filling them in once.
--
-- The backfill is the address's local part. That is a starting value and not an
-- answer — it is only ever what somebody typed in front of an @ — but it is
-- never empty, it is theirs, and it is one edit away from being right. New
-- accounts are seeded the same way when the row is made, because the account
-- exists from the moment a code is verified and the road does not ask for a
-- name until the payment is settled. Nothing is ever without one in between.
UPDATE users
   SET name = split_part(email, '@', 1)
 WHERE name IS NULL OR btrim(name) = '';

ALTER TABLE users ALTER COLUMN name SET NOT NULL;
