use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::read;
use super::read::part;

/// What arrived, on the last screen of the flow: a figure and what it counts,
/// read down a column.
#[component]
pub(super) fn Tally(view: Signal<ViewOnboarding>) -> impl IntoView {
    let count = part(view, |view| read::tally(view).len());

    view! {
        <div class="tally">
            <For each=move || 0..count.get() key=|i| *i let:i>
                <Figure view=view i=i />
            </For>
        </div>
    }
}

#[component]
fn Figure(view: Signal<ViewOnboarding>, i: usize) -> impl IntoView {
    let figure = part(view, move |view| {
        read::tally(view)
            .get(i)
            .map(|tally| (tally.value.clone(), tally.label.clone()))
            .unwrap_or_default()
    });

    view! {
        <div class="r">
            <span class="v">{move || figure.get().0}</span>
            <span class="k">{move || figure.get().1}</span>
        </div>
    }
}
