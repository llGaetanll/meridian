//! Home: the highlight of each tab and nothing more.
//!
//! Every block is a door to the page that owns the detail. No "see all" link —
//! the dock is right there, and the block's title already says where this
//! lives.

use leptos::prelude::*;
use phone_state::EventUi;
use phone_state::Screen;
use phone_state::ViewHome;

use crate::runtime_app::use_runtime;
use crate::ui::Breakdown;
use crate::ui::ConnectionBlock;
use crate::ui::Masked;
use crate::ui::PhoneShell;
use crate::ui::StreamRowView;
use crate::ui::TxRowView;

#[component]
pub fn HomeScreen(view: ViewHome) -> impl IntoView {
    let ledger = view.ledger.clone();
    let upcoming = view.recurring.upcoming.clone();
    let attention = view.attention.clone();

    view! {
        <PhoneShell>
            <div class="home">
                <Block title="Balances" screen=Screen::Balances>
                    <div class="hi-lead num">
                        <Masked
                            value=view.balances.headline
                            masked=view.balances.masked
                            tappable=true
                        />
                    </div>
                    <div class="hi-sub">{view.balances.detail}</div>
                </Block>

                <Block title="Recurring" screen=Screen::Recurring>
                    <div class="hi-lead num">
                        <Masked
                            value=view.recurring.headline
                            masked=view.recurring.masked
                            tappable=true
                        />
                        <span class="per">"/mo"</span>
                    </div>

                    <Breakdown slices=view.recurring.breakdown legend=true />

                    <div class="hi-sub">{view.recurring.detail}</div>

                    <For each=move || upcoming.clone() key=|row| row.id.clone() let:row>
                        <StreamRowView row=row />
                    </For>
                </Block>

                <Block title="Ledger" screen=Screen::Ledger>
                    <For each=move || ledger.clone() key=|row| row.id.clone() let:row>
                        <TxRowView row=row compact=true />
                    </For>
                </Block>

                // Only drawn when something actually needs attention — a
                // healthy connection is not news.
                {(!attention.is_empty())
                    .then(|| {
                        view! {
                            <div class="hi">
                                <div class="hi-head">
                                    <span class="hi-title">"Connections"</span>
                                </div>
                                {attention
                                    .iter()
                                    .map(|connection| {
                                        view! { <ConnectionBlock connection=connection.clone() /> }
                                    })
                                    .collect_view()}
                            </div>
                        }
                    })}
            </div>
        </PhoneShell>
    }
}

/// One block, named for the tab it previews and tapping through to it.
#[component]
fn Block(#[prop(into)] title: String, screen: Screen, children: Children) -> impl IntoView {
    let runtime = use_runtime();

    view! {
        <div class="hi">
            <div
                class="hi-head"
                on:click=move |_| runtime.ui(EventUi::ScreenOpened { screen })
            >
                <span class="hi-title">{title}</span>
            </div>
            {children()}
        </div>
    }
}
