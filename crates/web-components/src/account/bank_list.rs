use client_state::ViewAccountBank;
use leptos::prelude::*;

use super::BankRow;
use crate::live;

/// Every attached bank, one row each.
#[component]
pub fn BankList(#[prop(into)] banks: Signal<Vec<ViewAccountBank>>) -> impl IntoView {
    view! {
        <div class="banks">
            <For
                each=move || banks.get()
                key=|bank| bank.id.clone()
                children=move |first| {
                    view! { <BankRow bank=live(banks, first, |b| &b.id) /> }
                }
            />
        </div>
    }
}
