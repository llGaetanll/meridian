use client_state::EventUi;
use leptos::prelude::*;
use web_ui::Glyph;

/// What the button says while Plaid's page is on its way. The phone prints
/// the same words.
const OPENING: &str = "Opening Plaid…";

/// The button that opens Plaid, on both screens that have one.
///
/// A moment passes between the click and Plaid's page: the server has to open
/// a Link session before the page has anywhere to go. So the button says what
/// it is waiting on and stops taking clicks until the machine says the wait is
/// over.
#[component]
pub(super) fn OpenBank(
    /// The glyph in front of the label while nothing is happening.
    icon: &'static str,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] opening: Signal<bool>,
    ui: Callback<EventUi>,
) -> impl IntoView {
    view! {
        <button
            type="button"
            class=move || {
                if opening.get() { "btn primary lg block busy" } else { "btn primary lg block" }
            }
            disabled=move || opening.get()
            on:click=move |_| ui.run(EventUi::BankConnectRequested)
        >
            <Glyph name=Signal::derive(move || {
                if opening.get() { "spin" } else { icon }.to_string()
            }) />
            {move || if opening.get() { OPENING.to_string() } else { label.get() }}
        </button>
    }
}
