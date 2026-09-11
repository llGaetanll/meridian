use client_state::ViewAccountBank;
use leptos::prelude::*;

/// One bank: its mark, its name, and whether it is still working.
///
/// A bank that has stopped syncing is the one thing in the app that stops the
/// numbers being true, so its second line says so in amber.
#[component]
pub fn BankRow(#[prop(into)] bank: Signal<ViewAccountBank>) -> impl IntoView {
    view! {
        <div class="row" class:needs=move || bank.with(|b| b.needs_reconnect)>
            <span class="mark">{move || bank.with(|b| b.initials.clone())}</span>
            <div class="rt">
                {move || bank.with(|b| b.name.clone())}
                <div class="rs">{move || bank.with(|b| b.detail.clone())}</div>
            </div>
        </div>
    }
}
