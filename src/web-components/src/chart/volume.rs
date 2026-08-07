use api_common::ChartData;
use leptos::prelude::*;

use super::ClosedBands;
use super::Layout;
use super::math;

const VOLUME_H: f64 = 100.0;

/// Traded volume, drawn as bars under the price chart. The series reuses
/// [`ChartData`]: the "price" of each point is that bar's volume.
///
/// `layout` should be the one the price chart above was drawn with, so that
/// each bar sits under its point and the closed hours line up.
#[component]
pub fn VolumeChart(
    data: ChartData,
    layout: Layout,
    #[prop(into, default = String::from("Volume"))] label: String,
) -> impl IntoView {
    let max = data.range_price().end.max(f64::MIN_POSITIVE);
    let width = (layout.slot() * math::VIEW_W * 0.62).max(0.5);

    let bars = data
        .iter()
        .enumerate()
        .map(|(i, (_, volume))| {
            let height = (volume / max).clamp(0.0, 1.0) * VOLUME_H;
            // Centred on the same slot the price chart draws point `i` at.
            let x = layout.x(i, math::VIEW_W) - width / 2.0;

            view! {
                <rect
                    x=format!("{x:.2}")
                    y=format!("{:.1}", VOLUME_H - height)
                    width=format!("{width:.2}")
                    height=format!("{height:.1}")
                />
            }
        })
        .collect_view();

    view! {
        <div class="volume-wrap">
            <svg
                class="volume"
                viewBox=format!("0 0 {} {}", math::VIEW_W, VOLUME_H)
                preserveAspectRatio="none"
            >
                {bars}
            </svg>

            <ClosedBands layout=layout />

            <div class="y-axis">
                <span style="top:8%">{label}</span>
            </div>
        </div>
    }
}
