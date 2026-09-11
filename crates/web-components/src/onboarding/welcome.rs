use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;
use web_ui::Glyph;

use super::Fault;
use super::Runs;
use super::read;
use super::read::part;

/// Who you are, and one door to it: an address that exists signs in and one
/// that does not signs up.
#[component]
pub(super) fn Welcome(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let title = part(view, |view| read::title(view).to_string());
    let fault = part(view, |view| read::fault(view).map(str::to_string));
    let action = part(view, |view| {
        read::action(view).unwrap_or_default().to_string()
    });
    let legal = part(view, |view| read::legal(view).to_string());

    view! {
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />
        <Fault text=fault />

        <div class="acts">
            <button
                type="button"
                class="btn primary lg block"
                on:click=move |_| ui.run(EventUi::EmailChosen)
            >
                <Glyph name="mail" />
                {move || action.get()}
            </button>
        </div>

        <div class="say-legal">{move || legal.get()}</div>
    }
}
