use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;
use web_ui::Glyph;

use super::Runs;
use super::Tally;
use super::read;
use super::read::part;

/// The end of the flow: counts rather than a tick and a slogan, and no
/// balance, which is covered until somebody asks to see it.
#[component]
pub(super) fn Ready(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let title = part(view, |view| read::title(view).to_string());
    let another = part(view, |view| {
        read::aside(view).unwrap_or_default().to_string()
    });
    let open = part(view, |view| {
        read::action(view).unwrap_or_default().to_string()
    });

    view! {
        <div class="say-outcome">
            <Glyph name="tick-circle" class="say-icon done" />
            <div class="say-title">{move || title.get()}</div>
        </div>
        <Runs view=view />
        <Tally view=view />

        // The quieter way out above, the button the screen is about below.
        <div class="acts">
            <button
                type="button"
                class="btn lg block"
                on:click=move |_| ui.run(EventUi::BankConnectRequested)
            >
                {move || another.get()}
            </button>
            <button
                type="button"
                class="btn primary lg block"
                on:click=move |_| ui.run(EventUi::OnboardingFinished)
            >
                {move || open.get()}
            </button>
        </div>
    }
}
