use api_common::format::fmt_money;
use api_common::format::fmt_pct;
use leptos::prelude::*;
use web_ui::RailSection;
use web_ui::Tone;

use super::Quote;

/// A compact list of assets with their last price and move, e.g. "Recently seen".
#[component]
pub fn QuoteList(
    #[prop(into)] title: String,
    #[prop(optional, into)] sub: String,
    quotes: Vec<Quote>,
    #[prop(optional)] on_select: Option<Callback<String>>,
) -> impl IntoView {
    let rows = quotes
        .into_iter()
        .map(|quote| {
            let symbol = quote.symbol.clone();
            let tone = Tone::of(quote.change);

            view! {
                <div
                    class="mini-row"
                    on:click=move |_| {
                        if let Some(cb) = on_select {
                            cb.run(symbol.clone());
                        }
                    }
                >
                    <div>
                        <div class="sym">{quote.symbol}</div>
                        <div class="nm">{quote.name}</div>
                    </div>
                    <div class="mini-quote">
                        <div class="num">{fmt_money(quote.price)}</div>
                        <div class=format!("num mini-change {}", tone.class())>
                            {fmt_pct(quote.change)}
                        </div>
                    </div>
                </div>
            }
        })
        .collect_view();

    view! {
        <RailSection title=title sub=sub>
            <div class="mini-list">{rows}</div>
        </RailSection>
    }
}
