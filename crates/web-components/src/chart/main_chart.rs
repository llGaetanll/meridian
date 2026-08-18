use std::ops::Range;

use api_common::ChartData;
use api_common::Price;
use leptos::prelude::*;

use super::math;

/// A price series plotted over a flatter reference series. Superseded by
/// [`super::PriceChart`] for the markets page, kept for the plain two-series case.
#[component]
pub fn MainChart(data: ChartData, historical_data: ChartData) -> impl IntoView {
    let px_rng = data.range_price();
    let px_ticks = math::nice_ticks(&px_rng);

    view! {
        <div
            class="grid gap-3 flex-1"
            style="grid-template-columns: 1fr 50px; grid-template-rows: 1fr 50px;"
        >
            <svg class="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
                <ChartTicks px_ticks=px_ticks.clone() px_rng=px_rng.clone() />
                <ChartPath
                    id=String::from("main")
                    px_rng=px_rng.clone()
                    data=data
                    color=String::from("var(--chart-accent)")
                />
            </svg>

            <div class="relative w-full h-full">
                <ChartTexts y_ticks=px_ticks px_rng=px_rng />
            </div>

            <svg class="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
                <ChartPath
                    id=String::from("history")
                    px_rng=historical_data.range_price()
                    data=historical_data
                    color=String::from("var(--chart-base)")
                />
            </svg>

            <span />
        </div>
    }
}

#[component]
fn ChartPath(id: String, data: ChartData, px_rng: Range<Price>, color: String) -> impl IntoView {
    let line = math::polyline_points(&data, &px_rng, 100.0, 100.0);
    let area = math::area_points(&data, &px_rng, 100.0, 100.0);

    let gradient_id = format!("chart-gradient-{id}");

    view! {
        <defs>
            <linearGradient id=gradient_id.clone() x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" style=format!("stop-color:{color};stop-opacity:0.3") />
                <stop offset="100%" style="stop-color:transparent;stop-opacity:0" />
            </linearGradient>
        </defs>

        <polygon points=area fill=format!("url(#{gradient_id})") stroke="none" />

        <polyline
            points=line
            fill="none"
            stroke=color
            stroke-width="2.0"
            stroke-linecap="round"
            stroke-linejoin="round"
            vector-effect="non-scaling-stroke"
        />
    }
}

#[component]
fn ChartTicks(px_ticks: Vec<Price>, px_rng: Range<Price>) -> impl IntoView {
    px_ticks
        .iter()
        .map(|&tick| {
            let y = math::px_to_y(tick, &px_rng, 100.0);
            view! {
                <line
                    x1="0"
                    y1=y
                    x2="100"
                    y2=y
                    stroke="var(--chart-base)"
                    stroke-width="0.8"
                    vector-effect="non-scaling-stroke"
                />
            }
        })
        .collect_view()
}

#[component]
fn ChartTexts(y_ticks: Vec<Price>, px_rng: Range<Price>) -> impl IntoView {
    y_ticks
        .iter()
        .map(|&tick| {
            let top = math::px_to_y(tick, &px_rng, 100.0);
            view! {
                <label
                    class="absolute -translate-y-1/2"
                    style=format!("top: {top}%; color: var(--chart-base)")
                >
                    {tick}
                </label>
            }
        })
        .collect_view()
}
