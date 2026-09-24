use leptos::prelude::*;

/// The icon set used across the app. Every icon is drawn on a 24x24 grid with
/// `currentColor` strokes, so sizing and coloring is left to CSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconKind {
    Portfolio,
    Markets,
    Spending,
    Ledger,
    Connections,
    Settings,
    Search,
    ChevronDown,
    ChevronUpDown,
    ArrowRight,
    Swap,
    SignOut,
    Trash,

    /// A head and shoulders, for the settings rail's one page. The same
    /// drawing as the `"user"` glyph the machine names.
    User,

    /// The cross that shuts a window.
    Close,

    /// A disc half filled, for the row that turns the theme over. One glyph
    /// for both themes rather than a sun and a moon: the row says what the
    /// theme is in words beside it, and a glyph that changes with the value
    /// is a second answer to the same question.
    Theme,
}

impl IconKind {
    fn paths(&self) -> &'static str {
        match self {
            Self::Portfolio => {
                r#"<path d="M21.21 15.89A10 10 0 1 1 8 2.83"/><path d="M22 12A10 10 0 0 0 12 2v10z"/>"#
            }
            Self::Markets => {
                r#"<polyline points="22 7 13.5 15.5 8.5 10.5 2 17"/><polyline points="16 7 22 7 22 13"/>"#
            }
            Self::Spending => {
                r#"<rect x="2" y="5" width="20" height="14" rx="2"/><line x1="2" y1="10" x2="22" y2="10"/>"#
            }
            Self::Ledger => {
                r#"<line x1="9" y1="6" x2="21" y2="6"/><line x1="9" y1="12" x2="21" y2="12"/><line x1="9" y1="18" x2="21" y2="18"/><circle cx="4" cy="6" r="1"/><circle cx="4" cy="12" r="1"/><circle cx="4" cy="18" r="1"/>"#
            }
            Self::Connections => {
                r#"<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>"#
            }
            Self::Settings => {
                r#"<line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/>"#
            }
            Self::Search => {
                r#"<circle cx="11" cy="11" r="7"/><line x1="16.5" y1="16.5" x2="21" y2="21"/>"#
            }
            Self::ChevronDown => r#"<polyline points="6 9 12 15 18 9"/>"#,
            Self::ChevronUpDown => {
                r#"<polyline points="8 9 12 5 16 9"/><polyline points="16 15 12 19 8 15"/>"#
            }
            Self::ArrowRight => {
                r#"<line x1="4" y1="12" x2="19" y2="12"/><polyline points="13 6 19 12 13 18"/>"#
            }
            Self::Swap => {
                r#"<polyline points="8 5 12 1 16 5"/><line x1="12" y1="1" x2="12" y2="10"/><polyline points="16 19 12 23 8 19"/><line x1="12" y1="23" x2="12" y2="14"/>"#
            }
            Self::SignOut => {
                r#"<path d="M9.5 4H5.5A1.5 1.5 0 0 0 4 5.5v13A1.5 1.5 0 0 0 5.5 20h4"/><polyline points="15.5 8 19.5 12 15.5 16"/><line x1="19.5" y1="12" x2="9.5" y2="12"/>"#
            }
            Self::Trash => {
                r#"<path d="M4.5 6.5h15"/><path d="M9 6.5V4.8h6v1.7"/><path d="M6.5 6.5 7.4 20h9.2l.9-13.5"/>"#
            }
            Self::User => {
                r#"<circle cx="12" cy="8.5" r="3.8"/><path d="M4.5 20.5a7.5 7.5 0 0 1 15 0"/>"#
            }
            Self::Close => {
                r#"<line x1="6" y1="6" x2="18" y2="18"/><line x1="18" y1="6" x2="6" y2="18"/>"#
            }
            Self::Theme => {
                r#"<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5a8.5 8.5 0 0 1 0 17z" fill="currentColor" stroke="none"/>"#
            }
        }
    }
}

#[component]
pub fn Icon(kind: IconKind, #[prop(optional, into)] class: String) -> impl IntoView {
    view! {
        <svg
            class=class
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            inner_html=kind.paths()
        />
    }
}
