use leptos::prelude::*;
use web_ui::Glyph;

/// The eye on a covered figure. One switch for every figure in the app.
///
/// An open eye while the figures are covered, because that is what pressing
/// it does: it shows them.
#[component]
pub fn MaskButton(
    #[prop(into)] masked: Signal<bool>,
    /// What pressing it does, for the tooltip and a screen reader.
    #[prop(into)]
    label: Signal<String>,
    on_toggle: Callback<()>,
) -> impl IntoView {
    let glyph = Signal::derive(move || if masked.get() { "eye" } else { "eye-off" }.to_string());

    view! {
        <button
            class="btn plain"
            title=label
            aria-label=label
            aria-pressed=move || masked.get().to_string()
            on:click=move |_| on_toggle.run(())
        >
            <Glyph name=glyph />
        </button>
    }
}
