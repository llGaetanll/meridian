-- Market data, fetched once and held for everyone.
--
-- Two tables that only mean anything together. `bars` is the points a source
-- served; `bar_coverage` is the stretches of time that have been asked about.
-- They are not the same thing and cannot be inferred from one another: a
-- market is shut more often than it is open, so a window with no points in it
-- is just as likely to be fully known as never fetched. Coverage is what
-- stops the same empty weekend being fetched again every time it is drawn.

-- Where a series came from. Symbols are the source's own namespace — 'AAPL'
-- is Yahoo's word and 'bitcoin' is CoinGecko's — so a series is keyed by its
-- source and two sources never mix.
CREATE TYPE bar_source AS ENUM ('yahoo', 'coingecko');

-- How much time one bar stands for. Only fixed-length intervals: a month is
-- not a number of seconds, so bars of that size cannot be laid on a grid or
-- folded into one another.
--
-- The interval stored is the one the source actually served, which is not
-- always the one asked for — CoinGecko picks its own from the window's width,
-- and Yahoo refuses intraday bars past a certain age.
CREATE TYPE bar_interval AS ENUM ('1m', '2m', '5m', '15m', '30m', '1h', '1d', '1wk');

CREATE TABLE bars (
    source bar_source NOT NULL,
    symbol TEXT NOT NULL,
    interval bar_interval NOT NULL,

    -- The start of the interval the bar stands for.
    ts TIMESTAMPTZ NOT NULL,

    -- The close is the value every chart reads, so a point without one is not
    -- worth a row. The rest of the candle is optional because sources leave
    -- holes in each field independently.
    open DOUBLE PRECISION,
    high DOUBLE PRECISION,
    low DOUBLE PRECISION,
    close DOUBLE PRECISION NOT NULL,

    -- Units traded over the interval. A source that reports none says zero.
    volume BIGINT NOT NULL DEFAULT 0,

    -- One bar per point on the grid. A refetch overwrites rather than
    -- duplicating, which is also what lets an adjusted series land on top of
    -- the old one.
    PRIMARY KEY (source, symbol, interval, ts)
);

-- The stretches of time a series is known over. Spans are half-open
-- `[span_start, span_end)`, and the writer merges any that meet or overlap,
-- so what this table holds for one series is always sorted and disjoint.
CREATE TABLE bar_coverage (
    source bar_source NOT NULL,
    symbol TEXT NOT NULL,
    interval bar_interval NOT NULL,

    span_start TIMESTAMPTZ NOT NULL,
    span_end TIMESTAMPTZ NOT NULL,

    PRIMARY KEY (source, symbol, interval, span_start)
);
