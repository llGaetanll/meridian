use client_state::EventUi;
use client_state::ViewRecurring;
use leptos::prelude::*;
use web_components::CalendarStrip;
use web_components::DayCard;
use web_components::Hero;
use web_components::Sec;
use web_components::Split;
use web_components::StreamGroups;

use crate::runtime_app::RuntimeApp;

/// What comes back, as the phone draws it: the net of it, split by category,
/// then the month it lands on and every stream behind it, in one column.
#[component]
pub fn RecurringPhone(view: Signal<ViewRecurring>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |read: fn(&ViewRecurring) -> &String| {
        Signal::derive(move || view.with(|view| read(view).clone()))
    };

    view! {
        <div class="phone-page recurring-phone">
            <div data-band-anchor="">
                <Hero
                    label=text(|view| &view.headline_label)
                    value=text(|view| &view.headline)
                    flow_in=text(|view| &view.flow_in)
                    flow_out=text(|view| &view.flow_out)
                />
            </div>

            <Sec title=text(|view| &view.breakdown_label) value=text(|view| &view.breakdown_total)>
                <Split slices=Signal::derive(move || view.with(|view| view.breakdown.clone())) />
            </Sec>

            // No heading: the calendar is the split above it drawn a second
            // way, and a title over it would name the same thing twice.
            <div class="sec">
                <CalendarStrip
                    calendar=Signal::derive(move || view.with(|view| view.calendar.clone()))
                    on_pick=Callback::new(move |id| runtime.ui(EventUi::CalendarDaySelected { id }))
                    on_step=Callback::new(move |step| runtime.ui(EventUi::CalendarMonthStepped { step }))
                />
                <DayCard card=Signal::derive(move || view.with(|view| view.day_card.clone())) />
            </div>

            <Sec title=text(|view| &view.streams_label) value=text(|view| &view.streams_count)>
                <StreamGroups groups=Signal::derive(move || view.with(|view| view.groups.clone())) />
            </Sec>
        </div>
    }
}
