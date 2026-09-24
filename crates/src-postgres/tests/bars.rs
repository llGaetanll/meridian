//! The bar queries, against a real Postgres.
//!
//! Most of these are about coverage: the claim that a window already fetched
//! is never fetched again rests on spans merging exactly, so what matters is
//! the arithmetic at the seams — spans that touch, spans that overlap, and
//! spans that only look adjacent.

use api_common::Bar;
use api_common::Timestamp;
use sqlx::PgPool;
use src_postgres::BarInterval;
use src_postgres::BarSource;

/// A moment to hang the spans off.
fn at(secs: u64) -> Timestamp {
    Timestamp::from_secs(secs)
}

/// A bar with a close and nothing else, since most claims here are about
/// where a bar is rather than what it holds.
fn bar(secs: u64, close: f64) -> Bar {
    Bar {
        ts: at(secs),
        open: None,
        high: None,
        low: None,
        close,
        volume: 0,
    }
}

/// Bars written are read back in time order, and the window is half-open: a
/// bar starting exactly at the end belongs to the next window.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn bars_come_back_in_order_over_a_half_open_window(db: PgPool) {
    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[bar(600, 2.0), bar(300, 1.0), bar(900, 3.0)],
        at(300)..at(1200),
    )
    .await
    .expect("the bars are written");

    let held = src_postgres::load_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        at(300)..at(900),
    )
    .await
    .expect("the read works");

    assert_eq!(held, vec![bar(300, 1.0), bar(600, 2.0)]);
}

/// A refetched bar overwrites the one standing, which is what lets an
/// adjusted series land on top of the old one.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_bar_fetched_again_is_one_row_with_the_new_values(db: PgPool) {
    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Day1,
        &[bar(0, 1.0)],
        at(0)..at(86_400),
    )
    .await
    .expect("the first write lands");

    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Day1,
        &[bar(0, 2.0)],
        at(0)..at(86_400),
    )
    .await
    .expect("the second write lands");

    let held = src_postgres::load_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Day1,
        at(0)..at(86_400),
    )
    .await
    .expect("the read works");

    assert_eq!(held, vec![bar(0, 2.0)]);
}

/// The whole point: `t0..t1` then `t1..t2` reads back as one span, so a later
/// ask for `t0..t2` has nothing missing.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn touching_spans_merge_into_one(db: PgPool) {
    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(0)..at(500),
    )
    .await
    .expect("the first span is written");

    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(500)..at(900),
    )
    .await
    .expect("the second span is written");

    let coverage = src_postgres::load_coverage(&db, BarSource::Yahoo, "SPY", BarInterval::Min5)
        .await
        .expect("the read works");

    assert_eq!(coverage, vec![at(0)..at(900)]);
}

/// A span bridging two others swallows all three into one.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_bridging_span_swallows_both_sides(db: PgPool) {
    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(0)..at(300),
    )
    .await
    .expect("the left span is written");

    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(600)..at(900),
    )
    .await
    .expect("the right span is written");

    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(200)..at(700),
    )
    .await
    .expect("the bridge is written");

    let coverage = src_postgres::load_coverage(&db, BarSource::Yahoo, "SPY", BarInterval::Min5)
        .await
        .expect("the read works");

    assert_eq!(coverage, vec![at(0)..at(900)]);
}

/// Spans that do not meet stay apart, in order — the gap between them is
/// exactly what a planner still has to fetch.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn disjoint_spans_stay_apart_and_sorted(db: PgPool) {
    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(600)..at(900),
    )
    .await
    .expect("the later span is written");

    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(0)..at(300),
    )
    .await
    .expect("the earlier span is written");

    let coverage = src_postgres::load_coverage(&db, BarSource::Yahoo, "SPY", BarInterval::Min5)
        .await
        .expect("the read works");

    assert_eq!(coverage, vec![at(0)..at(300), at(600)..at(900)]);
}

/// A series is its source, symbol and interval together: coverage written for
/// one is no claim about the others.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn coverage_does_not_leak_across_series(db: PgPool) {
    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(0)..at(300),
    )
    .await
    .expect("the span is written");

    let other_interval =
        src_postgres::load_coverage(&db, BarSource::Yahoo, "SPY", BarInterval::Day1)
            .await
            .expect("the read works");

    let other_source =
        src_postgres::load_coverage(&db, BarSource::Coingecko, "SPY", BarInterval::Min5)
            .await
            .expect("the read works");

    assert_eq!(other_interval, vec![]);
    assert_eq!(other_source, vec![]);
}

/// An empty span is no news and writes nothing.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn an_empty_span_is_dropped(db: PgPool) {
    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min5,
        &[],
        at(300)..at(300),
    )
    .await
    .expect("the write lands");

    let coverage = src_postgres::load_coverage(&db, BarSource::Yahoo, "SPY", BarInterval::Min5)
        .await
        .expect("the read works");

    assert_eq!(coverage, vec![]);
}
