//! The pieces more than one screen draws.
//!
//! Every one of these takes a finished value from the core and prints it. If
//! you find yourself wanting a `format!` or an `if amount < 0.0` in this file,
//! the field is missing from the view type — add it there instead. That rule
//! is what keeps this crate a faithful preview of what SwiftUI and Compose
//! will do.

use api_core::AccountRow;
use api_core::BreakdownSlice;
use api_core::ConnectionView;
use api_core::StreamRow;
use api_core::TxRow;
use api_core::UiEvent;
use leptos::prelude::*;

use crate::runtime::use_runtime;
use crate::ui::Icon;

/// One transaction. The same row on Home and in the ledger, at two sizes.
#[component]
pub fn TxRowView(row: TxRow, #[prop(optional)] compact: bool) -> impl IntoView {
    let runtime = use_runtime();
    let id = row.id.clone();

    let class = if compact { "hi-row" } else { "tx" };
    let class = if row.pending {
        format!("{class} pending")
    } else {
        class.to_string()
    };

    let amount_class = if row.is_inflow {
        "tx-amt num up"
    } else {
        "tx-amt num"
    };

    view! {
        <div
            class=class
            on:click=move |_| {
                runtime.ui(UiEvent::TxSelected { id: id.clone() });
            }
        >
            <Logo url=row.logo_url compact=compact />
            <div class="tx-id">
                <div class="tx-name">{row.title}</div>
                <div class="tx-meta">
                    {row.pending.then(|| view! { <span class="pending-tag">"Pending"</span> })}
                    // Home previews the category as plain text rather than a
                    // chip: at preview scale a row of pills reads as noise,
                    // and the sketch's Home rows are text too.
                    {if compact {
                        view! { <span class="cat-plain">{row.category}</span> }.into_any()
                    } else {
                        view! {
                            <span class=format!("cat {}", row.category_token)>{row.category}</span>
                        }
                            .into_any()
                    }}
                </div>
            </div>
            <div class=amount_class>{row.amount}</div>
        </div>
    }
}

/// The logo slot. Reserved whether or not there is a logo, so every row's text
/// starts on the same line — the sketch is explicit that there is no stand-in
/// glyph.
#[component]
pub fn Logo(url: Option<String>, #[prop(optional)] compact: bool) -> impl IntoView {
    let class = if url.is_some() {
        "tx-logo"
    } else {
        "tx-logo none"
    };

    let class = if compact {
        format!("{class} small")
    } else {
        class.to_string()
    };

    view! {
        <div class=class>
            {url
                .map(|url| {
                    view! {
                        // `onerror` blanks the slot rather than leaving a
                        // broken-image glyph: Plaid's logo host is reachable
                        // from a phone but not from every network.
                        <img
                            src=url
                            alt=""
                            loading="lazy"
                            on:error=|ev| {
                                use leptos::wasm_bindgen::JsCast;
                                if let Some(img) = ev
                                    .target()
                                    .and_then(|t| t.dyn_into::<web_sys::HtmlElement>().ok())
                                {
                                    let _ = img.style().set_property("display", "none");
                                }
                            }
                        />
                    }
                })}
        </div>
    }
}

/// One account on the balances screen.
#[component]
pub fn AccountRowView(row: AccountRow) -> impl IntoView {
    let amount_class = if row.owed {
        "acct-amt num owed"
    } else {
        "acct-amt num"
    };

    view! {
        <div class="acct">
            <div class="acct-id">
                <div class="acct-name">{row.name}</div>
                <div class="acct-sub">{row.subtitle}</div>
                {row.utilisation.map(|text| view! { <div class="acct-util">{text}</div> })}
            </div>
            <div class=amount_class>{row.amount}</div>
        </div>
    }
}

/// One connection: its accounts, its total, and its state.
#[component]
pub fn ConnectionBlock(connection: ConnectionView) -> impl IntoView {
    let class = if connection.is_debts {
        "conn debts"
    } else {
        "conn"
    };

    let total_class = if connection.is_debts {
        "v num owed"
    } else {
        "v num"
    };

    view! {
        <div class=class>
            <div class="conn-head">
                <div class="conn-logo">{connection.initials}</div>
                <div class="conn-name">{connection.name}</div>
                {if connection.needs_reconnect {
                    view! { <ReconnectButton /> }.into_any()
                } else if connection.is_debts {
                    view! { <span class="conn-tag">"Owed"</span> }.into_any()
                } else {
                    view! { <span class="conn-sync">{connection.synced}</span> }.into_any()
                }}
            </div>

            <For
                each=move || connection.accounts.clone()
                key=|account| account.id.clone()
                let:account
            >
                <AccountRowView row=account />
            </For>

            <div class="conn-total">
                <span class="k">{connection.total_label}</span>
                <span class=total_class>{connection.total}</span>
            </div>
        </div>
    }
}

/// The action a stale connection needs.
///
/// Inert, and deliberately so: clearing `ITEM_LOGIN_REQUIRED` takes Plaid Link
/// in update mode, which is a hosted page and a server round trip that do not
/// exist yet. A button that looked live and did nothing would be worse than
/// one that says what it is waiting for.
#[component]
pub fn ReconnectButton() -> impl IntoView {
    view! {
        <button class="fix" title="Plaid Link update mode is not wired up yet" disabled>
            "Reconnect"
        </button>
    }
}

/// One recurring stream.
#[component]
pub fn StreamRowView(row: StreamRow) -> impl IntoView {
    let when_class = if row.soon {
        "rec-when soon"
    } else {
        "rec-when"
    };

    view! {
        <div class="rec">
            <Logo url=row.logo_url />
            <div class="rec-id">
                <div class="rec-name">{row.title}</div>
                <div class="rec-meta">
                    <span class=when_class>{row.when}</span>
                </div>
            </div>
            <div class="rec-right">
                <div class="rec-amt num">{row.amount}</div>
                <div class="rec-freq">{row.frequency}</div>
            </div>
        </div>
    }
}

/// The breakdown bar and its legend — two views of one split, so they are
/// drawn from one list.
#[component]
pub fn Breakdown(slices: Vec<BreakdownSlice>, #[prop(optional)] legend: bool) -> impl IntoView {
    let bar = slices.clone();

    view! {
        <div class="rec-bar">
            <For each=move || bar.clone() key=|slice| slice.label.clone() let:slice>
                <i
                    class=slice.token.clone()
                    style=format!("flex:{}", slice.share.max(0.0))
                ></i>
            </For>
        </div>

        {legend
            .then(|| {
                view! {
                    <div class="rec-legend">
                        <For
                            each=move || slices.clone()
                            key=|slice| slice.label.clone()
                            let:slice
                        >
                            <div class=format!("item {}", slice.token)>
                                <i class="sw"></i>
                                <span class="nm">{slice.label}</span>
                                <span class="amt num">{slice.amount}</span>
                            </div>
                        </For>
                    </div>
                }
            })}
    }
}

/// A figure that can be covered.
///
/// The core already replaced the value with dots when masked, so this only
/// draws the affordance. The real figure is never in the DOM while covered —
/// which is stricter than the sketch, where it sat underneath at zero opacity.
#[component]
pub fn Masked(
    #[prop(into)] value: String,
    masked: bool,
    #[prop(optional)] tappable: bool,
) -> impl IntoView {
    let runtime = use_runtime();

    let class = match (masked, tappable) {
        (true, true) => "masked tappable",
        (true, false) => "masked",
        (false, _) => "",
    };

    view! {
        <span
            class=class
            on:click=move |_| {
                if tappable {
                    runtime.ui(UiEvent::MaskToggled);
                }
            }
        >
            {if masked && tappable {
                view! {
                    <span class="hint">
                        <Icon name="eye" />
                        "Tap to reveal"
                    </span>
                }
                    .into_any()
            } else {
                view! { <span class="val">{value}</span> }.into_any()
            }}
        </span>
    }
}
