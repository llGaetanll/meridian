-- Who read customer data: one row per read, and rows only ever arrive.
--
-- Separate from the app logs on purpose. A log line says what the server did
-- and rotates away; this table answers "who has seen this person's data" for
-- as long as the question can be asked, and it leaves the box every night
-- inside the encrypted dump — which is what makes a quiet rewrite visible.
--
-- No foreign key to users, and no cascade: an account's deletion takes the
-- data and leaves the record of who read it. The row holds nothing but ids
-- and names, so what survives is the fact of the access, not the data.
CREATE TABLE audit_log (
    at      TIMESTAMPTZ NOT NULL,

    -- Whose data was read. NULL is every user's at once: a backup.
    user_id UUID,

    -- What was read: 'snapshot', 'billing', 'sync', 'backup'. Text rather
    -- than an enum type, because a scope added later is a row to write, not
    -- a migration.
    scope   TEXT NOT NULL,

    -- Where the read was asked from — the address bucket the app logs use,
    -- or the machine a backup ran on.
    client  TEXT NOT NULL
);

CREATE INDEX audit_log_user ON audit_log (user_id, at);

-- Owned by a role nothing logs in as, so the grant below is what the
-- connecting role has — an owner cannot be kept out of its own table.
-- Cluster-wide, so under `sqlx::test` two databases migrating at once both
-- reach this line; the losing one finds the role already there.
DO $$
BEGIN
    CREATE ROLE audit_owner NOLOGIN;
EXCEPTION
    WHEN duplicate_object OR unique_violation THEN NULL;
END $$;

ALTER TABLE audit_log OWNER TO audit_owner;

REVOKE ALL ON audit_log FROM PUBLIC;

DO $$
BEGIN
    EXECUTE format('GRANT SELECT, INSERT ON audit_log TO %I', current_user);
END $$;

-- The grant does not bind a superuser, and the container's init role is one.
-- The trigger is what stands in the way there: rewriting history takes
-- disabling it first, which is a deliberate act rather than a slip.
CREATE FUNCTION audit_log_append_only() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'audit_log is append-only';
END $$;

CREATE TRIGGER audit_log_append_only
    BEFORE UPDATE OR DELETE OR TRUNCATE ON audit_log
    FOR EACH STATEMENT EXECUTE FUNCTION audit_log_append_only();
