//! The icon set, as inline SVG.
//!
//! Inline rather than a sprite sheet: the phone app is one page, the icons are
//! few, and a `<use>` reference costs a document-wide id namespace that the
//! sketch had to work around. The names match the `icon` field the core puts
//! on a `ViewDockItem`, so the mapping from core to glyph is a lookup rather than
//! a decision.

use leptos::prelude::*;

#[component]
pub fn Icon(#[prop(into)] name: String, #[prop(optional, into)] class: String) -> impl IntoView {
    let class = format!("icon {class}");

    view! {
        <svg
            class=class
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            {path(&name)}
        </svg>
    }
}

fn path(name: &str) -> AnyView {
    match name {
        "home" => view! {
            <path d="M3 10.2 12 3l9 7.2" />
            <path d="M5.5 9.2V20a1 1 0 0 0 1 1H10v-6h4v6h3.5a1 1 0 0 0 1-1V9.2" />
        }
        .into_any(),

        "ledger" => view! {
            <line x1="9" y1="6" x2="21" y2="6" />
            <line x1="9" y1="12" x2="21" y2="12" />
            <line x1="9" y1="18" x2="21" y2="18" />
            <circle cx="4" cy="6" r="1" />
            <circle cx="4" cy="12" r="1" />
            <circle cx="4" cy="18" r="1" />
        }
        .into_any(),

        "recurring" => view! {
            <path d="M3 11a9 9 0 0 1 15-6.7L21 7" />
            <polyline points="21 3 21 7 17 7" />
            <path d="M21 13a9 9 0 0 1-15 6.7L3 17" />
            <polyline points="3 21 3 17 7 17" />
        }
        .into_any(),

        "balances" => view! {
            <path d="M21.21 15.89A10 10 0 1 1 8 2.83" />
            <path d="M22 12A10 10 0 0 0 12 2v10z" />
        }
        .into_any(),

        "search" => view! {
            <circle cx="11" cy="11" r="7" />
            <line x1="16.5" y1="16.5" x2="21" y2="21" />
        }
        .into_any(),

        "filter" => view! {
            <line x1="4" y1="7" x2="20" y2="7" />
            <line x1="7" y1="12" x2="17" y2="12" />
            <line x1="10" y1="17" x2="14" y2="17" />
        }
        .into_any(),

        "chevron" => view! { <polyline points="6 9 12 15 18 9" /> }.into_any(),

        // Open in new, and it is telling the truth: Plaid Link in update mode
        // is a hosted page, so the tap leaves the app.
        "ext" => view! {
            <path d="M14 4h6v6" />
            <path d="M20 4 12.5 11.5" />
            <path d="M18.5 14v5.5a1 1 0 0 1-1 1h-13a1 1 0 0 1-1-1v-13a1 1 0 0 1 1-1H10" />
        }
        .into_any(),

        "caret" => view! { <polygon points="6 9 18 9 12 16" fill="currentColor" stroke="none" /> }
            .into_any(),

        "eye" => view! {
            <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z" />
            <circle cx="12" cy="12" r="3" />
        }
        .into_any(),

        "eye-off" => view! {
            <path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z" />
            <circle cx="12" cy="12" r="3" />
            <line x1="3" y1="21" x2="21" y2="3" />
        }
        .into_any(),

        "tag" => view! {
            <path d="M20.6 13.4 12 4.8H4.8V12l8.6 8.6a1.7 1.7 0 0 0 2.4 0l4.8-4.8a1.7 1.7 0 0 0 0-2.4Z" />
            <circle cx="8.4" cy="8.4" r="1.2" />
        }
        .into_any(),

        "clock" => view! {
            <circle cx="12" cy="12" r="9" />
            <polyline points="12 7 12 12 15.5 14" />
        }
        .into_any(),

        "refresh" => view! {
            <path d="M3 12a9 9 0 0 1 15-6.7L21 8" />
            <polyline points="21 3 21 8 16 8" />
        }
        .into_any(),

        // First run's set. Drawn here rather than only in Swift because this
        // file is the drawing both apps copy: an icon that exists on one
        // platform is how two apps end up with two icon sets.
        "mail" => view! {
            <rect x="2.5" y="5" width="19" height="14" rx="2.5" />
            <path d="m3 7 9 6 9-6" />
        }
        .into_any(),

        // Back, and only back. The dock's `chevron` points down; this one is
        // the same stroke turned, so the two read as one family.
        "back" => view! { <polyline points="15 5 8 12 15 19" /> }.into_any(),

        "tick-circle" => view! {
            <circle cx="12" cy="12" r="9" />
            <polyline points="16 9.5 10.8 15 8 12.4" />
        }
        .into_any(),

        "warn" => view! {
            <circle cx="12" cy="12" r="9" />
            <line x1="12" y1="7.5" x2="12" y2="13" />
            <circle cx="12" cy="16.5" r="0.6" fill="currentColor" stroke="none" />
        }
        .into_any(),

        "lock" => view! {
            <rect x="4.5" y="10.5" width="15" height="10" rx="2.5" />
            <path d="M8 10.5V7.8a4 4 0 0 1 8 0v2.7" />
        }
        .into_any(),

        "bank" => view! {
            <path d="m3 9.5 9-5.5 9 5.5" />
            <line x1="4" y1="20" x2="20" y2="20" />
            <line x1="6.5" y1="12" x2="6.5" y2="17" />
            <line x1="12" y1="12" x2="12" y2="17" />
            <line x1="17.5" y1="12" x2="17.5" y2="17" />
        }
        .into_any(),

        "trash" => view! {
            <path d="M4.5 6.5h15" />
            <path d="M9 6.5V4.8h6v1.7" />
            <path d="M6.5 6.5 7.4 20h9.2l.9-13.5" />
        }
        .into_any(),

        // One three-quarter arc and no faint track behind it. The turning is
        // what says "working"; a second ring at a fifth of the opacity is a
        // detail nothing reads at 18px, and it would have to be a second shape
        // on a platform that strokes a glyph in one colour.
        "spin" => view! { <path d="M12 3.5a8.5 8.5 0 1 1-8.5 8.5" /> }.into_any(),

        // A step not reached yet: the tick's circle with nothing in it.
        "dot" => view! { <circle cx="12" cy="12" r="8" /> }.into_any(),

        // An unknown name draws nothing rather than a "missing" glyph: a gap
        // is easier to spot in review than a box that looks deliberate.
        _ => ().into_any(),
    }
}

/// The wordmark, from `sketches/brand/meridian-mark.svg`.
#[component]
pub fn BrandMark() -> impl IntoView {
    view! {
        <svg
            class="brand-mark"
            viewBox="0 0 200 200"
            fill="none"
            stroke="currentColor"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
        >
            <defs>
                <clipPath id="globe-clip">
                    <circle cx="100" cy="100" r="93" />
                </clipPath>
            </defs>
            <g clip-path="url(#globe-clip)" stroke-width="14">
                <path d="M 181.753,73.308 C 171.596,42.2,126.76,28.932,81.61,43.674 C 36.459,58.415,8.09,95.584,18.247,126.692" />
                <path d="M 118.172,15.942 C 108.574,13.867,92.658,49.819,82.622,96.243 C 72.586,142.667,72.23,181.983,81.828,184.058" />
                <path d="M 14,100 C 14,133.585,52.504,160.811,100,160.811 C 147.496,160.811,186,133.585,186,100" />
                <path d="M 100,14 C 66.415,14,39.189,33.252,39.189,57 C 39.189,80.748,66.415,100,100,100 C 133.585,100,160.811,80.748,160.811,57 C 160.811,33.252,133.585,14,100,14" />
                <path d="M 147.342,171.797 C 166.464,159.188,160.77,116.822,134.624,77.17 C 108.477,37.517,71.78,15.595,52.658,28.203" />
            </g>
            <circle cx="100" cy="100" r="86" stroke-width="14" />
        </svg>
    }
}
