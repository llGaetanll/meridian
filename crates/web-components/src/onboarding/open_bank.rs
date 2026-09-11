use client_state::EventUi;
use leptos::prelude::*;
use web_ui::Glyph;

/// The button that opens Plaid, on both screens that have one.
///
/// A moment passes between the click and Plaid's page: the server has to open
/// a Link session before the page has anywhere to go. So while the machine
/// says it is `opening`, the button wears those words instead of its label and
/// takes no clicks.
#[component]
pub(super) fn OpenBank(
    /// The glyph in front of the label while nothing is happening.
    icon: &'static str,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] opening: Signal<Option<String>>,
    ui: Callback<EventUi>,
) -> impl IntoView {
    let busy = move || opening.with(Option::is_some);

    view! {
        <button
            type="button"
            class=move || {
                if busy() { "btn primary lg block busy" } else { "btn primary lg block" }
            }
            disabled=busy
            on:click=move |_| ui.run(EventUi::BankConnectRequested)
        >
            <Glyph name=Signal::derive(move || {
                if busy() { "spin" } else { icon }.to_string()
            }) />
            {move || opening.get().unwrap_or_else(|| label.get())}
        </button>
    }
}
