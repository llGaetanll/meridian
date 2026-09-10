//! What somebody sees when the session is good and has not paid.
//!
//! **Not an error screen.** Nothing has gone wrong: the account exists, the
//! token works, and the server is answering. What it says comes from
//! `GET /billing/status` — the provider's word for the state, and the end of
//! the period that was paid for.

use api_common::format::fmt_date;
use leptos::prelude::*;
use web_components::Brand;

use crate::io::BillingStatus;

/// The price, as `site` prints it and Stripe bills it. Not on the wire:
/// `/billing/status` says whether somebody has paid, never what it costs.
const PRICE: &str = "$5.99 a month";

#[component]
pub fn PaywallPage(billing: Option<BillingStatus>) -> impl IntoView {
    let status = billing
        .as_ref()
        .and_then(|billing| billing.status.as_deref());
    let said = Said::of(status);

    // Always in the past. Entitlement is the date against the clock, so a
    // period still running is a session that never reached this screen.
    let ran_out = billing
        .as_ref()
        .and_then(|billing| billing.good_until)
        .map(fmt_date);

    view! {
        <div class="paywall ui">
            <div class="measure">
                <Brand />

                <div class="say-title">{said.title}</div>
                <div class="say-body">{said.body}</div>

                <div class="facts">
                    <div class="row flat">
                        <div class="rt">Price</div>
                        <span class="rv hard money">{PRICE}</span>
                    </div>
                    {ran_out
                        .map(|date| {
                            view! {
                                <div class="row flat">
                                    <div class="rt">Ran out</div>
                                    <span class="rv hard">{date}</span>
                                </div>
                            }
                        })}
                </div>

                // Drawn and not built: the card form is its own change, and a
                // button that looked live and did nothing would be worse.
                <div class="acts">
                    <button class="btn primary lg block dead" disabled>
                        Subscribe
                    </button>
                </div>
            </div>
        </div>
    }
}

/// The two sentences at the top, for the state the provider names.
struct Said {
    title: &'static str,
    body: &'static str,
}

impl Said {
    fn of(status: Option<&str>) -> Self {
        match status {
            None => Self {
                title: "Meridian is a subscription",
                body: "Your account is ready. There is one price and nothing else to \
                       choose, and you can leave whenever you like.",
            },

            // A card that stopped working. `past_due` while the period is
            // still paid for never reaches this screen, so by the time it is
            // read here the period has run out as well.
            Some("past_due" | "unpaid" | "incomplete" | "incomplete_expired") => Self {
                title: "Your last payment didn't go through",
                body: "The card on file was declined and the period you had paid for has \
                       since run out. A working card starts it again.",
            },

            // `canceled`, and any word we have no line for: there was a
            // subscription, and the period it paid for has passed.
            Some(_) => Self {
                title: "Your subscription has ended",
                body: "It ran to the end of the period you had paid for, and that has now \
                       passed. Subscribing again opens the app back up.",
            },
        }
    }
}
