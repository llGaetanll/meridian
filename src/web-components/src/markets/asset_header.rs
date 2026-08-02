use api_common::Price;
use api_common::format::fmt_money;
use leptos::prelude::*;
use web_ui::Chip;
use web_ui::Seg;

/// Ticker and source on the left, live price and the range picker on the right.
#[component]
pub fn AssetHeader(
    #[prop(into)] symbol: String,
    price: Price,
    /// Where the price comes from, e.g. "Yahoo Finance".
    #[prop(into)]
    source: String,
    /// Classification line, e.g. "ETF · USD · NYSE Arca".
    #[prop(into)]
    details: String,
    #[prop(default = vec!["1D", "1W", "1M", "1Y", "5Y", "All"])] ranges: Vec<&'static str>,
    #[prop(optional)] selected_range: Option<RwSignal<usize>>,
) -> impl IntoView {
    let selected_range = selected_range.unwrap_or_else(|| RwSignal::new(0));

    view! {
        <div class="asset-head">
            <div>
                <div class="asset-sym">{symbol}</div>
                <div class="asset-tags">
                    <Chip swatch=true>{source}</Chip>
                    <Chip>{details}</Chip>
                </div>
            </div>
            <div class="asset-price">
                <div class="p num">{fmt_money(price)}</div>
                <div class="asset-range">
                    <Seg options=ranges selected=selected_range />
                </div>
            </div>
        </div>
    }
}
