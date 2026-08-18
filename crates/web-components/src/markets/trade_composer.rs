use api_common::Asset;
use api_common::Price;
use api_common::Quantity;
use api_common::format::fmt_decimal;
use api_common::format::fmt_money;
use leptos::prelude::*;
use web_ui::Btn;
use web_ui::BtnVariant;
use web_ui::Icon;
use web_ui::IconKind;
use web_ui::LinkButton;

/// The "log a trade" box: what goes in, what comes out, and where it lands.
///
/// Editing is not wired up yet — the fields show the proposed trade only.
#[component]
pub fn TradeComposer(
    /// The asset paid with, and how much of it.
    #[prop(into)]
    pay_asset: Asset,
    pay_amount: Quantity,
    /// The asset received, and how much of it.
    #[prop(into)]
    receive_asset: Asset,
    receive_amount: Quantity,
    /// Price of one `receive_asset` in `pay_asset` terms, or `None` until it
    /// is known.
    #[prop(into)]
    rate: Signal<Option<Price>>,
    #[prop(default = 0.0)] fees: Price,
    #[prop(into)] portfolio: String,
    #[prop(into)] date_label: String,
) -> impl IntoView {
    let rate_label = format!("1 {receive_asset} = ");

    view! {
        <div class="trade-card">
            <div class="rail-head trade-head">
                <span class="rail-title">Log a trade</span>
                <LinkButton chevron=true>{date_label}</LinkButton>
            </div>

            <div class="trade">
                <TradeLeg label="In" asset=pay_asset amount=pay_amount />

                <button class="swap" title="Flip direction">
                    <Icon kind=IconKind::Swap />
                </button>

                <TradeLeg label="Out" asset=receive_asset amount=receive_amount />
            </div>

            <div class="trade-rate">
                <span class="faint">
                    {rate_label}
                    <span class="num">
                        {move || rate.get().map(fmt_money).unwrap_or_else(|| "—".to_string())}
                    </span>
                </span>
                <span class="faint">{format!("Fees {}", fmt_money(fees))}</span>
            </div>

            <div class="field">
                <span class="field-label">Portfolio</span>
                <button class="pick-wide">
                    {portfolio}
                    <Icon kind=IconKind::ChevronDown />
                </button>
            </div>

            <Btn variant=BtnVariant::Primary block=true>Log trade</Btn>
        </div>
    }
}

#[component]
fn TradeLeg(
    #[prop(into)] label: String,
    #[prop(into)] asset: Asset,
    amount: Quantity,
) -> impl IntoView {
    let decimals = if amount.abs() >= 100.0 { 2 } else { 3 };

    view! {
        <div class="leg">
            <span class="leg-label">{label}</span>
            <div class="leg-row">
                <button class="asset-pick">
                    {asset}
                    <Icon kind=IconKind::ChevronDown />
                </button>
                <input
                    class="leg-amount num"
                    value=fmt_decimal(amount, decimals)
                    inputmode="decimal"
                />
            </div>
        </div>
    }
}
