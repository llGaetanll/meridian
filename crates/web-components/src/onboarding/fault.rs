use leptos::prelude::*;
use web_ui::Glyph;

/// What the app is waiting for, under the thing it is waiting on. Absent
/// until something has actually gone wrong.
#[component]
pub(super) fn Fault(#[prop(into)] text: Signal<Option<String>>) -> impl IntoView {
    move || {
        text.get().map(|text| {
            view! {
                <div class="say-fault">
                    <Glyph name="warn" />
                    {text}
                </div>
            }
        })
    }
}
