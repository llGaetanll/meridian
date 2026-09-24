//! The fetch log and the coverage page's reads, against a real Postgres.

use api_common::Bar;
use api_common::Timestamp;
use sqlx::PgPool;
use src_postgres::BarInterval;
use src_postgres::BarSource;
use src_postgres::FetchOutcome;

/// A moment to hang the rows off.
fn at(secs: u64) -> Timestamp {
    Timestamp::from_secs(secs)
}

/// A bar with a close and nothing else.
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

/// Asks land in hour buckets, faults are counted apart from asks, and the
/// totals agree with the buckets.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn asks_are_counted_by_the_hour(db: PgPool) {
    const HOUR: u64 = 3_600;

    for (at_secs, outcome) in [
        (10, FetchOutcome::Ok),
        (20, FetchOutcome::Ok),
        (30, FetchOutcome::RateLimited),
        (HOUR + 10, FetchOutcome::Ok),
    ] {
        src_postgres::record_fetch(
            &db,
            at(at_secs),
            BarSource::Yahoo,
            "SPY",
            BarInterval::Day1,
            at(0)..at(100),
            outcome,
        )
        .await
        .expect("the ask is written");
    }

    let hours = src_postgres::load_fetch_hours(&db, at(0))
        .await
        .expect("the hours read back");

    assert_eq!(hours.len(), 2);
    assert_eq!(
        (hours[0].hour, hours[0].asked, hours[0].faulted),
        (at(0), 3, 1)
    );
    assert_eq!(
        (hours[1].hour, hours[1].asked, hours[1].faulted),
        (at(HOUR), 1, 0)
    );

    let totals = src_postgres::load_fetch_totals(&db, at(0))
        .await
        .expect("the totals read back");

    assert_eq!((totals.asked, totals.ok), (4, 3));
}

/// Only the asks that ended badly come back as faults, in time order, and
/// `since` cuts the older ones off.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn faults_come_back_alone_and_in_order(db: PgPool) {
    for (at_secs, outcome) in [
        (10, FetchOutcome::Refused),
        (20, FetchOutcome::Ok),
        (30, FetchOutcome::Failed),
    ] {
        src_postgres::record_fetch(
            &db,
            at(at_secs),
            BarSource::Yahoo,
            "TSLA",
            BarInterval::Min5,
            at(0)..at(100),
            outcome,
        )
        .await
        .expect("the ask is written");
    }

    let faults = src_postgres::load_fetch_faults(&db, at(0))
        .await
        .expect("the faults read back");

    assert_eq!(faults.len(), 2);
    assert_eq!(faults[0].outcome, FetchOutcome::Refused);
    assert_eq!(faults[1].outcome, FetchOutcome::Failed);

    let later = src_postgres::load_fetch_faults(&db, at(20))
        .await
        .expect("the cut read works");

    assert_eq!(later.len(), 1);
    assert_eq!(later[0].outcome, FetchOutcome::Failed);
}

/// The newest asks come back newest first, and the limit is a limit.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn recent_asks_come_back_newest_first(db: PgPool) {
    for at_secs in [10, 30, 20] {
        src_postgres::record_fetch(
            &db,
            at(at_secs),
            BarSource::Yahoo,
            "SPY",
            BarInterval::Day1,
            at(0)..at(100),
            FetchOutcome::Ok,
        )
        .await
        .expect("the ask is written");
    }

    let recent = src_postgres::load_recent_fetches(&db, 2)
        .await
        .expect("the asks read back");

    assert_eq!(recent.len(), 2);
    assert_eq!((recent[0].at, recent[1].at), (at(30), at(20)));
    assert_eq!(recent[0].span, at(0)..at(100));
}

/// The page's whole-table reads see every series at an interval, and only
/// that interval.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn page_reads_see_every_series_at_one_interval(db: PgPool) {
    for (symbol, close) in [("SPY", 1.0), ("TSLA", 2.0)] {
        src_postgres::save_bars(
            &db,
            BarSource::Yahoo,
            symbol,
            BarInterval::Day1,
            &[bar(0, close), bar(86_400, close)],
            at(0)..at(172_800),
            None,
        )
        .await
        .expect("the bars are written");
    }

    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Week1,
        &[bar(0, 9.0)],
        at(0)..at(604_800),
        None,
    )
    .await
    .expect("the other interval is written");

    let spans = src_postgres::load_coverage_spans(&db, BarSource::Yahoo, BarInterval::Day1)
        .await
        .expect("the spans read back");

    assert_eq!(spans.len(), 2);
    assert_eq!(
        (spans[0].symbol.as_str(), spans[0].span.clone()),
        ("SPY", at(0)..at(172_800))
    );
    assert_eq!(spans[1].symbol.as_str(), "TSLA");

    let stats = src_postgres::load_series_stats(&db, BarSource::Yahoo, BarInterval::Day1, at(0))
        .await
        .expect("the stats read back");

    assert_eq!(stats.len(), 2);
    assert_eq!((stats[0].symbol.as_str(), stats[0].bars), ("SPY", 2));
    assert_eq!((stats[0].first, stats[0].last), (at(0), at(86_400)));
}

/// Thinning keeps the first bar, keeps time order, and never thins below
/// what the target allows.
#[sqlx::test(migrator = "src_postgres::MIGRATOR")]
async fn a_thinned_curve_keeps_its_first_point_and_its_order(db: PgPool) {
    let bars: Vec<Bar> = (0..100).map(|i| bar(i * 60, i as f64)).collect();

    src_postgres::save_bars(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min1,
        &bars,
        at(0)..at(6_000),
        None,
    )
    .await
    .expect("the bars are written");

    let curve = src_postgres::load_closes_thinned(
        &db,
        BarSource::Yahoo,
        "SPY",
        BarInterval::Min1,
        at(0),
        10,
    )
    .await
    .expect("the curve reads back");

    assert_eq!(curve[0].ts, at(0));
    assert!(
        curve.len() >= 10 && curve.len() <= 20,
        "kept {}",
        curve.len()
    );
    assert!(curve.windows(2).all(|pair| pair[0].ts < pair[1].ts));
}
