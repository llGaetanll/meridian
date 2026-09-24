use client_state::EventUi;
use client_state::ViewConnection;
use leptos::prelude::*;

use super::AccountRow;
use super::ConnectionHead;
use crate::live;

/// One login, and every account it brings in — held and owed together, which
/// is what the page is a list of.
///
/// No total under the rows: a login that reports a current account and a
/// mortgage has no figure worth adding them into, and the rows are the
/// numbers to read.
#[component]
pub fn ConnectionBlock(
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
                    view! { <AccountRow row=live(accounts, first, |row| &row.id) /> }
                }
            />
        </div>
    }
}
