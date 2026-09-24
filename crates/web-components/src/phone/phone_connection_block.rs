use client_state::EventUi;
use client_state::ViewConnection;
use leptos::prelude::*;

use super::PhoneAccountRow;
use crate::ConnectionHead;
use crate::live;

/// One connection as the phone stacks it: the bank, then its accounts one
/// under the other.
#[component]
pub fn PhoneConnectionBlock(
    #[prop(into)] connection: Signal<ViewConnection>,
    ui: Callback<EventUi>,
) -> impl IntoView {
    let accounts = Signal::derive(move || connection.with(|c| c.accounts.clone()));

    view! {
        <div class="conn">
            <ConnectionHead connection=connection ui=ui />

            <For
                each=move || accounts.get()
                key=|row| row.id.clone()
                children=move |first| {
                    view! { <PhoneAccountRow row=live(accounts, first, |row| &row.id) /> }
                }
            />
        </div>
    }
}
