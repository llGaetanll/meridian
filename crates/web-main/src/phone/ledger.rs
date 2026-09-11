use client_state::EventUi;
use client_state::ViewLedger;
use leptos::prelude::*;
use web_components::FilterRail;
use web_components::LedgerList;

use crate::runtime_app::RuntimeApp;

/// The ledger as the phone draws it: the filter, always open, and under it
/// everything the filter lets through, newest first.
///
/// No figure at its head. Once the filter has scrolled by, the band says what
/// it is set to instead.
#[component]
pub fn LedgerPhone(view: Signal<ViewLedger>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    view! {
        <div class="phone-page ledger-phone">
            <div data-band-anchor="">
                <FilterRail
                    filter=part(view, |view| &view.filter)
                    on_toggle=Callback::new(move |id| runtime.ui(EventUi::CategoryToggled { id }))
                    on_range=Callback::new(move |kind| runtime.ui(EventUi::RangeSelected { kind }))
                />
            </div>

            <LedgerList
                months=part(view, |view| &view.months)
                empty=part(view, |view| &view.empty)
                on_select=Callback::new(move |id| runtime.ui(EventUi::TxSelected { id }))
            />
        </div>
    }
}

/// One part of the view, taken out once per render. The list reads its months
/// once for every day on it, and a derived signal would copy them each time.
fn part<T>(view: Signal<ViewLedger>, read: fn(&ViewLedger) -> &T) -> Signal<T>
where
    T: Clone + Send + Sync + 'static,
{
    Memo::new_with_compare(move |_| view.with(|view| read(view).clone()), |_, _| true).into()
}
