//! The icon set the machine names.
//!
//! `client_state` sends an icon by name (`"mail"`, `"tick-circle"`) and each
//! platform resolves it in its own set. This is the web's, drawn from the same
//! path data as `swift/Sources/MeridianApp/Ui/Icon.swift` and the symbols in
//! `sketches/design-language.html`, so the two platforms show one glyph rather
//! than two that look close.

use leptos::prelude::*;

/// One icon, by the name the machine sent. Size and colour are the
/// stylesheet's; the stroke is set in the 24-unit box, so it scales with the
/// icon.
///
/// An unknown name draws nothing rather than a "missing" glyph, as the phone
/// does: a gap is easier to spot in review than a box that looks deliberate.
#[component]
pub fn Glyph(
    #[prop(into)] name: Signal<String>,
    #[prop(optional, into)] class: String,
) -> impl IntoView {
    view! {
        <svg
            class=class
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width=move || name.with(|name| weight(name))
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            inner_html=move || name.with(|name| paths(name))
        />
    }
}

/// The stroke each glyph is drawn at, as the sheet's symbols set it. The
/// stylesheet's `stroke-width` overrides it where a call site wants another.
fn weight(name: &str) -> &'static str {
    match name {
        "spin" => "2.4",
        "back" | "chevron" | "plus" | "prev" | "next" => "2.2",
        "ext" | "tick-circle" | "warn" => "1.9",
        _ => "1.8",
    }
}

fn paths(name: &str) -> &'static str {
    match name {
        "home" => {
            r#"<path d="M3 10.2 12 3l9 7.2"/><path d="M5.5 9.2V20a1 1 0 0 0 1 1H10v-6h4v6h3.5a1 1 0 0 0 1-1V9.2"/>"#
        }
        "ledger" => {
            r#"<line x1="9" y1="6" x2="21" y2="6"/><line x1="9" y1="12" x2="21" y2="12"/><line x1="9" y1="18" x2="21" y2="18"/><circle cx="4" cy="6" r="1"/><circle cx="4" cy="12" r="1"/><circle cx="4" cy="18" r="1"/>"#
        }
        "recurring" => {
            r#"<path d="M3 11a9 9 0 0 1 15-6.7L21 7"/><polyline points="21 3 21 7 17 7"/><path d="M21 13a9 9 0 0 1-15 6.7L3 17"/><polyline points="3 21 3 17 7 17"/>"#
        }
        "balances" => {
            r#"<path d="M21.21 15.89A10 10 0 1 1 8 2.83"/><path d="M22 12A10 10 0 0 0 12 2v10z"/>"#
        }
        "search" => r#"<circle cx="11" cy="11" r="7"/><line x1="16.5" y1="16.5" x2="21" y2="21"/>"#,
        "filter" => {
            r#"<line x1="4" y1="7" x2="20" y2="7"/><line x1="7" y1="12" x2="17" y2="12"/><line x1="10" y1="17" x2="14" y2="17"/>"#
        }
        "chevron" => r#"<polyline points="6 9 12 15 18 9"/>"#,
        "caret" => r#"<polygon points="6 9 18 9 12 16" fill="currentColor" stroke="none"/>"#,
        "eye" => {
            r#"<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/>"#
        }
        "eye-off" => {
            r#"<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/><line x1="3" y1="21" x2="21" y2="3"/>"#
        }
        "tag" => {
            r#"<path d="M20.6 13.4 12 4.8H4.8V12l8.6 8.6a1.7 1.7 0 0 0 2.4 0l4.8-4.8a1.7 1.7 0 0 0 0-2.4Z"/><circle cx="8.4" cy="8.4" r="1.2"/>"#
        }
        "clock" => r#"<circle cx="12" cy="12" r="9"/><polyline points="12 7 12 12 15.5 14"/>"#,
        "ext" => {
            r#"<path d="M14 4h6v6"/><path d="M20 4 12.5 11.5"/><path d="M18.5 14v5.5a1 1 0 0 1-1 1h-13a1 1 0 0 1-1-1v-13a1 1 0 0 1 1-1H10"/>"#
        }
        "refresh" => {
            r#"<path d="M3 12a9 9 0 0 1 15-6.7L21 8"/><polyline points="21 3 21 8 16 8"/>"#
        }
        "mail" => {
            r#"<rect x="2.5" y="5" width="19" height="14" rx="2.5"/><path d="m3 7 9 6 9-6"/>"#
        }
        "back" => r#"<polyline points="15 5 8 12 15 19"/>"#,
        "tick-circle" => {
            r#"<circle cx="12" cy="12" r="9"/><polyline points="16 9.5 10.8 15 8 12.4"/>"#
        }
        "warn" => {
            r#"<circle cx="12" cy="12" r="9"/><line x1="12" y1="7.5" x2="12" y2="13"/><circle cx="12" cy="16.5" r="0.6" fill="currentColor"/>"#
        }
        "lock" => {
            r#"<rect x="4.5" y="10.5" width="15" height="10" rx="2.5"/><path d="M8 10.5V7.8a4 4 0 0 1 8 0v2.7"/>"#
        }
        "bank" => {
            r#"<path d="m3 9.5 9-5.5 9 5.5"/><line x1="4" y1="20" x2="20" y2="20"/><line x1="6.5" y1="12" x2="6.5" y2="17"/><line x1="12" y1="12" x2="12" y2="17"/><line x1="17.5" y1="12" x2="17.5" y2="17"/>"#
        }
        "user" => r#"<circle cx="12" cy="8.5" r="3.8"/><path d="M4.5 20.5a7.5 7.5 0 0 1 15 0"/>"#,
        "trash" => {
            r#"<path d="M4.5 6.5h15"/><path d="M9 6.5V4.8h6v1.7"/><path d="M6.5 6.5 7.4 20h9.2l.9-13.5"/>"#
        }
        // The arc that turns, over the faint rest of its circle.
        "spin" => {
            r#"<path d="M12 3.5a8.5 8.5 0 1 1-8.5 8.5"/><path d="M3.5 12A8.5 8.5 0 0 1 12 3.5" opacity="0.2"/>"#
        }
        "dot" => r#"<circle cx="12" cy="12" r="8"/>"#,
        "plus" => r#"<line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>"#,
        "prev" => r#"<polyline points="15 18 9 12 15 6"/>"#,
        "next" => r#"<polyline points="9 18 15 12 9 6"/>"#,
        _ => "",
    }
}
