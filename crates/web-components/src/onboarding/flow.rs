use std::mem::discriminant;

use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::Bank;
use super::Code;
use super::Email;
use super::Failed;
use super::Ready;
use super::Syncing;
use super::Welcome;

/// First run: whichever screen of it the machine is showing.
///
/// Which screen is the machine's to say, so this switches on the arm it was
/// handed and holds no step of its own. A screen is built once when the flow
/// arrives at it; the renders after that — a keystroke, a tick of the
/// countdown, a page of the sync landing — reach only the parts that read
/// what changed, so a field keeps its focus and caret through them.
///
/// Everything a screen does goes out through `ui` as the event it is, and
/// nothing here decides what that event means.
#[component]
pub fn Onboarding(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let screen = Memo::new(move |_| view.with(discriminant));

    move || {
        // Read rather than only tracked: a memo nobody has read has not
        // subscribed to the view, so it would never say the screen changed.
        screen.get();

        view.with_untracked(|shown| match shown {
            ViewOnboarding::Welcome { .. } => view! { <Welcome view=view ui=ui /> }.into_any(),
            ViewOnboarding::Email { .. } => view! { <Email view=view ui=ui /> }.into_any(),
            ViewOnboarding::Code { .. } => view! { <Code view=view ui=ui /> }.into_any(),
            ViewOnboarding::Bank { .. } => view! { <Bank view=view ui=ui /> }.into_any(),
            ViewOnboarding::Sync { .. } => view! { <Syncing view=view /> }.into_any(),
            ViewOnboarding::Ready { .. } => view! { <Ready view=view ui=ui /> }.into_any(),
            ViewOnboarding::Failed { .. } => view! { <Failed view=view ui=ui /> }.into_any(),
        })
    }
}
