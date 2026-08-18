use leptos::prelude::*;

use crate::pages::TransactionsPage;
use crate::state::provide_transactions;

#[component]
pub fn App() -> impl IntoView {
    provide_transactions();

    view! { <TransactionsPage /> }
}
