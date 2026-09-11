use client_state::ViewBalances;
use leptos::prelude::*;

#[component]
pub fn BalancesPage(view: Signal<ViewBalances>) -> impl IntoView {
    let _ = view;

    view! { <div class="page">"Balances"</div> }
}
