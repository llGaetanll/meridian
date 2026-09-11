use client_state::ViewLedger;
use leptos::prelude::*;

#[component]
pub fn LedgerPage(view: Signal<ViewLedger>) -> impl IntoView {
    let _ = view;

    view! { <div class="page">"Ledger"</div> }
}
