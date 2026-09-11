use client_state::ViewOnboarding;
use leptos::prelude::*;

#[component]
pub fn OnboardingPage(view: Signal<ViewOnboarding>) -> impl IntoView {
    let _ = view;

    view! { <div class="page">"First run"</div> }
}
