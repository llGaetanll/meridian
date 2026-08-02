use api_common::Asset;
use api_common::format::fmt_date;
use api_common::format::fmt_decimal;
use api_common::format::fmt_money;
use api_common::format::fmt_money_signed;
use leptos::prelude::*;
use web_ui::Card;
use web_ui::Pill;
use web_ui::SectionHead;
use web_ui::Tone;

use super::AssetTrade;
use super::TradeResult;
use super::TradeSide;

/// Every trade the user made in one asset.
#[component]
pub fn AssetTradesTable(#[prop(into)] asset: Asset, trades: Vec<AssetTrade>) -> impl IntoView {
    let count = trades.len();
    let portfolios = trades
        .iter()
        .map(|trade| trade.portfolio.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();

    let rows = trades
        .into_iter()
        .map(|trade| {
            let tone = match trade.side {
                TradeSide::Buy => Tone::Up,
                TradeSide::Sell => Tone::Down,
            };

            let result = match trade.result {
                TradeResult::Unrealized(pl) => view! {
                    <td class=format!("r num {}", Tone::of(pl).class())>{fmt_money_signed(pl)}</td>
                }
                .into_any(),
                TradeResult::Realized(pl) => view! {
                    <td class="r num faint">{format!("realized {}", fmt_money_signed(pl))}</td>
                }
                .into_any(),
            };

            view! {
                <tr>
                    <td class="num">{fmt_date(trade.ts)}</td>
                    <td>
                        <Pill tone=tone>{trade.side.label()}</Pill>
                    </td>
                    <td class="muted">{trade.portfolio}</td>
                    <td class="r num">{fmt_decimal(trade.qty, 3)}</td>
                    <td class="r num">{fmt_money(trade.price)}</td>
                    <td class="r num">{fmt_money(trade.value)}</td>
                    {result}
                </tr>
            }
        })
        .collect_view();

    view! {
        <div class="table-block">
            <SectionHead
                title=format!("Trades involving {asset}")
                sub=format!("{count} trades · {portfolios} portfolios")
            />
            <Card>
                <table>
                    <thead>
                        <tr>
                            <th>Date</th>
                            <th>Direction</th>
                            <th>Portfolio</th>
                            <th class="r">Quantity</th>
                            <th class="r">Price</th>
                            <th class="r">Value</th>
                            <th class="r">Unrealized</th>
                        </tr>
                    </thead>
                    <tbody>{rows}</tbody>
                </table>
            </Card>
        </div>
    }
}
