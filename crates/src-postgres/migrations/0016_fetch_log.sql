-- Every ask a runtime makes of a market source, written as it is made.
--
-- `bar_coverage` holds what a fetch settled; this holds that the fetch
-- happened at all, and how it went. The state machine never reads it — it is
-- what the coverage page draws its uptime bar and request counts from, and
-- without it a quiet hour and an hour of refusals look the same.

CREATE TYPE fetch_outcome AS ENUM ('ok', 'rate_limited', 'refused', 'failed');

CREATE TABLE fetch_log (
    at TIMESTAMPTZ NOT NULL,
    source bar_source NOT NULL,
    symbol TEXT NOT NULL,
    interval bar_interval NOT NULL,

    -- The window asked about, half-open like coverage.
    span_start TIMESTAMPTZ NOT NULL,
    span_end TIMESTAMPTZ NOT NULL,

    outcome fetch_outcome NOT NULL
);

-- The page reads by time, never by symbol.
CREATE INDEX fetch_log_at ON fetch_log (at);
