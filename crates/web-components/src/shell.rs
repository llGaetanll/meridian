use leptos::prelude::*;
use web_ui::Icon;
use web_ui::IconKind;

/// The globe mark, sized by `.brand-mark`.
const BRAND_MARK: &str = r##"<defs><clipPath id="meridian-globe-clip"><circle cx="100" cy="100" r="93"/></clipPath></defs><g clip-path="url(#meridian-globe-clip)"><g stroke-width="14"><path d="M 181.753,73.308 C 171.596,42.2,126.76,28.932,81.61,43.674 C 36.459,58.415,8.09,95.584,18.247,126.692"/><path d="M 18.247,126.692 C 8.09,95.584,36.459,58.415,81.61,43.674 C 126.76,28.932,171.596,42.2,181.753,73.308"/><path d="M 118.172,15.942 C 108.574,13.867,92.658,49.819,82.622,96.243 C 72.586,142.667,72.23,181.983,81.828,184.058"/><path d="M 81.828,184.058 C 72.23,181.983,72.586,142.667,82.622,96.243 C 92.658,49.819,108.574,13.867,118.172,15.942"/><path d="M 14,100 C 14,133.585,52.504,160.811,100,160.811 C 147.496,160.811,186,133.585,186,100"/><path d="M 100,14 C 66.415,14,39.189,33.252,39.189,57 C 39.189,80.748,66.415,100,100,100 C 133.585,100,160.811,80.748,160.811,57 C 160.811,33.252,133.585,14,100,14"/></g><g stroke-width="14"><path d="M 147.342,171.797 C 166.464,159.188,160.77,116.822,134.624,77.17 C 108.477,37.517,71.78,15.595,52.658,28.203"/><path d="M 52.658,28.203 C 71.78,15.595,108.477,37.517,134.624,77.17 C 160.77,116.822,166.464,159.188,147.342,171.797"/></g></g><circle cx="100" cy="100" r="86" stroke-width="14"/>"##;

const AVATAR: &str = r##"<defs><linearGradient id="av" x1="0" y1="0" x2="1" y2="1"><stop offset="0%" stop-color="var(--violet-400)"/><stop offset="100%" stop-color="var(--violet-700)"/></linearGradient></defs><rect width="40" height="40" fill="url(#av)"/><circle cx="20" cy="14.8" r="7.2" fill="var(--bg)" opacity="0.34"/><path d="M4.6 40c0-8.6 6.9-13.6 15.4-13.6S35.4 31.4 35.4 40z" fill="var(--bg)" opacity="0.34"/>"##;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavItem {
    Home,
    Portfolios,
    Markets,
    Spending,
    Ledger,
    Connections,
}

impl NavItem {
    pub const ALL: [Self; 6] = [
        Self::Home,
        Self::Portfolios,
        Self::Markets,
        Self::Spending,
        Self::Ledger,
        Self::Connections,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Portfolios => "Portfolios",
            Self::Markets => "Markets",
            Self::Spending => "Spending",
            Self::Ledger => "Ledger",
            Self::Connections => "Connections",
        }
    }

    pub fn icon(&self) -> IconKind {
        match self {
            Self::Home => IconKind::Home,
            Self::Portfolios => IconKind::Portfolio,
            Self::Markets => IconKind::Markets,
            Self::Spending => IconKind::Spending,
            Self::Ledger => IconKind::Ledger,
            Self::Connections => IconKind::Connections,
        }
    }
}

/// Sidebar plus scrolling content area. Pages render inside `.content`.
#[component]
pub fn Shell(
    active: NavItem,
    #[prop(into)] account_name: String,
    #[prop(into)] account_status: String,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="shell">
            <Sidebar active=active account_name=account_name account_status=account_status />
            <div class="main">
                <div class="content">{children()}</div>
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
    active: NavItem,
    #[prop(into)] account_name: String,
    #[prop(into)] account_status: String,
) -> impl IntoView {
    view! {
        <aside class="sidebar">
            <Brand />

            <nav class="nav">
                {NavItem::ALL
                    .into_iter()
                    .map(|item| {
                        // Routing is not wired up yet.
                        view! {
                            <a class=if item == active { "active" } else { "" } href="#">
                                <Icon kind=item.icon() class="icon" />
                                {item.label()}
                            </a>
                        }
                    })
                    .collect_view()}
            </nav>

            <div class="sidebar-foot">
                <nav class="nav">
                    <a href="#">
                        <Icon kind=IconKind::Settings class="icon" />
                        Settings
                    </a>
                </nav>
                <div class="account">
                    <div class="avatar">
                        <svg viewBox="0 0 40 40" aria-hidden="true" inner_html=AVATAR />
                    </div>
                    <div class="account-id">
                        <div class="account-name">{account_name}</div>
                        <div class="account-meta">
                            <span class="status-dot" />
                            {account_status}
                        </div>
                    </div>
                    <span class="more">
                        <Icon kind=IconKind::ChevronUpDown />
                    </span>
                </div>
            </div>
        </aside>
    }
}
