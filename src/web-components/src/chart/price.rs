use api_common::ChartData;
use api_common::Price;
use api_common::format::fmt_money_short;
use leptos::prelude::*;
use web_ui::Tone;

use super::ClosedBands;
use super::Layout;
use super::math;

/// The main price curve: gridlines, a dotted area fill clipped to the curve,
/// the line itself, the stretches of closed market, and a right hand price
/// axis.
///
/// `layout` places the points along the x axis; pass the same one to whatever
/// is stacked under this chart so the two line up.
///
/// `id` only has to be unique among the charts mounted at the same time — it
/// names the clip path the fill is cut with.
#[component]
pub fn PriceChart(
    data: ChartData,
    layout: Layout,
    #[prop(into, default = String::from("price"))] id: String,
    #[prop(optional)] tone: Tone,
    /// Formats the price axis labels.
    #[prop(optional)]
    y_fmt: Option<Box<dyn Fn(Price) -> String>>,
) -> impl IntoView {
    let y_fmt = y_fmt.unwrap_or_else(|| Box::new(fmt_money_short));

    let px_rng = math::padded_price_range(&data, 0.08);
    let ticks = math::nice_ticks(&px_rng);

    let lines = math::session_lines(&data, &layout, &px_rng, math::VIEW_W, math::VIEW_H);
    let clips = math::session_clips(&data, &layout, &px_rng);

    let clip_id = format!("chart-clip-{id}");
    let color = tone.css_var();

    // Gridlines stop at the edge of each session rather than running on
    // through the hatching.
    let grid = ticks
        .iter()
        .flat_map(|&tick| {
            let y = math::px_to_y(tick, &px_rng, math::VIEW_H);

            layout
                .sessions()
                .iter()
                .map(|session| {
                    let x1 = layout.x(session.start, math::VIEW_W);
                    let x2 = layout.x(session.end.saturating_sub(1), math::VIEW_W);

                    view! {
                        <line
                            x1=format!("{x1:.1}")
                            y1=y
                            x2=format!("{x2:.1}")
                            y2=y
                            vector-effect="non-scaling-stroke"
                        />
                    }
                })
                .collect::<Vec<_>>()
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
                        {clips
                            .into_iter()
                            .map(|points| view! { <polygon points=points /> })
                            .collect_view()}
                    </clipPath>
                </defs>
                {lines
                    .into_iter()
                    .map(|points| {
                        view! {
                            <polyline
                                fill="none"
                                stroke=color
                                stroke-width="1.6"
                                stroke-linejoin="round"
                                stroke-linecap="round"
                                vector-effect="non-scaling-stroke"
                                points=points
                            />
                        }
                    })
                    .collect_view()}
            </svg>

            <ClosedBands layout=layout />

            <div class="y-axis">{axis}</div>
        </div>
    }
}
