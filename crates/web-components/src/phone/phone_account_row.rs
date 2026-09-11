use client_state::ViewAccountRow;
use leptos::prelude::*;

/// One account: its name over the sentence the machine assembled about it,
/// and what it comes to.
///
/// Owed is not red here: the section heading above already says these are
/// balances owed, and the minus sign says it again.
#[component]
pub fn PhoneAccountRow(#[prop(into)] row: Signal<ViewAccountRow>) -> impl IntoView {
    let utilisation = move || {
        row.with(|row| row.utilisation.clone())
            .map(|utilisation| view! { <div class="sub">{utilisation}</div> })
    };

    view! {
        <div class="acct-row">
            <div class="t">
                <div class="nm">{move || row.with(|row| row.name.clone())}</div>
                <div class="sub">{move || row.with(|row| row.subtitle.clone())}</div>
                {utilisation}
            </div>
            <span class="amt money">{move || row.with(|row| row.amount.clone())}</span>
        </div>
    }
}
