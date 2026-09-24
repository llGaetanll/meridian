-- What a source says about an asset itself, as opposed to its price history.
--
-- One row per series' subject, replaced whole on every fetch: these facts
-- arrive attached to every chart response and age in seconds, where a closed
-- bar never ages at all. That is why they are not columns on `bars` — a
-- bar's row is written once and settled, and this row is the freshest word.
--
-- Everything is nullable but the offset: a source states what it states, and
-- a fact it left out is left out here rather than guessed at.
CREATE TABLE assets (
    source bar_source NOT NULL,
    symbol TEXT NOT NULL,

    name TEXT,

    -- Classification line for a header, e.g. 'ETF · USD · NYSEArca'.
    details TEXT,

    currency TEXT,

    -- Seconds to add to a UTC timestamp to read it in exchange local time.
    utc_offset BIGINT NOT NULL DEFAULT 0,

    -- The live price when the market was open at the last fetch, and the
    -- session figures beside it. All of these go stale the moment they are
    -- written; a chart's own number comes from the bars, and these are the
    -- header dressing around it.
    price DOUBLE PRECISION,
    previous_close DOUBLE PRECISION,
    day_low DOUBLE PRECISION,
    day_high DOUBLE PRECISION,
    year_low DOUBLE PRECISION,
    year_high DOUBLE PRECISION,
    day_volume BIGINT,

    -- The regular session in progress at the last fetch, or the last one to
    -- have run — what lets a day chart show the session rather than the
    -- clock's last 24 hours.
    session_start TIMESTAMPTZ,
    session_end TIMESTAMPTZ,

    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),

    PRIMARY KEY (source, symbol)
);
