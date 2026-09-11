use leptos::prelude::*;

/// The headline, once it has scrolled by. The two directions go beside it
/// signed: there is no room here for the words that carry them on the hero.
#[component]
pub fn RecurringBand(
    #[prop(into)] headline: Signal<String>,
    #[prop(into)] flow_in: Signal<String>,
    #[prop(into)] flow_out: Signal<String>,
) -> impl IntoView {
    view! {
        <div class="band">
            <span class="fig money">{headline}</span>
            <span class="end flows">
                <span class="money in">{flow_in}</span>
                <span class="money out">{flow_out}</span>
            </span>
        </div>
    }
}
