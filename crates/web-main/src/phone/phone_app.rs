use client_state::EventUi;
use client_state::Screen;
use leptos::prelude::*;
use web_components::BalancesBand;
use web_components::LedgerBand;
use web_components::PhoneShell;
use web_components::RecurringBand;

use super::AccountPhone;
use super::BalancesPhone;
use super::LedgerPhone;
use super::RecurringPhone;
use crate::app::account;
use crate::app::balances;
use crate::app::follow;
use crate::app::ledger;
use crate::app::recurring;
use crate::runtime_app::RuntimeApp;
use crate::tab::Tab;
use crate::tab::follow_title;

/// The phone's frame, and whichever tab is showing in it: what `AppShell` is
/// on a wide window.
///
/// Markets is not among the tabs. It is the web's own page and not one of the
/// machine's screens, and the phone has nowhere to put it.
#[component]
pub fn PhoneApp() -> impl IntoView {
    let runtime = expect_context::<RuntimeApp>();
    let screen = runtime.screen();
    let dock = runtime.dock();

    let tab = Memo::new(move |_| {
        screen.with(|screen| screen.as_ref().and_then(|screen| Tab::of(screen, false)))
    });

    follow_title(tab.into());

    // The lit tab's name, in the dock's words.
    let title = Signal::derive(move || {
        dock.with(|dock| {
            dock.iter()
                .flat_map(|dock| &dock.items)
                .find(|item| item.selected)
                .map(|item| item.label.clone())
                .unwrap_or_default()
        })
    });

    let open = Callback::new(move |screen: Screen| runtime.ui(EventUi::ScreenOpened { screen }));
    let refresh = Callback::new(move |()| runtime.ui(EventUi::RefreshRequested));

    // What each page's headline becomes once it has scrolled by. The
    // Ledger's is what its filter is set to, and takes you back to it.
    let band = Callback::new(move |back: Callback<()>| match tab.get() {
        Some(Tab::Ledger) => follow(screen, ledger)
            .map(|view| {
                view! {
                    <LedgerBand
                        filter=Signal::derive(move || view.with(|view| view.filter.clone()))
                        on_open=back
                    />
                }
            })
            .into_any(),

        Some(Tab::Recurring) => follow(screen, recurring)
            .map(|view| {
                view! {
                    <RecurringBand
                        headline=text(view, |view| &view.headline)
                        flow_in=text(view, |view| &view.flow_in_signed)
                        flow_out=text(view, |view| &view.flow_out_signed)
                    />
                }
            })
            .into_any(),

        Some(Tab::Balances) => follow(screen, balances)
            .map(|view| {
                view! {
                    <BalancesBand
                        label=text(view, |view| &view.headline_label)
                        headline=text(view, |view| &view.headline)
                    />
                }
            })
            .into_any(),

        // No headline to stand in for, so nothing on Account is an anchor and
        // the band never comes.
        Some(Tab::Account | Tab::Markets) | None => ().into_any(),
    });

    view! {
        <PhoneShell
            title=title
            dock=dock
            error=runtime.error()
            loading=runtime.loading()
            on_screen=open
            on_refresh=refresh
            band=band
        >
            {move || match tab.get() {
                Some(Tab::Ledger) => {
                    follow(screen, ledger).map(|view| view! { <LedgerPhone view=view /> }).into_any()
                }
                Some(Tab::Recurring) => {
                    follow(screen, recurring)
                        .map(|view| view! { <RecurringPhone view=view /> })
                        .into_any()
                }
                Some(Tab::Balances) => {
                    follow(screen, balances)
                        .map(|view| view! { <BalancesPhone view=view /> })
                        .into_any()
                }
                Some(Tab::Account) => {
                    follow(screen, account).map(|view| view! { <AccountPhone view=view /> }).into_any()
                }
                Some(Tab::Markets) | None => ().into_any(),
            }}
        </PhoneShell>
    }
}

fn text<T>(view: Signal<T>, read: fn(&T) -> &String) -> Signal<String>
where
    T: Send + Sync + 'static,
{
    Signal::derive(move || view.with(|view| read(view).clone()))
}
