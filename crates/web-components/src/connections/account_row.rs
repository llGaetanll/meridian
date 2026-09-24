use client_state::ViewAccountRow;
use leptos::prelude::*;

/// One account: its name over its last four digits and what kind it is, and
/// what it comes to.
///
/// The digits lead the second line rather than following the kind, because
/// they are what tells two checking accounts apart. A row behind an expired
/// login is greyed whole — it is the row that is stale, not one fact in it.
#[component]
pub fn AccountRow(#[prop(into)] row: Signal<ViewAccountRow>) -> impl IntoView {
    let mask = move || {
        let mask = row.with(|row| row.mask.clone());

        (!mask.is_empty()).then(|| view! { <span class="mask">{mask}</span> })
    };

    view! {
        <div class="row flat" class:stale=move || row.with(|row| row.stale)>
            <div class="rt">
                {move || row.with(|row| row.name.clone())}
                <div class="rs">{mask} {move || row.with(|row| row.kind.clone())}</div>
            </div>

            <span class="rv hard money">{move || row.with(|row| row.amount.clone())}</span>
        </div>
    }
}
