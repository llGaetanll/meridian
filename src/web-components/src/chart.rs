use std::ops::Range;

use api_common::ChartData;
use api_common::Price;
use api_common::Timestamp;
use leptos::prelude::*;

#[component]
pub fn MainChart(data: ChartData, historical_data: ChartData) -> impl IntoView {
    let px_rng = data.range_price();
    let px_ticks = compute_nice_ticks(&px_rng);

    view! {
        <div class="grid gap-3 flex-1" style="grid-template-columns: 1fr 50px; grid-template-rows: 1fr 50px;">
            <svg
                class="w-full h-full"
                viewBox="0 0 100 100"
                preserveAspectRatio="none"
            >
                <ChartTicks px_ticks=px_ticks.clone() px_rng=px_rng.clone() />

                <ChartPath id=String::from("main") data=data color=String::from("var(--chart-accent)") />
            </svg>

            <div class="relative w-full h-full" >
                <ChartTexts y_ticks=px_ticks px_rng=px_rng />
            </div>

            <svg
                class="w-full h-full"
                viewBox="0 0 100 100"
                preserveAspectRatio="none"
            >
                <ChartPath id=String::from("history") data=historical_data color=String::from("var(--chart-base)") />
            </svg>

            <span />
        </div>
    }
}

#[component]
fn ChartPath(id: String, data: ChartData, color: String) -> impl IntoView {
    let path_data = compute_data_path(&data);
    let fill_path_data = compute_data_fill_path(&data);

    let gradient_id = format!("chart-gradient-{id}");

    view! {
        // Define the gradient
        <defs>
            <linearGradient id=gradient_id.clone() x1="0%" y1="0%" x2="0%" y2="100%">
                <stop offset="0%" style=format!("stop-color:{color};stop-opacity:0.3") />
                <stop offset="100%" style="stop-color:transparent;stop-opacity:0" />
            </linearGradient>
        </defs>

        // Fill area under the curve
        <path
            d=fill_path_data
            fill=format!("url(#{gradient_id})")
            stroke="none"
        />

        // Chart line
        <path
            d=path_data
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
        .map(|&y| {
            let y = px_to_svg_y(y, &px_rng);
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
        .map(|&y| {
            let y_percent = px_to_svg_y(y, &px_rng);
            view! {
                <label
                    class="absolute -translate-y-1/2"
                    style=format!("top: {}%; color: var(--chart-base)", y_percent)
                >
                    {y}
                </label>
            }
        })
        .collect_view()
}

fn compute_data_path(data: &ChartData) -> String {
    let ts_rng = data.range_time();
    let px_rng = data.range_price();

    data.into_iter()
        .enumerate()
        .map(|(i, (&ts, &px))| {
            let svg_x = ts_to_svg_x(ts, &ts_rng);
            let svg_y = px_to_svg_y(px, &px_rng);

            if i == 0 {
                format!("M {:.2} {:.2}", svg_x, svg_y)
            } else {
                format!("L {:.2} {:.2}", svg_x, svg_y)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn compute_data_fill_path(data: &ChartData) -> String {
    let ts_rng = data.range_time();
    let px_rng = data.range_price();

    let mut path_parts = Vec::new();

    // Start at bottom-left
    if let Some((first_ts, _)) = data.first() {
        let svg_x = ts_to_svg_x(*first_ts, &ts_rng);
        path_parts.push(format!("M {:.2} 100", svg_x));
    }

    // Draw up to first point and then along the line
    for (&ts, &px) in data {
        let svg_x = ts_to_svg_x(ts, &ts_rng);
        let svg_y = px_to_svg_y(px, &px_rng);

        path_parts.push(format!("L {:.2} {:.2}", svg_x, svg_y));
    }

    // Close the path by going down to bottom-right and back to start
    if let Some((last_ts, _)) = data.last() {
        let svg_x = ts_to_svg_x(*last_ts, &ts_rng);
        path_parts.push(format!("L {:.2} 100", svg_x));
        path_parts.push("Z".to_string());
    }

    path_parts.join(" ")
}

/// A float used in svg drawing space
type FSvg = f64;

// We add padding because, depending on the thickness of the line, it can appear to clip
// outside of the SVG.
const CHART_PAD: f64 = 0.; // TODO

#[inline]
fn px_to_svg_y(px: Price, px_rng: &Range<Price>) -> FSvg {
    let Range {
        start: min_y,
        end: max_y,
    } = px_rng;

    let (min_y, max_y) = (min_y - CHART_PAD, max_y + CHART_PAD);

    // `1. -` because higher y means higher on the graph, but the svg origin is always top left
    (1. - (px - min_y) / (max_y - min_y)) * 100.0
}

#[inline]
fn ts_to_svg_x(ts: Timestamp, x_rng: &Range<Timestamp>) -> FSvg {
    let Range {
        start: min_x,
        end: max_x,
    } = x_rng;

    let ts = ts as Price;
    let (min_x, max_x) = (*min_x as Price, *max_x as Price);

    let (min_x, max_x) = (min_x - CHART_PAD, max_x + CHART_PAD);

    ((ts - min_x) / (max_x - min_x)) * 100.0
}

const TARGET_TICKS: usize = 7;

/// Returns a list of "nice" tick mark positions for the given data range
fn compute_nice_ticks(rng: &Range<Price>) -> Vec<Price> {
    let Range { start, end } = rng;

    let range = end - start;
    let raw_step = range / (TARGET_TICKS - 1) as f64;

    // Find the magnitude (power of 10)
    let magnitude = 10_f64.powf(raw_step.log10().floor());

    // Normalize the step to be between 1 and 10
    let normalized_step = raw_step / magnitude;

    // Round to nearest "nice" number: 1, 2, 5, or 10
    let nice_step = if normalized_step <= 1.0 {
        1.0
    } else if normalized_step <= 2.0 {
        2.0
    } else if normalized_step <= 5.0 {
        5.0
    } else {
        10.0
    };

    let step = nice_step * magnitude;

    // Find nice start and end points
    let nice_start = (start / step).ceil() * step;
    let nice_end = (end / step).floor() * step;

    // Generate ticks
    let mut ticks = Vec::new();
    let mut current = nice_start;

    while current <= nice_end + f64::EPSILON {
        ticks.push(current);
        current += step;
    }

    ticks
}
