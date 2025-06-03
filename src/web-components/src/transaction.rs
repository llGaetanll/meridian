use api_ledger::Position;
use api_ledger::Timestamp;
use api_ledger::Tx;
use leptos::prelude::*;
use leptos::*;

#[component]
pub fn Transaction(tx: Tx) -> impl IntoView {
    match tx {
        Tx::Trade { ts, buy, sell } => view! {
            <TransactionTrade ts=ts buy=buy sell=sell />
        }
        .into_any(),
        Tx::Transfer { ts, pos } => view! {
            <TransactionTransfer ts=ts pos=pos />
        }
        .into_any(),
    }
}

#[component]
fn TransactionTrade(ts: Timestamp, buy: Position, sell: Position) -> impl IntoView {
    view! {
        <div class="bg-transparent p-4">
            <div class="flex items-center justify-between mb-3">
                <div class="flex items-center gap-2">
                    <div class="w-2 h-2 bg-purple-400 rounded-full"></div>
                    <span class="text-xs font-medium text-purple-300 font-semibold uppercase tracking-wide">Trade</span>
                </div>
                <time class="text-sm text-gray-400">{format_timestamp(ts)}</time>
            </div>

            <div class="grid gap-x-3" style="grid-template-columns: 1fr auto 1fr; grid-template-rows: min-content 1fr;">
                <div class="flex flex-col items-end">
                    <div class="text-xs text-white uppercase font-semibold">{sell.asset.clone()}</div>
                </div>
                <div></div>
                <div class="flex flex-col items-start">
                    <div class="text-xs text-white uppercase font-semibold">{buy.asset.clone()}</div>
                </div>

                <div class="text-4xl text-white font-bold text-right">{format!("{:.2}", sell.qty)}</div>
                <div class="flex items-center justify-center">
                    <svg class="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        <path stroke-linecap="square" stroke-linejoin="miter" stroke-width="2" d="M13 7l5 5m0 0l-5 5m5-5H6"></path>
                    </svg>
                </div>
                <div class="text-4xl text-white font-bold">{format!("{:.2}", buy.qty)}</div>
            </div>
        </div>
    }
}

#[component]
fn TransactionTransfer(ts: Timestamp, pos: Position) -> impl IntoView {
    enum TransferType {
        Deposit,
        Withdrawal,
    }

    let transfer = if pos.qty >= 0.0 {
        TransferType::Deposit
    } else {
        TransferType::Withdrawal
    };

    view! {
        <div class="bg-transparent p-4">
            <div class="flex items-center justify-between mb-3">
                {match transfer {
                    TransferType::Deposit => {
                        view! {
                            <div class="flex items-center gap-2">
                                <div class="w-2 h-2 bg-green-400 rounded-full"></div>
                                <span class="text-xs font-medium text-green-300 font-semibold uppercase tracking-wide">Deposit</span>
                            </div>
                        }.into_any()
                    },
                    TransferType::Withdrawal => {
                        view! {
                            <div class="flex items-center gap-2">
                                <div class="w-2 h-2 bg-red-400 rounded-full"></div>
                                <span class="text-xs font-medium text-red-300 font-semibold uppercase tracking-wide">Withdrawal</span>
                            </div>
                        }.into_any()
                    }
                }}
                <time class="text-sm text-gray-400">{format_timestamp(ts)}</time>
            </div>

            <div class="grid gap-x-3" style="grid-template-columns: 1fr auto 1fr; grid-template-rows: min-content 1fr;">
                <div />
                <div />
                <div class="flex flex-col items-start">
                    <div class="text-xs text-white uppercase font-semibold">{pos.asset.clone()}</div>
                </div>

                <div />
                <div class="flex items-center justify-center">
                    <svg class="w-6 h-6 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                        {match transfer {
                            TransferType::Deposit => {
                                view! {
                                    <path stroke-linecap="square" stroke-linejoin="miter" stroke-width="2" d="M12 6v12m6-6H6"></path>
                                }
                            },
                            TransferType::Withdrawal => {
                                view! {
                                    <path stroke-linecap="square" stroke-linejoin="miter" stroke-width="2" d="M6 12h12"></path>
                                }
                            }
                        }}
                    </svg>
                </div>
                <div class="text-4xl text-white font-bold text-left">{format!("{:.2}", pos.qty.abs())}</div>
            </div>
        </div>
    }
}

fn format_timestamp(ts: Timestamp) -> String {
    let date = web_sys::js_sys::Date::new(&(ts as f64 * 1000.0).into());
    date.to_locale_string("en-US", &web_sys::js_sys::Object::new())
        .as_string()
        .unwrap_or_default()
}
