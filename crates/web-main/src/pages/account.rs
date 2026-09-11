//! Which banks are attached, and the way out.
//!
//! Deliberately small: everything a setting could configure already lives on
//! the page it affects, so what is left here is the banks and the session. No
//! address at the top, because nothing the server sends says who is signed in.

use client_state::EventUi;
use client_state::ViewAccount;
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
                <button class="btn" on:click=move |_| runtime.ui(EventUi::SignOutRequested)>
                    {text(|v| &v.sign_out_label)}
                </button>
            </Sec>

            <div class="build">{text(|v| &v.build)}</div>
        </div>
    }
}
