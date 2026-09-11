//! What somebody sees when the session is good and has not paid.
//!
//! **Not an error screen.** Nothing has gone wrong: the account exists, the
//! token works, and the server is answering. Every word on it is the
//! machine's, so the phone and the web say the same thing about one account.

use client_state::EventUi;
use client_state::ViewPaywall;
use leptos::prelude::*;
use web_components::Brand;
use web_ui::Glyph;

use crate::runtime_app::RuntimeApp;

#[component]
pub fn PaywallPage(view: Signal<ViewPaywall>) -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();

    view! {
        <div class="paywall ui">
            <div class="measure">
                <Brand />

                <div class="say-title">{move || view.with(|view| view.title.clone())}</div>
                <div class="say-body">
                    {move || {
                        view.with(|view| {
                            view.body
                                .iter()
                                .map(|run| match run.strong {
                                    true => view! { <b>{run.text.clone()}</b> }.into_any(),
                                    false => run.text.clone().into_any(),
                                })
                                .collect_view()
                        })
                    }}
                </div>

                <div class="facts">
                    {move || {
                        view.with(|view| {
                            view.rows
                                .iter()
                                .map(|row| {
                                    view! {
                                        <div class="row">
                                            <Glyph name=row.icon.clone() />
                                            <div class="rt">
                                                {row.title.clone()}
                                                {row
                                                    .sub
                                                    .clone()
                                                    .map(|sub| view! { <div class="rs">{sub}</div> })}
                                            </div>
                                            {row
                                                .value
                                                .clone()
                                                .map(|value| {
                                                    view! { <span class="rv hard money">{value}</span> }
                                                })}
                                        </div>
                                    }
                                })
                                .collect_view()
                        })
                    }}
                </div>

                <div class="acts">
                    <button
                        class="btn lg block"
                        on:click=move |_| runtime.ui(EventUi::SignOutRequested)
                    >
                        {move || view.with(|view| view.sign_out.clone())}
                    </button>

                    // Drawn and not built: the card form is its own change, and a
                    // button that looked live and did nothing would be worse.
                    <button class="btn primary lg block dead" disabled>
                        Subscribe
                    </button>
                </div>
            </div>
        </div>
    }
}
