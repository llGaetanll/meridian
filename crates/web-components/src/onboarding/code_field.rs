use client_state::ViewCodeCell;
use client_state::ViewOnboarding;
use leptos::html;
use leptos::prelude::*;

use super::read;
use super::read::part;

/// A code: as many boxes as there are digits, and one real input behind all
/// of them.
///
/// One input rather than one per box, so a paste, a backspace and a code the
/// browser fills in are all the same event: the whole field, as it reads. The
/// machine keeps the digits and nothing else, and the boxes draw what it
/// kept. The input is invisible and on top, so a click anywhere on the boxes
/// lands in it.
#[component]
pub(super) fn CodeField(view: Signal<ViewOnboarding>, on_typed: Callback<String>) -> impl IntoView {
    let count = part(view, |view| read::cells(view).len());
    let digits = part(view, |view| {
        read::cells(view)
            .iter()
            .map(|cell| cell.digit.as_str())
            .collect::<String>()
    });

    // Whether the caret is in it, which only the browser knows. The live box
    // is drawn live only while it is.
    let focused = RwSignal::new(false);

    let input = NodeRef::<html::Input>::new();

    Effect::new(move || {
        if let Some(input) = input.get() {
            // A browser that refuses the focus leaves the field as it was.
            let _ = input.focus();
        }
    });

    view! {
        <div class="code">
            <For each=move || 0..count.get() key=|i| *i let:i>
                <Cell view=view i=i focused=focused.into() />
            </For>
            <input
                node_ref=input
                type="text"
                inputmode="numeric"
                autocomplete="one-time-code"
                prop:value=move || digits.get()
                on:input=move |event| on_typed.run(event_target_value(&event))
                on:focus=move |_| focused.set(true)
                on:blur=move |_| focused.set(false)
            />
        </div>
    }
}

/// One box. The box that takes the next digit carries the caret.
#[component]
fn Cell(view: Signal<ViewOnboarding>, i: usize, focused: Signal<bool>) -> impl IntoView {
    let digit = part(view, move |view| {
        cell(view, i)
            .map(|cell| cell.digit.clone())
            .unwrap_or_default()
    });
    let live = part(view, move |view| {
        cell(view, i).is_some_and(|cell| cell.live)
    });
    let bad = part(view, move |view| cell(view, i).is_some_and(|cell| cell.bad));

    let caret = move || live.get() && focused.get();

    let class = move || match (bad.get(), caret()) {
        (true, _) => "field bad",
        (false, true) => "field on",
        (false, false) => "field",
    };

    view! {
        <span class=class>
            <span class="in">
                {move || digit.get()}
                {move || caret().then(|| view! { <i class="caret" /> })}
            </span>
        </span>
    }
}

fn cell(view: &ViewOnboarding, i: usize) -> Option<&ViewCodeCell> {
    read::cells(view).get(i)
}
