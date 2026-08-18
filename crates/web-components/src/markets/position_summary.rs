use api_common::format::fmt_money;
use api_common::format::fmt_money_signed;
use api_common::format::fmt_qty;
use leptos::prelude::*;
use web_ui::RailSection;
use web_ui::StatRow;
use web_ui::Stats;
use web_ui::Tone;

use super::AssetPosition;

/// The user's total holding of one asset, with its P/L breakdown.
#[component]
pub fn PositionSummary(position: AssetPosition, portfolio_count: usize) -> impl IntoView {
    let AssetPosition {
        asset,
        qty,
        value,
        avg_cost,
        unrealized,
        realized,
        net_invested,
        today,
    } = position;

    view! {
        <RailSection title="Total position" sub=format!("{portfolio_count} portfolios")>
            <div class="position-value num">{fmt_money(value)}</div>
            <div class="position-sub muted">
                {format!("{} {asset} · avg cost ", fmt_qty(qty))}
                <span class="num">{fmt_money(avg_cost)}</span>
            </div>
            <Stats>
                <StatRow
                    label="Unrealized P/L"
                    value=fmt_money_signed(unrealized)
                    tone=Tone::of(unrealized)
                />
                <StatRow label="Realized P/L" value=fmt_money_signed(realized) />
                <StatRow label="Net invested" value=fmt_money(net_invested) />
                <StatRow label="Today" value=fmt_money_signed(today) tone=Tone::of(today) />
            </Stats>
        </RailSection>
    }
}
