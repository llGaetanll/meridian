use api_common::ChartData;
use api_ledger::Position;
use api_ledger::Tx;
use leptos::prelude::*;
use leptos_meta::Stylesheet;

use web_components::MainChart;
use web_components::Transaction;

#[component]
pub fn App() -> impl IntoView {
    let transactions = vec![
        Tx::Trade {
            ts: 1735689600, // 2025-01-01 00:00:00 UTC
            buy: Position {
                asset: "BTC".to_string(),
                qty: 0.5,
            },
            sell: Position {
                asset: "USD".to_string(),
                qty: 45000.0,
            },
        },
        Tx::Transfer {
            ts: 1735776000, // 2025-01-02 00:00:00 UTC
            pos: Position {
                asset: "ETH".to_string(),
                qty: 2.3,
            },
        },
        Tx::Trade {
            ts: 1735862400, // 2025-01-03 00:00:00 UTC
            buy: Position {
                asset: "USD".to_string(),
                qty: 7500.0,
            },
            sell: Position {
                asset: "ETH".to_string(),
                qty: 2.0,
            },
        },
        Tx::Trade {
            ts: 1735948800, // 2025-01-04 00:00:00 UTC
            buy: Position {
                asset: "SOL".to_string(),
                qty: 50.0,
            },
            sell: Position {
                asset: "USD".to_string(),
                qty: 11000.0,
            },
        },
        Tx::Transfer {
            ts: 1736035200, // 2025-01-05 00:00:00 UTC
            pos: Position {
                asset: "USDC".to_string(),
                qty: -1000.0,
            },
        },
        Tx::Trade {
            ts: 1736121600, // 2025-01-06 00:00:00 UTC
            buy: Position {
                asset: "MATIC".to_string(),
                qty: 1500.0,
            },
            sell: Position {
                asset: "USD".to_string(),
                qty: 975.0,
            },
        },
        Tx::Transfer {
            ts: 1736208000, // 2025-01-07 00:00:00 UTC
            pos: Position {
                asset: "BTC".to_string(),
                qty: 0.1,
            },
        },
        Tx::Trade {
            ts: 1736294400, // 2025-01-08 00:00:00 UTC
            buy: Position {
                asset: "USD".to_string(),
                qty: 850.0,
            },
            sell: Position {
                asset: "ADA".to_string(),
                qty: 1000.0,
            },
        },
        Tx::Trade {
            ts: 1736380800, // 2025-01-09 00:00:00 UTC
            buy: Position {
                asset: "DOT".to_string(),
                qty: 120.0,
            },
            sell: Position {
                asset: "USD".to_string(),
                qty: 960.0,
            },
        },
        Tx::Transfer {
            ts: 1736467200, // 2025-01-10 00:00:00 UTC
            pos: Position {
                asset: "LINK".to_string(),
                qty: -75.0,
            },
        },
    ];

    let chart_data = ChartData::default();

    view! {
        <Stylesheet id="leptos" href="/style/output.css" />
        <div class="flex flex-row h-screen bg-base-950">
            <div class="flex flex-1 p-4">
                <MainChart data=chart_data.clone() historical_data=chart_data />
            </div>
            <div class="flex flex-col w-[350px] border-l border-base-900 overflow-y-auto">
                <For
                    each=move || transactions.clone()
                    key=|tx| format!("{:?}", tx)
                    children=|tx| view! { <Transaction tx=tx /> }
                />
            </div>
        </div>
    }
}
