use client_state::EventUi;
use client_state::Screen;
use client_state::ViewDock;
use client_state::ViewDockItem;
use client_state::ViewWho;
use leptos::prelude::*;
use leptos::wasm_bindgen::JsCast;

use crate::theme;
use web_ui::Glyph;
use web_ui::Icon;
use web_ui::IconKind;

/// The globe mark, sized by `.brand-mark`.
const BRAND_MARK: &str = r##"<defs><clipPath id="meridian-globe-clip"><circle cx="100" cy="100" r="93"/></clipPath></defs><g clip-path="url(#meridian-globe-clip)"><g stroke-width="14"><path d="M 181.753,73.308 C 171.596,42.2,126.76,28.932,81.61,43.674 C 36.459,58.415,8.09,95.584,18.247,126.692"/><path d="M 18.247,126.692 C 8.09,95.584,36.459,58.415,81.61,43.674 C 126.76,28.932,171.596,42.2,181.753,73.308"/><path d="M 118.172,15.942 C 108.574,13.867,92.658,49.819,82.622,96.243 C 72.586,142.667,72.23,181.983,81.828,184.058"/><path d="M 81.828,184.058 C 72.23,181.983,72.586,142.667,82.622,96.243 C 92.658,49.819,108.574,13.867,118.172,15.942"/><path d="M 14,100 C 14,133.585,52.504,160.811,100,160.811 C 147.496,160.811,186,133.585,186,100"/><path d="M 100,14 C 66.415,14,39.189,33.252,39.189,57 C 39.189,80.748,66.415,100,100,100 C 133.585,100,160.811,80.748,160.811,57 C 160.811,33.252,133.585,14,100,14"/></g><g stroke-width="14"><path d="M 147.342,171.797 C 166.464,159.188,160.77,116.822,134.624,77.17 C 108.477,37.517,71.78,15.595,52.658,28.203"/><path d="M 52.658,28.203 C 71.78,15.595,108.477,37.517,134.624,77.17 C 160.77,116.822,166.464,159.188,147.342,171.797"/></g></g><circle cx="100" cy="100" r="86" stroke-width="14"/>"##;

/// The face nobody has given us one for.
///
/// A quiet one, in the theme's own greys, rather than the violet it used to
/// be: it stands beside a name and an address and is the least important of
/// the three. A Google sign-in will hand over a real one and this goes.
const AVATAR: &str = r##"<rect width="40" height="40" fill="var(--bg-input)"/><circle cx="20" cy="15.6" r="6.4" fill="var(--text-faint)"/><path d="M6.4 40c0-7.7 6.1-12.2 13.6-12.2S33.6 32.3 33.6 40z" fill="var(--text-faint)"/>"##;

/// Sidebar plus the one scroller beside it. Pages render inside `.content`.
///
/// The machine's tabs come from its dock, which also says which is lit.
/// Markets is not one of them — it is the web's own page — so it is lit from
/// `markets` instead, and while it is, no tab of the machine's is.
#[component]
pub fn Shell(
    #[prop(into)] dock: Signal<Option<ViewDock>>,
    #[prop(into)] who: Signal<Option<ViewWho>>,
    #[prop(into)] markets: Signal<bool>,
    #[prop(into)] leaving: Signal<bool>,
    #[prop(into)] error: Signal<Option<String>>,
    on_screen: Callback<Screen>,
    on_markets: Callback<()>,
    on_retry: Callback<()>,
    ui: Callback<EventUi>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="shell">
            <Sidebar
                dock=dock
                who=who
                markets=markets
                on_screen=on_screen
                on_markets=on_markets
                ui=ui
            />
            <div class="main">
                <div class="content">
                    <SearchRow />

                    {move || {
                        error.get().map(|message| view! { <Banner message=message on_retry=on_retry /> })
                    }}

                    <div class="showing" class:leaving=move || leaving.get()>
                        {children()}
                    </div>
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
    who: Signal<Option<ViewWho>>,
    markets: Signal<bool>,
    on_screen: Callback<Screen>,
    on_markets: Callback<()>,
    ui: Callback<EventUi>,
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

                // The web's own page, lit from `markets` rather than the
                // dock: it is not one of the machine's screens.
                <a
                    class:on=move || markets.get()
                    href="#"
                    on:click=move |ev| {
                        ev.prevent_default();
                        on_markets.run(());
                    }
                >
                    <Icon kind=IconKind::Markets class="icon" />
                    Markets
                </a>
            </nav>

            <div class="sidebar-foot">
                {move || {
                    who.get()
                        .map(|who| view! { <Account who=who on_screen=on_screen ui=ui /> })
                }}
            </div>

        </aside>
    }
}

/// Where the lit rectangle stands, and what colour it is.
///
/// Measured from the card's *bottom* edge rather than its top, because that is
/// the edge that holds still: the card grows upward out of the sidebar's foot,
/// so a row's distance from the top changes as it opens and its distance from
/// the bottom does not. Anchoring the other way would put the rectangle under
/// the chip at the wrong height for as long as the card was still growing.
#[derive(Clone, Copy, PartialEq)]
struct Lit {
    /// Pixels from the row's bottom edge to the card's.
    lift: f64,

    height: f64,

    /// The row that cannot be undone, which lights rose rather than grey.
    grave: bool,
}

/// Who is signed in, at the foot of the sidebar, and what can be done about
/// it.
///
/// Pressing the chip does not change page. It grows the card the chip already
/// sits in — the card is there in both states, and opening it is five pixels
/// of ground appearing inside while the same five come off the outside, so
/// everything printed on the chip stays exactly where it was. The geometry and
/// the timing are both in `css/app.css`; what is here is only which state it
/// is in and which row the pointer is on.
///
/// The card closes on a press anywhere else, which is a listener on the window
/// rather than a sheet of nothing over the page: a page you cannot scroll
/// because a menu is open is a page that feels broken.
#[component]
fn Account(who: ViewWho, on_screen: Callback<Screen>, ui: Callback<EventUi>) -> impl IntoView {
    let open = RwSignal::new(false);

    // Which theme is on, as a word for the row to carry. Read as the sidebar
    // is built and moved by the row itself, because nothing else in the app
    // changes it.
    let theme = RwSignal::new(theme_label(theme::is_light()));

    // Where the lit rectangle stands. Measured rather than counted, unlike the
    // nav's, because these rows are not one height: four of them are a line of
    // text and the last is a face.
    let lit = RwSignal::new(None::<Lit>);

    // Whether it may travel to get there. Off for the frame it appears in, so
    // it arrives under the pointer instead of sliding in from wherever it last
    // stood — which is the whole difference between a rectangle that follows
    // the mouse and one that chases it.
    let travel = RwSignal::new(false);

    let stand = move |row: &web_sys::HtmlElement, grave: bool| {
        let Some(card) = row.closest(".account-open").ok().flatten() else {
            return;
        };

        let row = row.get_bounding_client_rect();
        let card = card.get_bounding_client_rect();

        let arriving = lit.with_untracked(Option::is_none);

        lit.set(Some(Lit {
            lift: card.bottom() - row.bottom(),
            height: row.height(),
            grave,
        }));

        if arriving {
            travel.set(false);

            // A frame later, once it has been painted where it landed. Set in
            // the same breath and the browser would have nothing to move from.
            request_animation_frame(move || travel.set(true));
        }
    };

    let on_row = move |grave: bool| {
        move |event: leptos::ev::PointerEvent| {
            let row = event
                .current_target()
                .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok());

            if let Some(row) = row {
                stand(&row, grave);
            }
        }
    };

    // Pressing the chip opens the card with the pointer already on the chip,
    // and a pointer that is already somewhere sends no event saying so. So the
    // press lights the chip itself, rather than waiting for a `pointerenter`
    // that will not come until the pointer moves off it and back.
    let press = Callback::new(move |chip: web_sys::HtmlElement| {
        let opening = !open.get_untracked();

        open.set(opening);

        match opening {
            true => stand(&chip, false),

            // Shut, nothing is a row and there is nothing to light.
            false => lit.set(None),
        }
    });

    // A press anywhere else closes it. A listener rather than a sheet of
    // nothing over the page: a page you cannot scroll because a menu is open
    // is a page that feels broken.
    let close = window_event_listener(leptos::ev::click, move |_| open.set(false));
    on_cleanup(move || close.remove());

    let settings = who.settings_label.clone();
    let sign_out = who.sign_out_label.clone();
    let delete = who.delete_label.clone();

    view! {
        <div class="ui">
            <div
                class="menu account-open"
                class:open=move || open.get()
                on:click=|event| event.stop_propagation()
                on:pointerleave=move |_| lit.set(None)
            >
                <span
                    class="menu-pill"
                    class:off=move || lit.get().is_none()
                    class:still=move || !travel.get()
                    class:grave=move || lit.get().is_some_and(|lit| lit.grave)
                    style=move || match lit.get() {
                        Some(lit) => format!("--b:{}px;--h:{}px", lit.lift, lit.height),
                        None => String::new(),
                    }
                />

                // The rows, in a grid that goes from no rows to one: the only
                // way a height that is whatever the content is can be a height
                // that animates.
                <div class="options">
                    <div class="in">
                        <button class="item" on:pointerenter=on_row(false)
                            on:click=move |_| {
                                open.set(false);
                                on_screen.run(Screen::Account);
                            }
                        >
                            <Icon kind=IconKind::Settings />
                            {settings}
                        </button>

                        <button class="item" on:pointerenter=on_row(false)
                            on:click=move |_| theme.set(theme_label(theme::toggle() == "light"))
                        >
                            <Icon kind=IconKind::Theme />
                            "Theme"
                            <span class="val">{move || theme.get()}</span>
                        </button>

                        <div class="cut" />

                        <button class="item" on:pointerenter=on_row(false)
                            on:click=move |_| {
                                open.set(false);
                                ui.run(EventUi::SignOutRequested);
                            }
                        >
                            <Icon kind=IconKind::SignOut />
                            {sign_out}
                        </button>

                        // Opens the confirmation, which lives on the Account
                        // screen: there is no undo, and a popup over the
                        // ledger is not where that is asked.
                        <button class="item grave" on:pointerenter=on_row(true)
                            on:click=move |_| {
                                open.set(false);
                                on_screen.run(Screen::Account);
                                ui.run(EventUi::DeleteAccountRequested);
                            }
                        >
                            <Icon kind=IconKind::Trash />
                            {delete}
                        </button>

                        <div class="cut" />
                    </div>
                </div>

                // The chip lights only while the card is open, where it is a
                // row among rows. Shut, it is the thing you press to open the
                // card and nothing more — a rectangle appearing under it would
                // be the card answering a pointer it has not been given.
                <Chip who=who on_press=press lit=Signal::derive(move || open.get()) on_over=Box::new(on_row(false)) />
            </div>
        </div>
    }
}

/// The chip itself: a face, a name, and the address under it.
///
/// One component because it is drawn twice — on its own at the foot of the
/// sidebar, and again inside the card it opens, where the only difference is
/// the padding `.account-open .account` takes off it.
#[component]
fn Chip(
    who: ViewWho,
    on_press: Callback<web_sys::HtmlElement>,
    /// Whether this chip is one of the rows the card's rectangle visits, which
    /// it is only while the card is open.
    #[prop(into, optional)]
    lit: Signal<bool>,
    #[prop(optional)] on_over: Option<Box<dyn Fn(leptos::ev::PointerEvent)>>,
) -> impl IntoView {
    view! {
        <div
            class="account"
            on:pointerenter=move |event| {
                if lit.get_untracked()
                    && let Some(on_over) = &on_over
                {
                    on_over(event);
                }
            }
            on:click=move |event| {
                // Kept off the window listener that closes the card, which
                // would otherwise close it in the press that opened it.
                event.stop_propagation();

                let chip = event
                    .current_target()
                    .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok());

                if let Some(chip) = chip {
                    on_press.run(chip);
                }
            }
        >
            <div class="avatar">
                <svg viewBox="0 0 40 40" aria-hidden="true" inner_html=AVATAR />
            </div>
            <div class="account-id">
                <div class="account-name">{who.title}</div>
                <div class="account-mail">{who.subtitle}</div>
            </div>
            <span class="more">
                <Icon kind=IconKind::ChevronUpDown />
            </span>
        </div>
    }
}

/// The word the theme row carries. Here rather than in `client-state` for the
/// same reason the toggle is: nothing in the machine knows a colour.
fn theme_label(light: bool) -> &'static str {
    match light {
        true => "Light",
        false => "Dark",
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
