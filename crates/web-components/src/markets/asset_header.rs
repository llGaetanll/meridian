use api_common::Price;
use api_common::format::fmt_money;
use leptos::prelude::*;
use web_ui::Chip;
use web_ui::Seg;

/// Ticker and source on the left, live price and the range picker on the right.
///
/// Everything it shows is a signal, so the header stays put while a new price
/// is on its way. `None` for the price reads as "not in yet".
#[component]
pub fn AssetHeader(
    #[prop(into)] symbol: Signal<String>,
    #[prop(into)] price: Signal<Option<Price>>,
    /// Where the price comes from, e.g. "Yahoo Finance".
    #[prop(into)]
    source: Signal<String>,
    /// Classification line, e.g. "ETF · USD · NYSE Arca".
    #[prop(into)]
    details: Signal<String>,
    #[prop(default = vec!["1D", "1W", "1M", "1Y", "5Y", "All"])] ranges: Vec<&'static str>,
    #[prop(optional)] selected_range: Option<RwSignal<usize>>,
) -> impl IntoView {
    let selected_range = selected_range.unwrap_or_else(|| RwSignal::new(0));

    view! {
        <div class="asset-head">
            <div>
                <div class="asset-sym">{move || symbol.get()}</div>
                <div class="asset-tags">
                    <Chip swatch=true>{move || source.get()}</Chip>
                    <Chip>{move || details.get()}</Chip>
                </div>
            </div>
            <div class="asset-price">
                <div class="p num">
                    {move || price.get().map(fmt_money).unwrap_or_else(|| "—".to_string())}
                </div>
                <div class="asset-range">
                    <Seg options=ranges selected=selected_range />
                </div>
            </div>
        </div>
    }
}
