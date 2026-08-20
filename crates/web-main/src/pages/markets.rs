use std::ops::Range;

use api_common::ChartData;
use api_common::Timestamp;
use api_common::format::fmt_date;
use api_common::format::fmt_month_day;
use api_common::format::fmt_month_year;
use api_common::format::fmt_time_of_day;
use api_common::format::fmt_year;
use leptos::prelude::*;
use web_components::AssetHeader;
use web_components::AssetTradesTable;
use web_components::CHART_RESOLUTION;
use web_components::ChartFoot;
use web_components::HeldIn;
use web_components::KeyStats;
use web_components::Layout;
use web_components::PositionSummary;
use web_components::PriceChart;
use web_components::QuoteList;
use web_components::RangeFoot;
use web_components::RangeSelector;
use web_components::TradeComposer;
use web_components::VolumeChart;
use web_components::math;
use web_ui::SearchBar;
use web_ui::Tone;

use crate::fake;
use crate::io::AssetQuote;
use crate::io::ChartRange;
use crate::state::MarketsState;

/// 2026-08-02 — "today" for the parts still on stand-in data.
const TODAY: Timestamp = Timestamp::from_secs(1_785_628_800);

const DAY: u64 = 86_400;
const YEAR: u64 = 365 * DAY;

#[component]
pub fn MarketsPage() -> impl IntoView {
    let state = MarketsState::new(fake::SYMBOL);

    let quote = state.quote;
    let history = state.history;

    let position = fake::position();
    let holdings = fake::holdings();
    let portfolio_count = holdings.len();

    // The stretch of time the main chart is showing, for the range selector.
    let shown = Signal::derive(move || {
        quote
            .get()
            .and_then(|q| q.ok())
            .map(|q| q.prices.range_time())
    });

    let price = Signal::derive(move || quote.get().and_then(|q| q.ok()).map(|q| q.price));
    let details = Signal::derive(move || {
        quote
            .get()
            .and_then(|q| q.ok())
            .map(|q| q.details)
            .unwrap_or_default()
    });

    view! {
        <div class="asset-layout">
            <div class="col">
                <SearchBar />

                <div>
                    <AssetHeader
                        symbol=state.symbol()
                        price=price
                        source="Yahoo Finance".to_string()
                        details=details
                        ranges=ChartRange::ALL.map(|r| r.label()).to_vec()
                        selected_range=state.selected_range
                    />

                    {move || match quote.get() {
                        None => view! { <ChartPlaceholder message="Loading…" /> }.into_any(),
                        Some(Err(error)) => view! { <ChartPlaceholder message=error /> }.into_any(),
                        Some(Ok(quote)) => {
                            view! { <SessionCharts quote=quote /> }.into_any()
                        }
                    }}

                    {move || match history.get() {
                        Some(Ok(history)) => {
                            view! { <HistoryStrip history=history shown=shown.get() /> }.into_any()
                        }
                        _ => ().into_any(),
                    }}
                </div>

                <AssetTradesTable asset=fake::SYMBOL trades=fake::trades() />
            </div>

            <aside class="rail">
                <TradeComposer
                    pay_asset="USD"
                    pay_amount=6_497.40
                    receive_asset=fake::SYMBOL
                    receive_amount=10.0
                    rate=price
                    portfolio="Core"
                    date_label=fmt_date(TODAY)
                />

                <PositionSummary position=position portfolio_count=portfolio_count />

                <HeldIn asset=fake::SYMBOL holdings=holdings />

                {move || {
                    let stats = quote
                        .get()
                        .and_then(|q| q.ok())
                        .map(|q| q.key_stats)
                        .unwrap_or_default();

                    view! { <KeyStats source="Yahoo Finance" stats=stats /> }
                }}

                <QuoteList title="Recently seen" sub="Clear" quotes=fake::recently_seen() />
            </aside>
        </div>
    }
}

/// The price chart, its volume bars, and the shared time axis.
///
/// One layout for all three: it is what places the points along the x axis,
/// and passing the same one down is what keeps a bar under its point and the
/// closed hours in the same place on both charts.
#[component]
fn SessionCharts(quote: AssetQuote) -> impl IntoView {
    let tone = match quote.open.or(quote.previous_close) {
        Some(open) => Tone::of(quote.price - open),
        None => Tone::Neutral,
    };

    // Thinned here rather than on the way in: the series is cached whole, and
    // a chart only has so many pixels. Prices keep each bucket's close and
    // volume adds its bucket up, which lands them on the same buckets and so
    // keeps a bar under its point.
    let prices = quote.prices.resample_last(CHART_RESOLUTION);
    let volume = quote.volume.resample_sum(CHART_RESOLUTION);

    let layout = Layout::new(&prices, quote.utc_offset);
    let labels = axis_labels(&prices, &layout, 6, quote.utc_offset);

    // The point the crosshair is on. The price chart sets it; the time axis
    // reads it, so that the two say the same thing.
    let hover = RwSignal::new(None);
    let cursor = cursor_labels(&prices, &layout, hover, quote.utc_offset);

    view! {
        <PriceChart data=prices layout=layout.clone() tone=tone hover=hover />
        <VolumeChart data=volume layout=layout hover=hover />
        <ChartFoot labels=labels cursor=cursor />
    }
}

/// The full history strip, with the stretch on show highlighted.
#[component]
fn HistoryStrip(history: AssetQuote, shown: Option<Range<Timestamp>>) -> impl IntoView {
    let prices = history.prices.resample_last(CHART_RESOLUTION);

    let labels = range_labels(&prices, 7, history.utc_offset);
    let window = window_of(&prices, shown);

    view! {
        <RangeSelector data=prices window=window />
        <RangeFoot labels=labels />
    }
}

#[component]
fn ChartPlaceholder(#[prop(into)] message: String) -> impl IntoView {
    view! { <div class="chart-placeholder">{message}</div> }
}

/// Evenly spaced time labels, each with where along the width it belongs.
/// Placed by the layout rather than spread out on their own, so a label still
/// sits under its point once the closed hours have taken their share.
fn axis_labels(
    data: &ChartData,
    layout: &Layout,
    count: usize,
    utc_offset: i64,
) -> Vec<(f64, String)> {
    let format = label_format(data);

    math::axis_stops_at(data.len(), count)
        .into_iter()
        .map(|at| {
            let ts = data.timestamps()[at];
            (layout.fraction(at), format(local(ts, utc_offset)))
        })
        .collect()
}

/// The label for every point, ready for whichever one the crosshair lands on.
/// Written out up front: there are only as many as the chart has points, and
/// the pointer asks for them far more often than they change.
fn cursor_labels(
    data: &ChartData,
    layout: &Layout,
    hover: RwSignal<Option<usize>>,
    utc_offset: i64,
) -> Signal<Option<(f64, String)>> {
    let format = cursor_format(data);

    let labels = StoredValue::new(
        data.timestamps()
            .iter()
            .enumerate()
            .map(|(i, &ts)| (layout.fraction(i), format(local(ts, utc_offset))))
            .collect::<Vec<_>>(),
    );

    Signal::derive(move || {
        hover
            .get()
            .and_then(|i| labels.with_value(|l| l.get(i).cloned()))
    })
}

/// The same labels for the history strip, which spreads them itself.
fn range_labels(data: &ChartData, count: usize, utc_offset: i64) -> Vec<String> {
    let format = label_format(data);

    math::axis_stops(data, count)
        .into_iter()
        .map(|ts| format(local(ts, utc_offset)))
        .collect()
}

/// Read a timestamp where the asset trades, so days turn over at the
/// exchange's midnight rather than UTC's.
fn local(ts: Timestamp, utc_offset: i64) -> Timestamp {
    ts.saturating_add_signed(utc_offset)
}

/// How precisely to say when, at whatever the span calls for. The span is what
/// decides, not the interval: a month of half-hourly points still wants dates
/// on the axis, not clock times.
fn label_format(data: &ChartData) -> fn(Timestamp) -> String {
    let rng = data.range_time();
    let span = rng.end.since(rng.start).as_secs();

    if span <= 2 * DAY {
        fmt_time_of_day
    } else if span <= 90 * DAY {
        fmt_month_day
    } else if span <= 3 * YEAR {
        fmt_month_year
    } else {
        fmt_year
    }
}

/// The same, for the one point under the crosshair, which can afford to be
/// more precise than the axis: an intraday point is named by day and time
/// even when the axis only has room for the day.
fn cursor_format(data: &ChartData) -> fn(Timestamp) -> String {
    let rng = data.range_time();
    let span = rng.end.since(rng.start).as_secs();

    if span <= 2 * DAY {
        fmt_time_of_day
    } else if data.len() > 1 && span / data.len() as u64 <= DAY {
        fmt_day_time
    } else {
        fmt_date
    }
}

/// `"Aug 05 · 10:30"`
fn fmt_day_time(ts: Timestamp) -> String {
    format!("{} · {}", fmt_month_day(ts), fmt_time_of_day(ts))
}

/// Where `shown` falls inside the whole history, as fractions of its width.
/// Measured in the strip's own terms — slots, not clock time. A single session
/// is a hairline at this scale, so the window never gets thinner than
/// something the eye can catch.
fn window_of(history: &ChartData, shown: Option<Range<Timestamp>>) -> (f64, f64) {
    const MIN_WIDTH: f64 = 0.012;

    let Some(shown) = shown else {
        return (0.0, 1.0);
    };

    let start = math::slot_fraction(history, shown.start);
    let end = math::slot_fraction(history, shown.end);
    let width = (end - start).clamp(MIN_WIDTH, 1.0);

    ((end - width).max(0.0), end)
}
