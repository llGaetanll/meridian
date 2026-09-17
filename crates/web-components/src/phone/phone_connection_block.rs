use client_state::EventUi;
use client_state::ViewConnection;
use leptos::prelude::*;
use web_ui::Glyph;

use super::PhoneAccountRow;
use crate::live;

/// One connection as the phone stacks it: the bank, its accounts one under
/// the other, and what they come to under a rule.
#[component]
pub fn PhoneConnectionBlock(
    #[prop(into)] connection: Signal<ViewConnection>,
    ui: Callback<EventUi>,
) -> impl IntoView {
    let accounts = Signal::derive(move || connection.with(|c| c.accounts.clone()));

    // Plaid Link in update mode: the sign-in repairs this bank rather than
    // attaching another. While the machine says a repair is under way the
    // button waits, so a second tap cannot open a second sheet.
    let reconnect = move || {
        connection
            .with(|c| {
                c.needs_reconnect
                    .then(|| (c.reconnecting, c.item_id.clone()))
            })
            .map(|(busy, item_id)| {
                view! {
                    <button
                        class=if busy { "btn plain busy" } else { "btn plain" }
                        disabled=busy
                        on:click=move |_| {
                            ui.run(EventUi::BankReconnectRequested {
                                item_id: item_id.clone(),
                            })
                        }
                    >
                        <Glyph name=if busy { "spin" } else { "ext" } />
                        "Reconnect"
                    </button>
                }
            })
    };

    view! {
        <div class="conn">
            <div class="conn-head">
                <span class="mark sm">{move || connection.with(|c| c.initials.clone())}</span>
                <span class="conn-name">{move || connection.with(|c| c.name.clone())}</span>
                {reconnect}
            </div>

            <For
                each=move || accounts.get()
                key=|row| row.id.clone()
                children=move |first| {
                    view! { <PhoneAccountRow row=live(accounts, first, |row| &row.id) /> }
                }
            />

            <div class="conn-total">
                <span class="k">{move || connection.with(|c| c.total_label.clone())}</span>
                <span class="v money">{move || connection.with(|c| c.total.clone())}</span>
            </div>
        </div>
    }
}
