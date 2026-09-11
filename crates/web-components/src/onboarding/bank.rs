use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::IconRows;
use super::OpenBank;
use super::Runs;
use super::read;
use super::read::part;

/// What connecting a bank means, and the button that does it. No skip: every
/// screen in the app is a view of transactions.
#[component]
pub(super) fn Bank(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let step = part(view, |view| read::step(view).to_string());
    let title = part(view, |view| read::title(view).to_string());
    let action = part(view, |view| {
        read::action(view).unwrap_or_default().to_string()
    });
    let opening = part(view, read::opening);
    let legal = part(view, |view| read::legal(view).to_string());

    view! {
        <div class="say-step">{move || step.get()}</div>
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />
        <IconRows view=view />

        <div class="acts">
            <OpenBank icon="bank" label=action opening=opening ui=ui />
        </div>

        <div class="say-legal">{move || legal.get()}</div>
    }
}
