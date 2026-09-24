use client_state::EventUi;
use client_state::ViewConnection;
use leptos::prelude::*;
use web_ui::Glyph;

use super::RemoveAsk;

/// The bank's own line over its accounts: its mark, its name, when it last
/// synced — or, on a login that has expired, the button that fixes it — and
/// the way to remove it.
///
/// The mark is the bank glyph rather than the bank's initials: two letters in
/// a square read as a monogram nobody chose, and four of them down a page read
/// as four. A logo goes here once something asks Plaid for one.
///
/// The sign-in and the sync time take the same place on the line because they
/// answer the same question. The trash comes after both, and is a grey glyph
/// rather than a rose word: what it does cannot be undone, but four rose trash
/// cans down a list of banks would make the loudest thing on the page the one
/// thing nobody came to do — and the card it opens asks again in rose anyway.
#[component]
pub fn ConnectionHead(
    #[prop(into)] connection: Signal<ViewConnection>,
    ui: Callback<EventUi>,
) -> impl IntoView {
    // Plaid Link in update mode: the sign-in repairs this bank rather than
    // attaching another. While the machine says a repair is under way the
    // button waits, so a second tap cannot open a second sheet.
    let expired = move || {
        connection.with(|c| {
            c.needs_reconnect
                .then(|| (c.reconnecting, c.item_id.clone(), c.reconnect_label.clone()))
        })
    };

    let state = move || match expired() {
        Some((busy, item_id, label)) => view! {
            <button
                class=if busy { "btn plain warn busy" } else { "btn plain warn" }
                disabled=busy
                on:click=move |_| {
                    ui.run(EventUi::BankReconnectRequested { item_id: item_id.clone() })
                }
            >
                <Glyph name=if busy { "spin" } else { "ext" } />
                {label}
            </button>
        }
        .into_any(),

        None => view! {
            <span class="conn-when">{move || connection.with(|c| c.synced.clone())}</span>
        }
        .into_any(),
    };

    let remove = move || {
        let (item_id, label) = connection.with(|c| (c.item_id.clone(), c.remove_label.clone()));

        view! {
            <button
                class="btn plain mute"
                title=label.clone()
                aria-label=label
                // The press that opens the card must not reach the window, or
                // the listener the card puts there would shut it in the same
                // breath.
                on:click=move |event| {
                    event.stop_propagation();

                    ui.run(EventUi::BankRemoveRequested { item_id: item_id.clone() });
                }
            >
                <Glyph name="trash" />
            </button>
        }
    };

    // The question, on the block it is about, for as long as the machine says
    // this is the bank being removed.
    let ask = move || {
        connection
            .with(|c| c.remove.clone())
            .map(|remove| view! { <RemoveAsk remove=remove ui=ui /> })
    };

    view! {
        <div class="conn-head">
            <span class="mark bare">
                <Glyph name="bank" />
            </span>
            <span class="conn-name">{move || connection.with(|c| c.name.clone())}</span>

            <div class="conn-acts">{state} {remove} {ask}</div>
        </div>
    }
}
