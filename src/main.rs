use leptos::prelude::*;

mod util;
use util::chart::LineChart;

#[component]
fn App() -> impl IntoView {
    // Fake price data - (time, price)
    let price_data = vec![
        (0.0, 100.0),
        (1.0, 105.2),
        (2.0, 103.8),
        (3.0, 108.1),
        (4.0, 112.5),
        (5.0, 109.3),
        (6.0, 115.7),
        (7.0, 118.2),
        (8.0, 114.6),
        (9.0, 121.4),
        (10.0, 125.8),
        (11.0, 123.1),
        (12.0, 128.5),
        (13.0, 132.0),
        (14.0, 129.7),
        (15.0, 135.2),
    ];

    view! {
        <div style="width: 100%; height: 400px;">
            <LineChart data=price_data />
        </div>
    }
}

fn main() {
    mount_to_body(|| view! { <App /> })
}
