use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;
use web_ui::Glyph;

use super::IconRows;
use super::OpenBank;
use super::Runs;
use super::read;
use super::read::part;

/// The bank sign-in closed without a connection — the common ending, not a
/// rare one. The row under it says what is still true, and Open Meridian is
/// there only when a bank already connected.
#[component]
pub(super) fn Failed(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let step = part(view, |view| read::step(view).to_string());
    let title = part(view, |view| read::title(view).to_string());
    let action = part(view, |view| {
        read::action(view).unwrap_or_default().to_string()
    });
    let opening = part(view, |view| read::opening(view).map(str::to_string));
    let open = part(view, |view| read::aside(view).map(str::to_string));

    view! {
        <div class="say-step">{move || step.get()}</div>
        <Glyph name="warn" class="say-icon needs" />
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />
        <IconRows view=view />

        <div class="acts">
            {move || {
                open.get()
                    .map(|open| {
                        view! {
                            <button
                                type="button"
                                class="btn lg block"
                                on:click=move |_| ui.run(EventUi::OnboardingFinished)
                            >
                                {open}
                            </button>
                        }
                    })
            }}

            // Plaid's page is over the web, so trying again leaves the app the
            // way it did the first time, and the glyph says so.
            <OpenBank icon="ext" label=action opening=opening ui=ui />
        </div>
    }
}
