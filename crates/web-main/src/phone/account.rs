use client_state::EventUi;
use client_state::ViewAccount;
use leptos::prelude::*;
use web_components::BankList;
use web_components::Sec;

use crate::runtime_app::RuntimeApp;

/// Which banks are attached, and the way out, as the phone draws them.
#[component]
pub fn AccountPhone(view: Signal<ViewAccount>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    let text = move |read: fn(&ViewAccount) -> &String| {
        Signal::derive(move || view.with(|view| read(view).clone()))
    };

    let banks = Signal::derive(move || view.with(|view| view.banks.clone()));
    let empty = move || view.with(|view| view.banks_empty.clone());

    view! {
        <div class="phone-page account-phone">
            <Sec title=text(|view| &view.banks_label)>
                <Show
                    when=move || view.with(|view| view.banks_empty.is_none())
                    fallback=move || view! { <div class="banks-empty">{empty}</div> }
                >
                    <BankList banks=banks />
                </Show>
            </Sec>

            <Sec title=text(|view| &view.session_label)>
                // The one button on the tab, and quiet: nothing here is the
                // thing you came to do, so nothing here is violet.
                <div class="session">
                    <button class="btn lg block" on:click=move |_| runtime.ui(EventUi::SignOutRequested)>
                        {text(|view| &view.sign_out_label)}
                    </button>
                </div>
            </Sec>

            <div class="build">{text(|view| &view.build)}</div>
        </div>
    }
}
