use api_ledger::Position;
use api_ledger::Tx;
use leptos::prelude::*;
use leptos_meta::Stylesheet;

use web_components::Chart;
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

    let chart_data = vec![
        (0.0, 45.2),
        (1.0, 47.8),
        (2.0, 44.1),
        (3.0, 48.9),
        (4.0, 52.3),
        (5.0, 49.7),
        (6.0, 53.8),
        (7.0, 51.2),
        (8.0, 55.6),
        (9.0, 58.1),
        (10.0, 54.9),
        (11.0, 59.3),
        (12.0, 61.7),
        (13.0, 58.4),
        (14.0, 62.8),
        (15.0, 65.2),
        (16.0, 63.7),
        (17.0, 67.1),
        (18.0, 64.3),
        (19.0, 68.9),
        (20.0, 71.5),
        (21.0, 69.8),
        (22.0, 73.2),
        (23.0, 70.6),
        (24.0, 74.9),
        (25.0, 77.3),
        (26.0, 75.1),
        (27.0, 78.8),
        (28.0, 81.2),
        (29.0, 79.4),
    ];

    view! {
        <Stylesheet id="leptos" href="/style/output.css"/>
        <div class="flex flex-row h-screen bg-gray-950">
            <div class="flex flex-1 p-4">
                <Chart data=chart_data />
            </div>
            <div class="flex flex-col w-[400px] border-l border-gray-900">
                <For
                    each=move || transactions.clone()
                    key=|tx| format!("{:?}", tx)
                    children=|tx| view! { <Transaction tx=tx /> }
                />
            </div>
        </div>
    }
}
