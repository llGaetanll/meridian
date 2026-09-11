use client_state::EventUi;
use leptos::prelude::*;
use web_ui::Glyph;

/// The chevron, on the screens that have somewhere to go back to. Where it
/// goes is the machine's call, so it is an event rather than a history pop.
#[component]
pub(super) fn Back(ui: Callback<EventUi>) -> impl IntoView {
    view! {
        <button
            type="button"
            class="btn plain back"
            aria-label="Back"
            on:click=move |_| ui.run(EventUi::BackPressed)
        >
            <Glyph name="back" />
        </button>
    }
}
