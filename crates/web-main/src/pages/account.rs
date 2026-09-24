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
        <div class="account-page">
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

                {name
                    .action
                    .map(|label| view! { <button type="submit" class="btn">{label}</button> })}
            </form>
        </Sec>
    }
}

/// The delete confirmation, over the page.
///
/// A placeholder rather than a design: the board rules out an alert for this
/// and draws a pushed screen (`sketches/mobile/account.html`), and the
/// design-language sheet has no dialog. This overlay stands in until that
/// pass; every word on it comes from the machine.
#[component]
pub fn DeleteDialog(confirm: ViewAccountDelete) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let has_code = confirm.code.is_some();

    view! {
        <div class="dialog-overlay">
            <div class="dialog">
                <h2>{confirm.title}</h2>
                <p>{confirm.body}</p>

                {confirm.code.map(|digits| view! {
                    <input
                        class="dialog-code"
                        type="text"
                        inputmode="numeric"
                        autocomplete="one-time-code"
                        prop:value=digits
                        on:input=move |event| runtime.ui(EventUi::DeleteCodeTyped {
                            digits: event_target_value(&event),
                        })
                    />
                })}

                {confirm.error.map(|error| view! { <p class="dialog-error">{error}</p> })}

                <div class="dialog-actions">
                    // The loud button is the safe one.
                    {confirm.cancel_label.map(|label| view! {
                        <button
                            class="btn primary"
                            on:click=move |_| runtime.ui(EventUi::DeleteCancelled)
                        >
                            {label}
                        </button>
                    })}
                    {confirm.confirm_label.map(|label| view! {
                        <button
                            class="btn"
                            on:click=move |_| runtime.ui(match has_code {
                                true => EventUi::DeleteCodeSubmitted,
                                false => EventUi::DeleteConfirmed,
                            })
                        >
                            {label}
                        </button>
                    })}
                    {confirm.busy.then(|| view! {
                        <button class="btn busy" disabled>"One moment…"</button>
                    })}
                </div>
            </div>
        </div>
    }
}
