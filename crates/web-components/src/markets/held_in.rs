use api_common::Asset;
use api_common::format::fmt_money;
use api_common::format::fmt_pct;
use api_common::format::fmt_qty;
use leptos::prelude::*;
use web_ui::AllocBar;
use web_ui::RailSection;

use super::PortfolioHolding;

/// How one asset is spread across the user's portfolios.
#[component]
pub fn HeldIn(#[prop(into)] asset: Asset, holdings: Vec<PortfolioHolding>) -> impl IntoView {
    let rows = holdings
        .into_iter()
        .map(|holding| {
            let asset = asset.clone();
            let share = fmt_pct(holding.share_of_portfolio);
            // The share is always positive here, so drop the leading sign.
            let share = share.trim_start_matches('+').to_string();

            view! {
                <div class="pf-row">
                    <div class="pf-line">
                        <span class="pf-name">{holding.portfolio}</span>
                        <span class="pf-val num">{fmt_money(holding.value)}</span>
                    </div>
                    <AllocBar share=holding.share_of_portfolio />
                    <div class="pf-line pf-sub">
                        <span class="num">{format!("{} {asset}", fmt_qty(holding.qty))}</span>
                        <span class="num">{format!("{share} of portfolio")}</span>
                    </div>
                </div>
            }
        })
        .collect_view();

    view! { <RailSection title="Held in" sub="share of each portfolio">{rows}</RailSection> }
}
