use client_state::ViewOnboarding;
use leptos::prelude::*;
use web_ui::Glyph;

use super::Runs;
use super::read;
use super::read::part;

/// The flow waiting on something out of its hands — the snapshot that decides
/// which screen follows the code, or Plaid's window while it is up. Nothing to
/// press: what is waited on arriving is what moves the flow on, so the one
/// button is busy the whole time and only says what the wait is for.
#[component]
pub(super) fn Waiting(view: Signal<ViewOnboarding>) -> impl IntoView {
    let step = part(view, |view| read::step(view).to_string());
    let title = part(view, |view| read::title(view).to_string());
    let action = part(view, |view| {
        read::action(view).unwrap_or_default().to_string()
    });

    view! {
        <div class="say-step">{move || step.get()}</div>
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />

        <div class="acts">
            <button type="button" class="btn primary lg block busy" disabled>
                <Glyph name="spin" />
                {move || action.get()}
            </button>
        </div>
    }
}
