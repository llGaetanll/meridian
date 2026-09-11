use client_state::EventUi;
use client_state::Screen;
use client_state::ViewAccount;
use client_state::ViewBalances;
use client_state::ViewLedger;
use client_state::ViewOnboarding;
use client_state::ViewPaywall;
use client_state::ViewRecurring;
use client_state::ViewScreen;
use leptos::prelude::*;
use web_components::SearchRow;
use web_components::Shell;

use crate::frame::Frame;
use crate::pages::AccountPage;
use crate::pages::BalancesPage;
use crate::pages::LedgerPage;
use crate::pages::MarketsPage;
use crate::pages::OnboardingPage;
use crate::pages::PaywallPage;
use crate::pages::RecurringPage;
use crate::runtime_app::RuntimeApp;
use crate::state::provide_markets;
use crate::tab::Tab;

/// The whole app: the last screen the machine rendered, matched on.
///
/// There is no router and no page state. First run and the paywall draw on
/// their own; the four tabs draw inside the shell, which is mounted once and
/// stays while they change under it.
#[component]
pub fn App() -> impl IntoView {
    provide_markets();

    let runtime = RuntimeApp::new();

    provide_context(runtime);

    runtime.start();

    let screen = runtime.screen();
    let frame = Memo::new(move |_| screen.with(|screen| screen.as_ref().map(Frame::of)));

    move || match frame.get() {
        // Nothing rendered yet. The machine answers within a turn of the
        // session being read, which is too soon to be worth drawing for.
        None => ().into_any(),

        Some(Frame::Onboarding) => follow(screen, onboarding)
            .map(|view| view! { <OnboardingPage view=view /> })
            .into_any(),

        Some(Frame::Paywall) => follow(screen, paywall)
            .map(|view| view! { <PaywallPage view=view /> })
            .into_any(),

        Some(Frame::Shell) => view! { <AppShell /> }.into_any(),
    }
}

/// The sidebar, and whichever tab is showing beside it.
#[component]
fn AppShell() -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();
    let screen = runtime.screen();

    // Per visit to the shell, so the paywall or signing out puts it back.
    let markets = RwSignal::new(false);

    let tab = Memo::new(move |_| {
        screen.with(|screen| {
            screen
                .as_ref()
                .and_then(|screen| Tab::of(screen, markets.get()))
        })
    });

    let open = Callback::new(move |screen: Screen| {
        markets.set(false);

        runtime.ui(EventUi::ScreenOpened { screen });
    });

    let retry = Callback::new(move |()| runtime.ui(EventUi::RefreshRequested));

    view! {
        <Shell
            dock=runtime.dock()
            markets=markets
            error=runtime.error()
            on_screen=open
            on_markets=Callback::new(move |()| markets.set(true))
            on_retry=retry
        >
            {move || match tab.get() {
                None => ().into_any(),

                // Outside `.ui`: it was drawn before the design language, and
                // the language's `.btn`, `.chip` and `.field` would restyle it.
                Some(Tab::Markets) => view! { <MarketsPage /> }.into_any(),

                Some(Tab::Ledger) => {
                    follow(screen, ledger)
                        .map(|view| {
                            view! {
                                <div class="ui">
                                    <SearchRow />
                                    <LedgerPage view=view />
                                </div>
                            }
                        })
                        .into_any()
                }
                Some(Tab::Recurring) => {
                    follow(screen, recurring)
                        .map(|view| {
                            view! {
                                <div class="ui">
                                    <SearchRow />
                                    <RecurringPage view=view />
                                </div>
                            }
                        })
                        .into_any()
                }
                Some(Tab::Balances) => {
                    follow(screen, balances)
                        .map(|view| {
                            view! {
                                <div class="ui">
                                    <SearchRow />
                                    <BalancesPage view=view />
                                </div>
                            }
                        })
                        .into_any()
                }
                Some(Tab::Account) => {
                    follow(screen, account)
                        .map(|view| {
                            view! {
                                <div class="ui">
                                    <SearchRow />
                                    <AccountPage view=view />
                                </div>
                            }
                        })
                        .into_any()
                }
            }}
        </Shell>
    }
}

/// One screen's view, following every render of it, for the page showing it.
///
/// `None` if that screen is not the one showing. Once a page is up it keeps
/// the last view it was given while the app moves elsewhere, so that it has
/// something to hold in the moment before it comes down.
fn follow<T>(
    screen: ReadSignal<Option<ViewScreen>>,
    pick: fn(&ViewScreen) -> Option<&T>,
) -> Option<Signal<T>>
where
    T: Clone + Send + Sync + 'static,
{
    let first = screen.with_untracked(|screen| screen.as_ref().and_then(pick).cloned())?;

    let view = Memo::new_owning(move |last: Option<T>| {
        match screen.with(|screen| screen.as_ref().and_then(pick).cloned()) {
            Some(view) => (view, true),
            None => match last {
                Some(last) => (last, false),
                None => (first.clone(), true),
            },
        }
    });

    Some(view.into())
}

fn ledger(screen: &ViewScreen) -> Option<&ViewLedger> {
    match screen {
        ViewScreen::Ledger { view } => Some(view),
        _ => None,
    }
}

fn recurring(screen: &ViewScreen) -> Option<&ViewRecurring> {
    match screen {
        ViewScreen::Recurring { view } => Some(view),
        _ => None,
    }
}

fn balances(screen: &ViewScreen) -> Option<&ViewBalances> {
    match screen {
        ViewScreen::Balances { view } => Some(view),
        _ => None,
    }
}

fn account(screen: &ViewScreen) -> Option<&ViewAccount> {
    match screen {
        ViewScreen::Account { view } => Some(view),
        _ => None,
    }
}

fn onboarding(screen: &ViewScreen) -> Option<&ViewOnboarding> {
    match screen {
        ViewScreen::Onboarding { view } => Some(view),
        _ => None,
    }
}

fn paywall(screen: &ViewScreen) -> Option<&ViewPaywall> {
    match screen {
        ViewScreen::Paywall { view } => Some(view),
        _ => None,
    }
}
