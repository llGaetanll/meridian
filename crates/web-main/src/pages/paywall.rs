//! What somebody sees when the session is good and has not paid.
//!
//! **Not an error screen.** Nothing has gone wrong: the account exists, the
//! token works, and the server is answering. What it says comes from the
//! reason the server sent — our own word for the state, never a provider's —
//! and the end of the period that was paid for.

use api_common::Timestamp;
use api_common::format::fmt_date;
use leptos::prelude::*;
use web_components::Brand;

use server_wire::UnpaidReason;
use server_wire::WireUnpaid;

/// The price, as `site` prints it and Stripe bills it. Not on the wire:
/// `/billing/status` says whether somebody has paid, never what it costs.
const PRICE: &str = "$5.99 a month";

#[component]
pub fn PaywallPage(unpaid: Option<WireUnpaid>) -> impl IntoView {
    // A refusal that carried no readable body says nothing about a past,
    // which is what `Never` is.
    let reason = unpaid
        .as_ref()
        .map(WireUnpaid::reason)
        .unwrap_or(UnpaidReason::Never);

    let said = Said::of(reason);

    // Always in the past. Entitlement is the date against the clock, so a
    // period still running is a session that never reached this screen.
    let ran_out = ran_out(reason, unpaid.as_ref()).map(fmt_date);

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

/// When the period that was paid for ended, where there was one.
///
/// Withheld on the two reasons that name no period: nothing was ever bought,
/// or we cannot say what was. A date under "Ran out" beside "we can't confirm
/// your subscription" would be the one hard fact on a screen admitting it has
/// none.
fn ran_out(reason: UnpaidReason, unpaid: Option<&WireUnpaid>) -> Option<Timestamp> {
    match reason {
        UnpaidReason::Never | UnpaidReason::Unconfirmed => None,
        UnpaidReason::Declined | UnpaidReason::Ended => unpaid?.ran_out.map(Timestamp::from_secs),
    }
}

/// The two sentences at the top, for the reason the server sent.
///
/// The same four the phone draws, in the same words — `phone-state`'s
/// `paywall` is the other half of this screen, and somebody who signs in on
/// both should not be told two different stories about one account.
struct Said {
    title: &'static str,
    body: &'static str,
}

impl Said {
    fn of(reason: UnpaidReason) -> Self {
        match reason {
            UnpaidReason::Never => Self {
                title: "Meridian is a subscription",
                body: "Your account is ready. There is one price and nothing else to \
                       choose, and you can leave whenever you like.",
            },

            // Nothing about a period here. `incomplete_expired` is a first
            // payment that never landed, so it never bought one.
            UnpaidReason::Declined => Self {
                title: "Your last payment didn't go through",
                body: "The card on file was declined. A working card opens the app \
                       back up.",
            },

            UnpaidReason::Ended => Self {
                title: "Your subscription has ended",
                body: "It ran to the end of the period you had paid for, and that has now \
                       passed. Subscribing again opens the app back up.",
            },

            // A payment still in flight, or a provider that believes this
            // person is paying. Naming a cause here would be inventing one.
            UnpaidReason::Unconfirmed => Self {
                title: "We can't confirm your subscription",
                body: "If you have just paid, give it a minute and reload. Nothing here \
                       has been lost.",
            },
        }
    }
}
