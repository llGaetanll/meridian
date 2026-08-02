use api_common::ChartData;
use leptos::prelude::*;

use super::math;

const RANGE_H: f64 = 100.0;

/// The full history strip below the chart, with the visible window highlighted.
///
/// `window` is the visible slice as fractions of the whole history,
/// `(start, end)` in `0.0..=1.0`. Dragging it is not wired up yet.
#[component]
pub fn RangeSelector(
    data: ChartData,
    #[prop(default = (0.0, 1.0))] window: (f64, f64),
) -> impl IntoView {
    let px_rng = math::padded_price_range(&data, 0.02);
    let line = math::polyline_points(&data, &px_rng, math::VIEW_W, RANGE_H);
    let area = math::area_points(&data, &px_rng, math::VIEW_W, RANGE_H);

    let start = window.0.clamp(0.0, 1.0) * 100.0;
    let end = window.1.clamp(0.0, 1.0).max(window.0) * 100.0;

    view! {
        <div class="range-wrap">
            <div class="range-track">
                <svg
                    class="range"
                    viewBox=format!("0 0 {} {}", math::VIEW_W, RANGE_H)
                    preserveAspectRatio="none"
                >
                    <polygon fill="var(--bg-panel)" points=area />
                    <polyline
                        fill="none"
                        stroke="var(--bars)"
                        stroke-width="1"
                        vector-effect="non-scaling-stroke"
                        points=line
                    />
                </svg>

                <div class="range-mask" style=format!("left:0;width:{start:.2}%") />
                <div class="range-mask" style=format!("left:{end:.2}%;right:0") />
                <div
                    class="range-window"
                    style=format!("left:{start:.2}%;width:{:.2}%", end - start)
                />
            </div>
        </div>
    }
}
