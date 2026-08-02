mod main_chart;
pub mod math;
mod price;
mod range;
mod volume;

use leptos::prelude::*;

pub use main_chart::MainChart;
pub use price::PriceChart;
pub use range::RangeSelector;
pub use volume::VolumeChart;

/// The row of time labels under a chart. Kept separate from the charts
/// themselves so several stacked charts can share one axis.
#[component]
pub fn ChartFoot(labels: Vec<String>) -> impl IntoView {
    view! {
        <div class="chart-foot">
            {labels.into_iter().map(|label| view! { <span>{label}</span> }).collect_view()}
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
