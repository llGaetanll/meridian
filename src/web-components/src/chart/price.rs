use api_common::ChartData;
use api_common::Price;
use api_common::format::fmt_money_short;
use leptos::prelude::*;
use web_ui::Tone;

use super::math;

/// The main price curve: gridlines, a dotted area fill clipped to the curve,
/// the line itself, and a right hand price axis.
///
/// `id` only has to be unique among the charts mounted at the same time — it
/// names the clip path the fill is cut with.
#[component]
pub fn PriceChart(
    data: ChartData,
    #[prop(into, default = String::from("price"))] id: String,
    #[prop(optional)] tone: Tone,
    /// Formats the price axis labels.
    #[prop(optional)]
    y_fmt: Option<Box<dyn Fn(Price) -> String>>,
) -> impl IntoView {
    let y_fmt = y_fmt.unwrap_or_else(|| Box::new(fmt_money_short));

    let px_rng = math::padded_price_range(&data, 0.08);
    let ticks = math::nice_ticks(&px_rng);

    let line = math::polyline_points(&data, &px_rng, math::VIEW_W, math::VIEW_H);
    let clip = math::clip_polygon_points(&data, &px_rng);

    let clip_id = format!("chart-clip-{id}");
    let color = tone.css_var();

    let grid = ticks
        .iter()
        .map(|&tick| {
            let y = math::px_to_y(tick, &px_rng, math::VIEW_H);
            view! {
                <line x1="0" y1=y x2=math::VIEW_W y2=y vector-effect="non-scaling-stroke" />
            }
        })
        .collect_view();

    let axis = ticks
        .iter()
        .map(|&tick| {
            let top = math::px_to_y(tick, &px_rng, 100.0);
            view! { <span style=format!("top:{top:.2}%")>{y_fmt(tick)}</span> }
        })
        .collect_view();

    view! {
        <div class="chart-wrap">
            <svg
                class="chart-grid"
                viewBox=format!("0 0 {} {}", math::VIEW_W, math::VIEW_H)
                preserveAspectRatio="none"
                aria-hidden="true"
            >
                <g stroke="var(--border)">{grid}</g>
            </svg>

            <div
                class="chart-fill"
                style=format!("clip-path:url(#{clip_id});--chart-tone:{color}")
            />

            <svg
                class="chart"
                viewBox=format!("0 0 {} {}", math::VIEW_W, math::VIEW_H)
                preserveAspectRatio="none"
            >
                <defs>
                    <clipPath id=clip_id clipPathUnits="objectBoundingBox">
                        <polygon points=clip />
                    </clipPath>
                </defs>
                <polyline
                    fill="none"
                    stroke=color
                    stroke-width="1.6"
                    stroke-linejoin="round"
                    stroke-linecap="round"
                    vector-effect="non-scaling-stroke"
                    points=line
                />
            </svg>

            <div class="y-axis">{axis}</div>
        </div>
    }
}
