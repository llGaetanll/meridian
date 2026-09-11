use client_state::ViewConnection;
use leptos::prelude::*;
use web_ui::Glyph;

use super::BalanceRow;
use crate::live;

/// One connection's accounts, on one side of the ledger, and what they come to.
#[component]
pub fn ConnectionBlock(#[prop(into)] connection: Signal<ViewConnection>) -> impl IntoView {
    let accounts = Signal::derive(move || connection.with(|c| c.accounts.clone()));

    // Drawn and not built: clearing a stale sign-in takes Plaid Link in update
    // mode, and the machine has no event for it yet.
    let reconnect = move || {
        connection.with(|c| c.needs_reconnect).then(|| {
            view! {
                <button class="btn plain dead" disabled>
                    <Glyph name="ext" />
                    "Reconnect"
                </button>
            }
        })
    };

    view! {
        <div class="conn">
            <div class="conn-head">
                <span class="mark">{move || connection.with(|c| c.initials.clone())}</span>
                <span class="conn-name">{move || connection.with(|c| c.name.clone())}</span>
                {reconnect}
            </div>

            <For
                each=move || accounts.get()
                key=|row| row.id.clone()
                children=move |first| {
                    view! { <BalanceRow row=live(accounts, first, |row| &row.id) /> }
                }
            />

            <div class="conn-total">
                <span class="k">{move || connection.with(|c| c.total_label.clone())}</span>
                <span class="v money">{move || connection.with(|c| c.total.clone())}</span>
            </div>
        </div>
    }
}
