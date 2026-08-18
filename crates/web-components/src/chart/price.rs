use api_common::ChartData;
use api_common::Price;
use api_common::format::fmt_money_short;
use leptos::html;
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
///
/// `hover` is the point the pointer is over, which this chart writes as the
/// pointer moves. Pass the same signal to anything else that has to follow the
/// crosshair — the time axis under the chart, say.
#[component]
pub fn PriceChart(
    data: ChartData,
    layout: Layout,
    #[prop(into, default = String::from("price"))] id: String,
    #[prop(optional)] tone: Tone,
    #[prop(optional)] hover: Option<RwSignal<Option<usize>>>,
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

    let hover = hover.unwrap_or_else(|| RwSignal::new(None));

    // Where the crosshair goes for every point, worked out once: the pointer
    // moves far more often than the data changes.
    let cursors = StoredValue::new(
        data.iter()
            .enumerate()
            .map(|(i, (_, px))| Cursor {
                x: layout.fraction(i) * 100.0,
                y: math::px_to_y(px, &px_rng, 100.0),
                label: y_fmt(px),
            })
            .collect::<Vec<_>>(),
    );

    // The point under the pointer, or the last one there. Holding on to it is
    // what lets the axis value fade out where it stood instead of vanishing.
    let last = Memo::new(move |prev: Option<&Option<Cursor>>| {
        hover
            .get()
            .and_then(|i| cursors.with_value(|cs| cs.get(i).cloned()))
            .or_else(|| prev.cloned().flatten())
    });

    let plot = NodeRef::<html::Div>::new();
    let axis_of = StoredValue::new(layout.clone());

    let point_at = move |ev: &leptos::ev::MouseEvent| {
        let rect = plot.get_untracked()?.get_bounding_client_rect();

        if rect.width() <= 0.0 {
            return None;
        }

        let at = (ev.client_x() as f64 - rect.left()) / rect.width();
        axis_of.with_value(|layout| layout.nearest(at.clamp(0.0, 1.0)))
    };

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
        <div
            class="chart-wrap"
            on:mousemove=move |ev| hover.set(point_at(&ev))
            on:mouseleave=move |_| hover.set(None)
        >
            <svg
                class="chart-grid"
                class:cursor=move || hover.get().is_some()
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

            <div
                class="cursor-layer"
                node_ref=plot
                style=format!("--chart-tone:{color}")
                aria-hidden="true"
            >
                {move || {
                    let cursor = hover.get().and_then(|i| cursors.with_value(|cs| cs.get(i).cloned()))?;

                    Some(
                        view! {
                            <span class="cursor-line" style=format!("left:{:.3}%", cursor.x) />
                            <span
                                class="cursor-dot"
                                style=format!("left:{:.3}%;top:{:.3}%", cursor.x, cursor.y)
                            />
                        },
                    )
                }}
            </div>

            <div class="y-axis" class:cursor=move || hover.get().is_some()>
                {axis}
                <span
                    class="cursor-val"
                    style=move || last.get().map(|c| format!("top:{:.2}%", c.y))
                >
                    {move || last.get().map(|c| c.label)}
                </span>
            </div>
        </div>
    }
}

/// Where the crosshair sits for one point, as percentages of the plot, with
/// the price already written out for the axis.
#[derive(Clone, PartialEq)]
struct Cursor {
    x: f64,
    y: f64,
    label: String,
}
