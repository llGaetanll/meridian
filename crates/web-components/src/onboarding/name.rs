use client_state::EventUi;
use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::Runs;
use super::TextField;
use super::read;
use super::read::part;

/// What to call them, and nothing else on the screen. Enter is Continue.
///
/// No chevron: the screens behind this one are the address and the code, and
/// an account exists by the time it is drawn. There is nothing to go back to
/// that would not mean signing in again.
///
/// The button is absent until there is something in the field rather than
/// present and dim, which is the code screen's rule and the same reason — a
/// dim button still invites a press.
#[component]
pub(super) fn Name(view: Signal<ViewOnboarding>, ui: Callback<EventUi>) -> impl IntoView {
    let step = part(view, |view| read::step(view).to_string());
    let title = part(view, |view| read::title(view).to_string());
    let action = part(view, |view| read::action(view).map(str::to_string));

    view! {
        <div class="say-step">{move || step.get()}</div>
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />

        <form
            class="entry"
            novalidate
            on:submit=move |event| {
                event.prevent_default();
                ui.run(EventUi::NameSubmitted);
            }
        >
            <TextField
                view=view
                on_typed=Callback::new(move |text| ui.run(EventUi::NameTyped { text }))
                inputmode="text"
                autocomplete="name"
            />

            <div class="acts">
                {move || {
                    action
                        .get()
                        .map(|label| {
                            view! {
                                <button type="submit" class="btn primary lg block">
                                    {label}
                                </button>
                            }
                        })
                }}
            </div>
        </form>
    }
}
