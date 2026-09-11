use client_state::ViewBreakdownSlice;
use leptos::prelude::*;

/// A split read down a column: each part's name and figure, with its share
/// standing on the rule under it.
#[component]
pub fn Split(#[prop(into)] slices: Signal<Vec<ViewBreakdownSlice>>) -> impl IntoView {
    view! {
        // Keyed by everything a row says, so a row whose figure changes is
        // drawn again and the rest are left where they are.
        <For each=move || slices.get() key=|slice| format!("{slice:?}") let:slice>
            <div class="split-row">
                <span class="nm">{slice.label}</span>
                <span class="amt money">{slice.amount}</span>
                <span class="track">
                    <i style:width=format!("{:.1}%", slice.share * 100.0)></i>
                </span>
            </div>
        </For>
    }
}
