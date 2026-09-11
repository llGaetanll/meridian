use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::Back;
use super::CodeField;
use super::Fault;
use super::Resend;
use super::Runs;
use super::read;
use super::read::part;

/// Six boxes and a Continue that is there once all six are in.
///
/// Typing never sends the code: a wrong sixth digit would spend the guess
/// before anyone had seen it. Enter does, as Continue does.
#[component]
pub(super) fn Code(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let step = part(view, |view| read::step(view).to_string());
    let title = part(view, |view| read::title(view).to_string());
    let fault = part(view, |view| read::fault(view).map(str::to_string));
    let action = part(view, |view| read::action(view).map(str::to_string));

    view! {
        <Back ui=ui />
        <div class="say-step">{move || step.get()}</div>
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />

        <form
            class="entry"
            on:submit=move |event| {
                event.prevent_default();
                ui.run(EventUi::CodeSubmitted);
            }
        >
            <CodeField view=view on_typed=Callback::new(move |digits| ui.run(EventUi::CodeTyped { digits })) />
            <Fault text=fault />

            {move || {
                action
                    .get()
                    .map(|action| {
                        view! {
                            <div class="acts">
                                <button type="submit" class="btn primary lg block">
                                    {action}
                                </button>
                            </div>
                        }
                    })
            }}
        </form>

        <Resend view=view ui=ui />
    }
}
