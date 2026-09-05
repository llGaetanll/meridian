//! Which banks are attached, and the way out.
//!
//! Deliberately small. Everything a setting could configure already lives on
//! the screen it affects, so what is left for this tab is the connections and
//! the session.

use leptos::prelude::*;
use phone_state::EventUi;
use phone_state::ViewAccount;

use crate::runtime_app::use_runtime;
use crate::ui::PhoneShell;

#[component]
pub fn AccountScreen(view: ViewAccount) -> impl IntoView {
    let runtime = use_runtime();
    let banks = view.banks.clone();

    view! {
        <PhoneShell title="Account">
            <div class="me">
                <div class="me-head">{view.banks_label}</div>

                {match view.banks_empty {
                    Some(message) => view! { <div class="me-none">{message}</div> }.into_any(),
                    None => {
                        view! {
                            <For each=move || banks.clone() key=|bank| bank.id.clone() let:bank>
                                <div class="me-row" class:needs=bank.needs_reconnect>
                                    <span class="me-mark">{bank.initials}</span>
                                    <div class="me-id">
                                        <div class="me-name">{bank.name}</div>
                                        <div class="me-sub">{bank.detail}</div>
                                    </div>
                                </div>
                            </For>
                        }
                            .into_any()
                    }
                }}

                <div class="me-head">{view.session_label}</div>

                <button
                    class="me-act"
                    on:click=move |_| runtime.ui(EventUi::SignOutRequested)
                >
                    {view.sign_out_label}
                </button>

                <div class="me-build">{view.build}</div>
            </div>
        </PhoneShell>
    }
}
