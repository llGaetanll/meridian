use api_common::ChartData;
use leptos::prelude::*;

use super::math;

const VOLUME_H: f64 = 100.0;

/// Traded volume, drawn as bars under the price chart. The series reuses
/// [`ChartData`]: the "price" of each point is that bar's volume.
#[component]
pub fn VolumeChart(
    data: ChartData,
    #[prop(into, default = String::from("Volume"))] label: String,
) -> impl IntoView {
    let max = data.range_price().end.max(f64::MIN_POSITIVE);
    let count = data.len();
    let width = (math::VIEW_W / count.max(1) as f64 * 0.62).max(0.5);

    let bars = data
        .iter()
        .enumerate()
        .map(|(i, (_, volume))| {
            let height = (volume / max).clamp(0.0, 1.0) * VOLUME_H;
            // Centred on the same slot the price chart draws point `i` at.
            let x = math::slot_to_x(i, count, math::VIEW_W) - width / 2.0;

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
            <div class="y-axis">
                <span style="top:8%">{label}</span>
            </div>
        </div>
    }
}
