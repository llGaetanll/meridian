use client_state::Screen;
use client_state::ViewDock;
use client_state::ViewDockItem;
use leptos::prelude::*;
use web_ui::Glyph;
use web_ui::Icon;
use web_ui::IconKind;

/// The globe mark, sized by `.brand-mark`.
const BRAND_MARK: &str = r##"<defs><clipPath id="meridian-globe-clip"><circle cx="100" cy="100" r="93"/></clipPath></defs><g clip-path="url(#meridian-globe-clip)"><g stroke-width="14"><path d="M 181.753,73.308 C 171.596,42.2,126.76,28.932,81.61,43.674 C 36.459,58.415,8.09,95.584,18.247,126.692"/><path d="M 18.247,126.692 C 8.09,95.584,36.459,58.415,81.61,43.674 C 126.76,28.932,171.596,42.2,181.753,73.308"/><path d="M 118.172,15.942 C 108.574,13.867,92.658,49.819,82.622,96.243 C 72.586,142.667,72.23,181.983,81.828,184.058"/><path d="M 81.828,184.058 C 72.23,181.983,72.586,142.667,82.622,96.243 C 92.658,49.819,108.574,13.867,118.172,15.942"/><path d="M 14,100 C 14,133.585,52.504,160.811,100,160.811 C 147.496,160.811,186,133.585,186,100"/><path d="M 100,14 C 66.415,14,39.189,33.252,39.189,57 C 39.189,80.748,66.415,100,100,100 C 133.585,100,160.811,80.748,160.811,57 C 160.811,33.252,133.585,14,100,14"/></g><g stroke-width="14"><path d="M 147.342,171.797 C 166.464,159.188,160.77,116.822,134.624,77.17 C 108.477,37.517,71.78,15.595,52.658,28.203"/><path d="M 52.658,28.203 C 71.78,15.595,108.477,37.517,134.624,77.17 C 160.77,116.822,166.464,159.188,147.342,171.797"/></g></g><circle cx="100" cy="100" r="86" stroke-width="14"/>"##;

const AVATAR: &str = r##"<defs><linearGradient id="av" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="var(--violet-400)"/><stop offset="100%" stop-color="var(--violet-700)"/></linearGradient></defs><rect width="40" height="40" fill="url(#av)"/><circle cx="20" cy="14.8" r="7.2" fill="var(--bg)" opacity="0.34"/><path d="M4.6 40c0-8.6 6.9-13.6 15.4-13.6S35.4 31.4 35.4 40z" fill="var(--bg)" opacity="0.34"/>"##;

/// Sidebar plus the one scroller beside it. Pages render inside `.content`.
///
/// The machine's tabs come from its dock, which also says which is lit.
/// Markets is not one of them — it is the web's own page — so it is lit from
/// `markets` instead, and while it is, no tab of the machine's is.
#[component]
pub fn Shell(
    #[prop(into)] dock: Signal<Option<ViewDock>>,
    #[prop(into)] markets: Signal<bool>,
    #[prop(into)] error: Signal<Option<String>>,
    on_screen: Callback<Screen>,
    on_retry: Callback<()>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="shell">
            <Sidebar dock=dock markets=markets on_screen=on_screen />
            <div class="main">
                <div class="content">
                    {move || {
                        error.get().map(|message| view! { <Banner message=message on_retry=on_retry /> })
                    }}
                    {children()}
                </div>
            </div>
        </div>
    }
}

/// The globe and the name. Head of the sidebar, and the one thing on a screen
/// drawn without one that says whose app this is.
#[component]
pub fn Brand() -> impl IntoView {
    view! {
        <div class="brand">
            <svg
                class="brand-mark"
                viewBox="0 0 200 200"
                fill="none"
                stroke="currentColor"
                stroke-linecap="round"
                stroke-linejoin="round"
                aria-hidden="true"
                inner_html=BRAND_MARK
            />
            <span>Meridian</span>
        </div>
    }
}

#[component]
fn Sidebar(
    dock: Signal<Option<ViewDock>>,
    markets: Signal<bool>,
    on_screen: Callback<Screen>,
) -> impl IntoView {
    // The dock's Account is the block at the foot rather than a row here.
    let tabs = move || {
        dock.with(|dock| {
            dock.iter()
                .flat_map(|dock| &dock.items)
                .filter(|item| item.screen != Screen::Account)
                .cloned()
                .collect::<Vec<_>>()
        })
    };

    // Which row the pill stands on. The tabs draw in dock order, so the lit
    // one's place among them is where it sits. On Markets none of the
    // machine's tabs is lit and the pill goes.
    let pill = Memo::new(move |_| match markets.get() {
        true => None,
        false => dock.with(|dock| {
            dock.iter()
                .flat_map(|dock| &dock.items)
                .filter(|item| item.screen != Screen::Account)
                .position(|item| item.selected)
        }),
    });

    // Which screen is lit, read where a row is drawn rather than frozen into
    // it, so a row lives across the change and its lettering can cross.
    let here = Memo::new(move |_| {
        dock.with(|dock| {
            dock.iter()
                .flat_map(|dock| &dock.items)
                .find(|item| item.selected)
                .map(|item| item.screen)
        })
    });

    let account = Memo::new(move |_| {
        dock.with(|dock| {
            dock.iter()
                .flat_map(|dock| &dock.items)
                .find(|item| item.screen == Screen::Account)
                .map(|item| item.label.clone())
        })
    });

    view! {
        <aside class="sidebar">
            <Brand />

            <nav class="nav" style=move || pill.get().map(|row| format!("--lit:{row}"))>
                <span class="nav-pill" class:off=move || pill.get().is_none() />

                <For
                    each=tabs
                    key=|item| item.label.clone()
                    children=move |item: ViewDockItem| {
                        let screen = item.screen;

                        view! {
                            <a
                                class:on=move || here.get() == Some(screen) && !markets.get()
                                href="#"
                                on:click=move |ev| {
                                    ev.prevent_default();
                                    on_screen.run(screen);
                                }
                            >
                                <Glyph name=item.icon class="icon" />
                                {item.label}
                            </a>
                        }
                    }
                />

                // Drawn and not built, the same state a chip has. A nav that
                // pretended the app was four pages would have to grow later
                // and move everything under it.
                <a class="dead" aria-disabled="true">
                    <Icon kind=IconKind::Portfolio class="icon" />
                    Portfolios
                </a>

                // Locked: the page is built but runs on stand-in data, and the
                // markets side of the app is a later pass.
                <a class="dead" aria-disabled="true">
                    <Icon kind=IconKind::Markets class="icon" />
                    Markets
                </a>
            </nav>

            <div class="sidebar-foot">
                // Everything a setting could configure lives on the screen it
                // affects, and the rest is Account.
                <nav class="nav">
                    <a class="dead" aria-disabled="true">
                        <Icon kind=IconKind::Settings class="icon" />
                        Settings
                    </a>
                </nav>

                {move || {
                    account
                        .get()
                        .map(|label| {
                            view! {
                                <div class="account" on:click=move |_| on_screen.run(Screen::Account)>
                                    <div class="avatar">
                                        <svg viewBox="0 0 40 40" aria-hidden="true" inner_html=AVATAR />
                                    </div>
                                    <div class="account-id">
                                        <div class="account-name">{label}</div>
                                    </div>
                                    <span class="more">
                                        <Icon kind=IconKind::ChevronUpDown />
                                    </span>
                                </div>
                            }
                        })
                }}
            </div>
        </aside>
    }
}

/// The search box at the head of a page, drawn and dead, as the dock's search
/// button is on the phone: nothing has drawn the screen behind it.
#[component]
pub fn SearchRow() -> impl IntoView {
    view! {
        <div class="searchrow">
            <div class="search dead" aria-disabled="true">
                <Icon kind=IconKind::Search />
                <span class="placeholder">"Search merchants, accounts, streams…"</span>
                <span class="kbd">"⌘K"</span>
            </div>
        </div>
    }
}

/// What went wrong, over whatever was loaded before — which is still worth
/// looking at, so the failure sits above it instead of replacing it.
#[component]
fn Banner(message: String, on_retry: Callback<()>) -> impl IntoView {
    view! {
        <div class="ui">
            <div class="banner">
                <span class="say">{message}</span>
                <button class="btn" on:click=move |_| on_retry.run(())>
                    Retry
                </button>
            </div>
        </div>
    }
}
