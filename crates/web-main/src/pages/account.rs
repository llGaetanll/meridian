//! Who you are, which banks are attached, and the way out.
//!
//! Deliberately small: everything a setting could configure already lives on
//! the page it affects, so what is left is what belongs to the account rather
//! than to a page — the name the app calls you by, the banks, and the session.

use client_state::EventUi;
use client_state::ViewAccount;
use client_state::ViewAccountDelete;
use client_state::ViewAccountName;
use leptos::prelude::*;
use web_components::BankList;
use web_components::Confirm;
use web_components::Sec;
use web_ui::Glyph;

use crate::runtime_app::RuntimeApp;

#[component]
pub fn AccountPage(view: Signal<ViewAccount>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |field: fn(&ViewAccount) -> &String| {
        Signal::derive(move || view.with(|v| field(v).clone()))
    };

    let banks = Signal::derive(move || view.with(|v| v.banks.clone()));
    let empty = move || view.with(|v| v.banks_empty.clone());
    let delete = Signal::derive(move || view.with(|v| v.delete.clone()));
    let name = Signal::derive(move || view.with(|v| v.name.clone()));

    view! {
        <div class="page account-page">
            <div class="page-col">
                {move || name.get().map(|name| view! { <NameSection name=name /> })}

                <Sec title=text(|v| &v.banks_label)>
                    <Show
                        when=move || view.with(|v| v.banks_empty.is_none())
                        fallback=move || view! { <div class="banks-empty">{empty}</div> }
                    >
                        <BankList banks=banks />
                    </Show>
                </Sec>

                <Sec title=text(|v| &v.session_label)>
                    <div class="session-actions">
                        <button class="btn" on:click=move |_| runtime.ui(EventUi::SignOutRequested)>
                            {text(|v| &v.sign_out_label)}
                        </button>
                        <button
                            class="btn"
                            on:click=move |_| runtime.ui(EventUi::DeleteAccountRequested)
                        >
                            {text(|v| &v.delete_label)}
                        </button>
                    </div>
                </Sec>

                <div class="build">{text(|v| &v.build)}</div>
            </div>

            {move || delete.get().map(|confirm| view! { <DeleteDialog confirm=confirm /> })}
        </div>
    }
}

/// The name the app calls you by, and the field it is changed in.
///
/// Saved on Enter or on the button, never on a keystroke: a name saved as it
/// is typed is half a dozen writes and a chip that flickers through every
/// prefix of it. The button is absent while the field is blank or unchanged,
/// which is the same rule first run's Continue follows.
#[component]
fn NameSection(name: ViewAccountName) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    view! {
        <Sec title=name.label>
            <form
                class="account-name"
                on:submit=move |event| {
                    event.prevent_default();
                    runtime.ui(EventUi::NameSubmitted);
                }
            >
                <span class="field">
                    <span class="in">
                        <Glyph name=name.field.icon />
                        <input
                            type="text"
                            autocomplete="name"
                            placeholder=name.field.placeholder
                            prop:value=name.field.value
                            on:input=move |event| {
                                runtime.ui(EventUi::NameTyped { text: event_target_value(&event) })
                            }
                        />
                    </span>
                </span>

                <button type="submit" class="btn" class:dead=!name.can_save disabled=!name.can_save>
                    {name.save_label}
                </button>
            </form>
        </Sec>
    }
}

/// The delete confirmation, over the page.
///
/// The shared dialog, given the runtime the phone's pages take from the
/// context: the settings window draws the same one for the same step of the
/// same flow.
#[component]
pub fn DeleteDialog(confirm: ViewAccountDelete) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let ui = Callback::new(move |event: EventUi| runtime.ui(event));

    view! { <Confirm confirm=confirm ui=ui /> }
}
