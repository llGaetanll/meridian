use client_state::EventUi;
use client_state::ViewOnboarding;
use client_state::ViewResend;
use leptos::prelude::*;

use super::read;
use super::read::part;

/// The way out of a code that did not work: a countdown that gives way to a
/// button, rather than a button that does nothing for the first minute.
///
/// The countdown is the one thing on the code screen that changes every
/// second, so it is the one thing here that redraws when it does.
#[component]
pub(super) fn Resend(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let ready = part(view, |view| {
        matches!(read::resend(view), Some(ViewResend::Ready { .. }))
    });

    let countdown = part(view, |view| match read::resend(view) {
        Some(ViewResend::Waiting { text, .. }) => text.clone(),
        Some(ViewResend::Ready { .. }) | None => String::new(),
    });
    let wrong_address = part(view, |view| match read::resend(view) {
        Some(ViewResend::Waiting { wrong_address, .. }) => wrong_address.clone(),
        Some(ViewResend::Ready { .. }) | None => String::new(),
    });
    let label = part(view, |view| match read::resend(view) {
        Some(ViewResend::Ready { label }) => label.clone(),
        Some(ViewResend::Waiting { .. }) | None => String::new(),
    });

    move || {
        if ready.get() {
            view! {
                <button
                    type="button"
                    class="btn lg block resend"
                    on:click=move |_| ui.run(EventUi::ResendRequested)
                >
                    {move || label.get()}
                </button>
            }
            .into_any()
        } else {
            // The address is one click from being fixed. A typo in the step
            // before is otherwise a dead end: the code never arrives and
            // nothing here says why.
            view! {
                <div class="say-note">
                    {move || countdown.get()}
                    " · "
                    <button
                        type="button"
                        class="btn plain"
                        on:click=move |_| ui.run(EventUi::BackPressed)
                    >
                        {move || wrong_address.get()}
                    </button>
                </div>
            }
            .into_any()
        }
    }
}
