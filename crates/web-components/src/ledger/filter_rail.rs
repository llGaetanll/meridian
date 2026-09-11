use client_state::RangeSelection;
use client_state::ViewFilter;
use leptos::prelude::*;

use super::CategoryRows;
use super::RangeControl;

/// The filter, in a column of its own beside the list it narrows.
///
/// Both groups are always open: the phone folds them because it has one
/// column of room, and here what the filter is set to is never something the
/// list has to say on its behalf.
#[component]
pub fn FilterRail(
    filter: Signal<ViewFilter>,
    #[prop(into)] on_toggle: Callback<String>,
    #[prop(into)] on_range: Callback<RangeSelection>,
) -> impl IntoView {
    // Held once per render: every chip reads the list, and a derived signal
    // would build it again for each of them.
    let chips = Memo::new_with_compare(
        move |_| filter.with(|filter| filter.categories.clone()),
        |_, _| true,
    );
    let range = Memo::new_with_compare(
        move |_| filter.with(|filter| filter.range.clone()),
        |_, _| true,
    );

    view! {
        <aside class="rail">
            <div class="rail-sec">
                <div class="rail-head">
                    <span class="rail-title">"Categories"</span>
                </div>
                <CategoryRows chips=chips.into() on_toggle=on_toggle />
            </div>

            <div class="rail-sec">
                <div class="rail-head">
                    <span class="rail-title">"Time range"</span>
                </div>
                <RangeControl range=range.into() on_select=on_range />
            </div>
        </aside>
    }
}
