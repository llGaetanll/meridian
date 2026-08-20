//! The phone's frame: a status bar, a header, a scrolling body, and the dock
//! floating over it.
//!
//! Deliberately not `web_components::Shell` — that one is a desktop sidebar.

use leptos::prelude::*;
use phone_state::EventUi;
use phone_state::ViewDock;

use crate::runtime_app::use_runtime;
use crate::ui::BrandMark;
use crate::ui::Icon;

/// The frame every screen sits in.
#[component]
pub fn PhoneShell(
    /// The big title. `None` on Home, which shows the mark instead.
    #[prop(optional, into)]
    title: Option<String>,
    #[prop(optional, into)] subtitle: Option<String>,
    /// Drawn between the header and the body — the filter panel, on the one
    /// screen that has one.
    #[prop(optional, into)]
    panel: Option<ViewFn>,
    children: Children,
) -> impl IntoView {
    let runtime = use_runtime();
    let error = runtime.error();
    let loading = runtime.loading();

    view! {
        <div class="phone">
            <div class="statusbar">
                <span class="num">"9:41"</span>
                <span class="glyphs">
                    <StatusGlyphs />
                </span>
            </div>

            <header class="header">
                {match title {
                    None => {
                        view! {
                            <div class="brandline">
                                <BrandMark />
                                <span class="brand-name">"Meridian"</span>
                            </div>
                        }
                            .into_any()
                    }
                    Some(title) => {
                        view! {
                            <div>
                                <div class="page-title">{title}</div>
                                {subtitle.map(|text| view! { <div class="page-sub">{text}</div> })}
                            </div>
                        }
                            .into_any()
                    }
                }}

                <button
                    class="refresh"
                    class:spinning=move || loading.get()
                    aria-label="Refresh"
                    on:click=move |_| runtime.ui(EventUi::RefreshRequested)
                >
                    <Icon name="refresh" />
                </button>
            </header>

            // A banner rather than a screen: whatever was loaded before is
            // still worth looking at, so the failure sits above it instead of
            // replacing it.
            {move || {
                error
                    .get()
                    .map(|message| {
                        view! {
                            <div class="banner">
                                <span>{message}</span>
                                <button on:click=move |_| runtime.ui(EventUi::RefreshRequested)>
                                    "Retry"
                                </button>
                            </div>
                        }
                    })
            }}

            {panel.map(|panel| panel.run())}

            <main class="body">{children()}</main>

            <Dock />
        </div>
    }
}

/// The dock. Which tab is lit is the core's answer, not a comparison here.
#[component]
fn Dock() -> impl IntoView {
    let runtime = use_runtime();
    let dock = runtime.dock();

    view! {
        <div class="dockrow">
            <nav class="dock glass">
                {move || {
                    dock.get()
                        .map(|dock: ViewDock| {
                            dock.items
                                .into_iter()
                                .map(|item| {
                                    let screen = item.screen;
                                    view! {
                                        <button
                                            class="dock-item"
                                            class:on=item.selected
                                            on:click=move |_| {
                                                runtime.ui(EventUi::ScreenOpened { screen })
                                            }
                                        >
                                            <Icon name=item.icon />
                                            {item.label}
                                        </button>
                                    }
                                })
                                .collect_view()
                        })
                }}
            </nav>

            // The sketch draws this button but never the screen behind it, so
            // it stays inert rather than opening something invented. See
            // docs/DECISIONS.md.
            <button
                class="dock-search glass"
                aria-label="Search"
                title="Search is not designed yet"
                disabled
            >
                <Icon name="search" />
            </button>
        </div>
    }
}

/// The status bar glyphs — signal, wifi, battery. Scenery, so that the frame
/// reads as a phone rather than as a narrow web page.
#[component]
fn StatusGlyphs() -> impl IntoView {
    view! {
        <svg viewBox="0 0 90 24" class="status-glyphs" aria-hidden="true">
            <g fill="currentColor">
                <rect x="0" y="14" width="3.6" height="6" rx="1" />
                <rect x="5.5" y="11" width="3.6" height="9" rx="1" />
                <rect x="11" y="7.5" width="3.6" height="12.5" rx="1" />
                <rect x="16.5" y="4" width="3.6" height="16" rx="1" />
            </g>
            <g
                fill="none"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                transform="translate(26 0)"
            >
                <path d="M2 8.5a15 15 0 0 1 20 0" />
                <path d="M5.5 12.5a10 10 0 0 1 13 0" />
                <path d="M9 16.5a5 5 0 0 1 6 0" />
            </g>
            <circle cx="38" cy="20" r="1" fill="currentColor" />
            <g transform="translate(54 0)" fill="none" stroke="currentColor" stroke-width="1.6">
                <rect x="1.5" y="7.5" width="19" height="9" rx="2.6" />
                <rect
                    x="3.5"
                    y="9.5"
                    width="13"
                    height="5"
                    rx="1.2"
                    fill="currentColor"
                    stroke="none"
                />
                <path d="M22.6 11v2" stroke-linecap="round" />
            </g>
        </svg>
    }
}
