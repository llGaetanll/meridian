use client_state::ViewAccount;
use leptos::prelude::*;

#[component]
pub fn AccountPage(view: Signal<ViewAccount>) -> impl IntoView {
    let _ = view;

    view! { <div class="page">"Account"</div> }
}
