use client_state::ViewRecurring;
use leptos::prelude::*;

#[component]
pub fn RecurringPage(view: Signal<ViewRecurring>) -> impl IntoView {
    let _ = view;

    view! { <div class="page">"Recurring"</div> }
}
