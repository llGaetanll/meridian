use leptos::prelude::*;
use web_components::Transaction;
use web_ui::Button;
use web_ui::ButtonSize;
use web_ui::ButtonVariant;

use crate::shell::PhoneShell;
use crate::state::TxEvent;
use crate::state::use_transactions;

/// The list of everything that has happened, newest first.
#[component]
pub fn TransactionsPage() -> impl IntoView {
    let transactions = use_transactions();
    let txs = transactions.txs();

    let footer = move || {
        view! {
            <Button
                variant=ButtonVariant::Primary
                size=ButtonSize::Lg
                class="w-full m-0 tap".to_string()
                on_click=Box::new(move || transactions.dispatch(TxEvent::AddRequested))
            >
                "Add transaction"
            </Button>
        }
    };

    view! {
        <PhoneShell title="Transactions" footer=footer>
            <For each=move || txs.get().into_iter().enumerate() key=|(i, _)| *i let:entry>
                <div class="tx-row">
                    <Transaction tx=entry.1 />
                </div>
            </For>
        </PhoneShell>
    }
}
