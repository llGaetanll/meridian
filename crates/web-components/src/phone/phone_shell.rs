use client_state::Screen;
use client_state::ViewDock;
use leptos::ev::TouchEvent;
use leptos::html;
use leptos::prelude::*;
use leptos::web_sys::Element;
use leptos::web_sys::ScrollBehavior;
use leptos::web_sys::ScrollToOptions;
use web_ui::Glyph;

use super::Dock;

/// The band arrives as the last of its anchor goes under the top of the body:
/// a headline still half on screen does not need standing in for.
const ARRIVES: f64 = 24.0;

/// And leaves later than it arrived, so the two cannot chase each other — but
/// short of the anchor's own height, which is where an anchor at rest sits.
const LEAVES: f64 = 48.0;

/// How far the list has to be pulled down past its top, once damped, for
/// letting go to ask for it again.
const PULL: f64 = 56.0;

/// The spinner comes down half as far as the finger, so the pull resists the
/// way the list does.
const DAMPING: f64 = 0.5;

/// The phone's frame: the page's title, a scrolling body, and the dock floating
/// over it. `swift/Sources/MeridianApp/Ui/Shell.swift` is what it draws.
///
/// Mounted once while the tabs change under it, so a change of tab moves a
/// title and a body and leaves the frame where it is. A page marks what its
/// band stands in for with `data-band-anchor`; once that has scrolled by,
/// `band` floats over the top of the body, handed the way back to it.
///
/// Pulling the body down past its top asks for everything again, as Retry on
/// the banner does, and the spinner stays for as long as `loading` says the
/// answer is on its way.
#[component]
pub fn PhoneShell(
    #[prop(into)] title: Signal<String>,
    #[prop(into)] dock: Signal<Option<ViewDock>>,
    #[prop(into)] error: Signal<Option<String>>,
    #[prop(into)] loading: Signal<bool>,
    on_screen: Callback<Screen>,
    on_refresh: Callback<()>,
    band: Callback<Callback<()>, AnyView>,
    children: Children,
) -> impl IntoView {
    let body = NodeRef::<html::Div>::new();
    let banded = RwSignal::new(false);

    let anchor = move || -> Option<Element> {
        body.get_untracked()?
            .query_selector("[data-band-anchor]")
            .ok()
            .flatten()
    };

    // Where the anchor's foot is, down from the top of the body. No anchor
    // reads as far below, which is what a page with no band is.
    let scrolled = move || {
        let Some(body) = body.get_untracked() else {
            return;
        };

        let foot = anchor().map_or(f64::INFINITY, |anchor| {
            anchor.get_bounding_client_rect().bottom() - body.get_bounding_client_rect().top()
        });

        let shown = banded.get_untracked();
        let wanted = if shown { foot < LEAVES } else { foot < ARRIVES };

        if wanted != shown {
            banded.set(wanted);
        }
    };

    // A new tab is a new page, read from the top.
    Effect::new(move |last: Option<String>| {
        let shown = title.get();

        if last.is_some_and(|last| last != shown) {
            if let Some(body) = body.get_untracked() {
                body.set_scroll_top(0);
            }

            banded.set(false);
        }

        shown
    });

    let back = Callback::new(move |()| {
        let (Some(body), Some(anchor)) = (body.get_untracked(), anchor()) else {
            return;
        };

        let top = anchor.get_bounding_client_rect().top() - body.get_bounding_client_rect().top()
            + f64::from(body.scroll_top());

        let options = ScrollToOptions::new();
        options.set_top(top);
        options.set_behavior(ScrollBehavior::Smooth);

        body.scroll_to_with_scroll_to_options(&options);
    });

    // Where a pull began, if it began at the top; how far it has come; and
    // whether one let go far enough and is waiting on its answer.
    let start = StoredValue::new(None::<f64>);
    let pulled = RwSignal::new(0.0_f64);
    let refreshing = RwSignal::new(false);

    Effect::new(move |_| {
        if !loading.get() {
            refreshing.set(false);
        }
    });

    let touched = move |event: TouchEvent| {
        let top = body
            .get_untracked()
            .is_some_and(|body| body.scroll_top() <= 0);

        start.set_value(if top { first_y(&event) } else { None });
    };

    let dragged = move |event: TouchEvent| {
        let (Some(from), Some(y)) = (start.get_value(), first_y(&event)) else {
            return;
        };

        let now = ((y - from) * DAMPING).max(0.0);

        if now > 0.0 || pulled.get_untracked() > 0.0 {
            pulled.set(now);
        }
    };

    let released = move |_: TouchEvent| {
        if start.get_value().is_none() {
            return;
        }

        start.set_value(None);

        let far = pulled.get_untracked() >= PULL;

        pulled.set(0.0);

        if far {
            on_refresh.run(());

            // Only if a load went out: one the machine chose not to make has
            // nothing to wait for.
            refreshing.set(loading.get_untracked());
        }
    };

    let pull = move || {
        let shown = pulled.get().min(PULL * 1.5);

        format!(
            "--pulled: {shown:.1}px; --reach: {:.3}",
            (shown / PULL).min(1.0)
        )
    };

    view! {
        <div class="phone-shell ui">
            <div class="page-title">
                // Drawn again when it changes, so the new title fades in
                // rather than being swapped under the eye.
                {move || {
                    let title = title.get();

                    view! { <span>{title}</span> }
                }}
            </div>

            // Over whatever was loaded before, which is still worth looking at.
            {move || {
                error
                    .get()
                    .map(|message| {
                        view! {
                            <div class="phone-banner">
                                <div class="banner">
                                    <span class="say">{message}</span>
                                    <button class="btn" on:click=move |_| on_refresh.run(())>
                                        "Retry"
                                    </button>
                                </div>
                            </div>
                        }
                    })
            }}

            <div class="phone-body" class:refreshing=move || refreshing.get()>
                <div
                    class="phone-scroll"
                    node_ref=body
                    on:scroll=move |_| scrolled()
                    on:touchstart=touched
                    on:touchmove=dragged
                    on:touchend=released
                    on:touchcancel=released
                >
                    {children()}
                </div>

                <div class="pull" class:spinning=move || refreshing.get() style=pull>
                    <Glyph name="spin" />
                </div>

                // Over the list rather than above it: inset, the list would
                // move by the band's height the moment it arrived.
                <div class="band-slot" class:on=move || banded.get() inert=move || !banded.get()>
                    {move || band.run(back)}
                </div>
            </div>

            <Dock dock=dock on_screen=on_screen />
        </div>
    }
}

fn first_y(event: &TouchEvent) -> Option<f64> {
    event
        .touches()
        .get(0)
        .map(|touch| f64::from(touch.client_y()))
}
