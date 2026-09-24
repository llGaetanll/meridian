use client_state::ViewOnboarding;
use leptos::html;
use leptos::prelude::*;
use web_ui::Glyph;

use super::read;
use super::read::part;

/// The one line a screen of the flow asks for, in the design language's field.
///
/// The value is the machine's: every keystroke goes out as the whole field,
/// and what comes back is written into the input as a property, which leaves
/// the caret where it was. Focused as it opens, since it is the only thing on
/// the screen to type into.
///
/// One component for the address and the name, because they are the same
/// control: what differs is the two hints the browser reads, which are the
/// screen's to hand over — `inputmode` puts the right keyboard under a thumb,
/// and `autocomplete` decides which of somebody's saved values is offered.
#[component]
pub(super) fn TextField(
    view: Signal<ViewOnboarding>,
    on_typed: Callback<String>,
    #[prop(into)] inputmode: String,
    #[prop(into)] autocomplete: String,
) -> impl IntoView {
    let icon = part(view, |view| {
        read::field(view)
            .map(|field| field.icon.clone())
            .unwrap_or_default()
    });
    let value = part(view, |view| {
        read::field(view)
            .map(|field| field.value.clone())
            .unwrap_or_default()
    });
    let placeholder = part(view, |view| {
        read::field(view)
            .map(|field| field.placeholder.clone())
            .unwrap_or_default()
    });
    let bad = part(view, |view| read::fault(view).is_some());

    // Whether the caret is in it, which only the browser knows.
    let focused = RwSignal::new(false);

    let input = NodeRef::<html::Input>::new();

    Effect::new(move || {
        if let Some(input) = input.get() {
            // A browser that refuses the focus leaves the field as it was.
            let _ = input.focus();
        }
    });

    let class = move || match (bad.get(), focused.get()) {
        (true, _) => "field bad",
        (false, true) => "field on",
        (false, false) => "field",
    };

    view! {
        <span class=class>
            <span class="in">
                <Glyph name=icon />
                <input
                    node_ref=input
                    type="text"
                    inputmode=inputmode
                    autocomplete=autocomplete
                    autocapitalize="off"
                    spellcheck="false"
                    placeholder=move || placeholder.get()
                    prop:value=move || value.get()
                    on:input=move |event| on_typed.run(event_target_value(&event))
                    on:focus=move |_| focused.set(true)
                    on:blur=move |_| focused.set(false)
                />
            </span>
        </span>
    }
}
