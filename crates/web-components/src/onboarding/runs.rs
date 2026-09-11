use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::read;
use super::read::part;

/// The body, in runs. A strong run is the sketch's `<b>` — the address on the
/// code screen, Plaid's name on the bank screen — set inline, so it wraps with
/// the sentence it is part of.
#[component]
pub(super) fn Runs(view: Signal<ViewOnboarding>) -> impl IntoView {
    let count = part(view, |view| read::body(view).len());

    view! {
        <div class="say-body">
            <For each=move || 0..count.get() key=|i| *i let:i>
                <Run view=view i=i />
            </For>
        </div>
    }
}

#[component]
fn Run(view: Signal<ViewOnboarding>, i: usize) -> impl IntoView {
    let run = part(view, move |view| {
        read::body(view)
            .get(i)
            .map(|run| (run.text.clone(), run.strong))
    });

    move || {
        run.get().map(|(text, strong)| {
            if strong {
                view! { <b>{text}</b> }.into_any()
            } else {
                text.into_any()
            }
        })
    }
}
