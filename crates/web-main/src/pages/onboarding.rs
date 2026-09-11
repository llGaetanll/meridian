//! First run: what somebody sees before anything else, ending with a bank
//! connected.
//!
//! Drawn without the shell, as the paywall is: there is no app behind a
//! sidebar yet, so a nav would be all dead ends. The brand and one column
//! down the middle, and the screen the machine is showing in it.

use client_state::ViewOnboarding;
use leptos::prelude::*;
use web_components::Brand;
use web_components::Onboarding;

use crate::runtime_app::RuntimeApp;

#[component]
pub fn OnboardingPage(view: Signal<ViewOnboarding>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    // Synchronous, straight from the click: Connect a bank opens Plaid's
    // window inside this call, and a browser refuses a window opened later.
    let ui = Callback::new(move |event| runtime.ui(event));

    // Welcome is the one screen of the flow that the phone opens on its mark.
    let welcome = move || view.with(|view| matches!(view, ViewOnboarding::Welcome { .. }));

    view! {
        <div class="onboarding ui" class:welcome=welcome>
            <div class="measure">
                <Brand />
                <Onboarding view=view ui=ui />
            </div>
        </div>
    }
}
