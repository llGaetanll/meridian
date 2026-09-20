//! Which banks are attached, and the way out.
//!
//! Deliberately small: everything a setting could configure already lives on
//! the page it affects, so what is left here is the banks and the session. No
//! address at the top, because nothing the server sends says who is signed in.

use client_state::EventUi;
use client_state::ViewAccount;
use client_state::ViewAccountDelete;
use leptos::prelude::*;
use web_components::BankList;
use web_components::Sec;

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

    view! {
        <div class="account-page">
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
