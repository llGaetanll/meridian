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
const TODAY: Timestamp = 1_785_628_800;

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

    let layout = Layout::new(&quote.prices, quote.utc_offset);
    let labels = axis_labels(&quote.prices, &layout, 6, quote.utc_offset);

    view! {
        <PriceChart data=quote.prices layout=layout.clone() tone=tone />
        <VolumeChart data=quote.volume layout=layout />
        <ChartFoot labels=labels />
    }
}

/// The full history strip, with the stretch on show highlighted.
#[component]
fn HistoryStrip(history: AssetQuote, shown: Option<Range<Timestamp>>) -> impl IntoView {
    let labels = range_labels(&history.prices, 7, history.utc_offset);
    let window = window_of(&history.prices, shown);

    view! {
        <RangeSelector data=history.prices window=window />
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
    let span = rng.end.saturating_sub(rng.start);

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
    let width = (end - start).max(MIN_WIDTH).min(1.0);

    ((end - width).max(0.0), end)
}
