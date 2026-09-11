use client_state::ViewAccountRow;
use leptos::prelude::*;

/// One account: its name, what kind it is, what else is worth knowing, and
/// what it comes to.
///
/// The phone reads the middle two as one sentence under the name; here they
/// are columns, so two current accounts can be compared without reading
/// either to the end.
#[component]
pub fn BalanceRow(#[prop(into)] row: Signal<ViewAccountRow>) -> impl IntoView {
    let utilisation = move || {
        row.with(|row| row.utilisation.clone())
            .map(|utilisation| view! { <span class="acct-util">{utilisation}</span> })
    };

    view! {
        <div class="acct">
            <span class="acct-name">{move || row.with(|row| row.name.clone())}</span>
            <span class="acct-kind">{move || row.with(|row| row.kind.clone())}</span>
            <span class="acct-detail">
                {move || row.with(|row| row.detail.clone())} {utilisation}
            </span>
            <span class="acct-amt money">{move || row.with(|row| row.amount.clone())}</span>
        </div>
    }
}
