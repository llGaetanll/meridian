use leptos::prelude::*;

/// The headline, once it has scrolled by: what it is a figure for, and the
/// figure.
#[component]
pub fn BalancesBand(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] headline: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="band">
            <span class="k">{label}</span>
            <span class="end fig money">{headline}</span>
        </div>
    }
}
