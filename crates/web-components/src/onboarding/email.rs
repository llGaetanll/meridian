use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::Back;
use super::Fault;
use super::Runs;
use super::TextField;
use super::read;
use super::read::part;

/// The address, and nothing else on the screen. Enter is Continue.
#[component]
pub(super) fn Email(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let step = part(view, |view| read::step(view).to_string());
    let title = part(view, |view| read::title(view).to_string());
    let fault = part(view, |view| read::fault(view).map(str::to_string));
    let action = part(view, |view| {
        read::action(view).unwrap_or_default().to_string()
    });

    view! {
        <Back ui=ui />
        <div class="say-step">{move || step.get()}</div>
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />

        // The browser's own check on an address is off: whether it is one is
        // the machine's to say, in its own words.
        <form
            class="entry"
            novalidate
            on:submit=move |event| {
                event.prevent_default();
                ui.run(EventUi::EmailSubmitted);
            }
        >
            <TextField
                view=view
                on_typed=Callback::new(move |text| ui.run(EventUi::EmailTyped { text }))
                inputmode="email"
                autocomplete="email"
            />
            <Fault text=fault />

            <div class="acts">
                <button type="submit" class="btn primary lg block">
                    {move || action.get()}
                </button>
            </div>
        </form>
    }
}
