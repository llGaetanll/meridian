//! The phone's frame: a title, a scrolling body, and a footer that holds the
//! one action the screen is for.
//!
//! This is deliberately not `web_components::Shell` — that one is a desktop
//! sidebar. If a second phone screen wants this frame it moves into
//! `web-components` then, not before.

use leptos::prelude::*;

#[component]
pub fn PhoneShell(
    #[prop(into)] title: String,
    #[prop(optional, into)] footer: Option<ViewFn>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="phone-shell">
            <header class="phone-header">
                <div class="phone-title">{title}</div>
            </header>

            <main class="phone-body">{children()}</main>

            {footer
                .map(|footer| {
                    view! { <footer class="phone-footer">{footer.run()}</footer> }
                })}
        </div>
    }
}
