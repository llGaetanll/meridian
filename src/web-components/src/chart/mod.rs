mod layout;
mod main_chart;
pub mod math;
mod price;
mod range;
mod volume;

use leptos::prelude::*;

pub use layout::Closed;
pub use layout::Closure;
pub use layout::Layout;
pub use main_chart::MainChart;
pub use price::PriceChart;
pub use range::RangeSelector;
pub use volume::VolumeChart;

/// The hours the market was shut, hatched across whatever chart they are laid
/// over. Every chart sharing a [`Layout`] gets the same bands in the same
/// places, which is what makes the stack read as one picture.
#[component]
pub fn ClosedBands(layout: Layout) -> impl IntoView {
    if layout.closures().is_empty() {
        return ().into_any();
    }

    view! {
        <div class="band-layer" aria-hidden="true">
            {layout
                .closures()
                .iter()
                .map(|closure| {
                    let left = closure.start * 100.0;
                    let width = (closure.end - closure.start) * 100.0;

                    view! {
                        <span
                            class="closed-band"
                            data-label=closure.kind.label()
                            style=format!("left:{left:.3}%;width:{width:.3}%")
                        />
                    }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

/// The row of time labels under a chart, each one placed at the point it
/// belongs to. Kept separate from the charts themselves so several stacked
/// charts can share one axis.
#[component]
pub fn ChartFoot(labels: Vec<(f64, String)>) -> impl IntoView {
    view! {
        <div class="chart-foot">
            {labels
                .into_iter()
                .map(|(at, label)| {
                    // The labels at either end would hang off the chart if
                    // they were centred on their point like the rest.
                    let class = match at {
                        at if at <= 0.001 => "at-start",
                        at if at >= 0.999 => "at-end",
                        _ => "",
                    };

                    view! {
                        <span class=class style=format!("left:{:.3}%", at * 100.0)>
                            {label}
                        </span>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// The same row, sized for the range selector below the chart.
#[component]
pub fn RangeFoot(labels: Vec<String>) -> impl IntoView {
    view! {
        <div class="range-foot">
            {labels.into_iter().map(|label| view! { <span>{label}</span> }).collect_view()}
        </div>
    }
}
