use client_state::EventUi;
use client_state::ViewRecurring;
use leptos::prelude::*;
use web_components::Calendar;
use web_components::DayCard;
use web_components::Hero;
use web_components::Sec;
use web_components::Split;
use web_components::StreamList;

use crate::runtime_app::RuntimeApp;

/// What comes back: the net of it and where it goes, every stream behind it,
/// and in the rail the month it lands on.
#[component]
pub fn RecurringPage(view: Signal<ViewRecurring>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |read: fn(&ViewRecurring) -> &String| {
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

                <Sec title=text(|view| &view.breakdown_label) value=text(|view| &view.breakdown_total)>
                    <Split slices=Signal::derive(move || view.with(|view| view.breakdown.clone())) />
                </Sec>

                <Sec title=text(|view| &view.streams_label) value=text(|view| &view.streams_count)>
                    <StreamList groups=Signal::derive(move || view.with(|view| view.groups.clone())) />
                </Sec>
            </div>

            <aside class="rail">
                <div class="rail-sec">
                    <Calendar
                        calendar=Signal::derive(move || view.with(|view| view.calendar.clone()))
                        on_pick=Callback::new(move |id| runtime.ui(EventUi::CalendarDaySelected { id }))
                        on_step=Callback::new(move |step| runtime.ui(EventUi::CalendarMonthStepped { step }))
                    />
                    <DayCard card=Signal::derive(move || view.with(|view| view.day_card.clone())) />
                </div>
            </aside>
        </div>
    }
}
