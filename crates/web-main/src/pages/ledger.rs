use client_state::EventUi;
use client_state::ViewLedger;
use leptos::prelude::*;
use web_components::FilterRail;
use web_components::Hero;
use web_components::LedgerList;

use crate::runtime_app::RuntimeApp;

/// Everything that has happened, newest first, under the figure it comes to,
/// and in the rail the filter that narrows it.
#[component]
pub fn LedgerPage(view: Signal<ViewLedger>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |read: fn(&ViewLedger) -> &String| {
        Signal::derive(move || view.with(|view| read(view).clone()))
    };

    view! {
        <div class="page">
            <div class="page-col">
                <Hero
                    label=text(|view| &view.headline_label)
                    value=text(|view| &view.headline)
                    flow_in=text(|view| &view.flow_in)
                    flow_out=text(|view| &view.flow_out)
                />

                <LedgerList
                    months=part(view, |view| &view.months)
                    empty=part(view, |view| &view.empty)
                    on_select=Callback::new(move |id| runtime.ui(EventUi::TxSelected { id }))
                />
            </div>

            <FilterRail
                filter=part(view, |view| &view.filter)
                on_toggle=Callback::new(move |id| runtime.ui(EventUi::CategoryToggled { id }))
                on_range=Callback::new(move |kind| runtime.ui(EventUi::RangeSelected { kind }))
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
