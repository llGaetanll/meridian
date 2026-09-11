use client_state::ViewIconRow;
use client_state::ViewOnboarding;
use client_state::ViewRowTone;
use leptos::prelude::*;

use super::IconRow;
use super::read;
use super::read::part;

/// The rows a screen of first run lists, whatever they are about: what
/// connecting a bank means, what the first sync has fetched, what is still
/// true after a failure.
///
/// Kept by position, so a row the sync moves on is the same row with new
/// words in it.
#[component]
pub(super) fn IconRows(view: Signal<ViewOnboarding>) -> impl IntoView {
    let count = part(view, |view| read::rows(view).len());

    view! {
        <div class="rows">
            <For each=move || 0..count.get() key=|i| *i let:i>
                <IconRow row=Signal::derive(move || {
                    view.with(|view| read::rows(view).get(i).cloned().unwrap_or_else(blank))
                }) />
            </For>
        </div>
    }
}

/// A row with nothing in it, for the moment a render has taken the list away
/// and the row has not yet come down.
fn blank() -> ViewIconRow {
    ViewIconRow {
        icon: String::new(),
        tone: ViewRowTone::Plain,
        title: String::new(),
        sub: None,
        value: None,
    }
}
