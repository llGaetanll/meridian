//! Who a Link session is being opened for.

use crate::WebOrigins;

/// Where the person asking for a bank sign-in will be looking at it, which
/// decides where Plaid sends them when it ends.
///
/// Decided from the request's `Origin`, and an origin only ever reaches Plaid
/// from [`WebOrigins`]: a redirect Plaid follows is an address we vouch for,
/// and a header is something anybody can type.
#[derive(Debug, Clone)]
pub enum LinkOpener {
    /// The phone, in a web session it holds open over the app.
    ///
    /// Plaid's page opens [`APP_REDIRECT`], which closes that session; and an
    /// OAuth bank hands back to the app, where the deployment has somewhere
    /// for it to hand back to.
    App,

    /// The web app, served from this origin, in a popup it opened.
    ///
    /// Plaid sends the popup to [`WEB_REDIRECT_PATH`] on the web app's own
    /// origin, a page that closes itself. An OAuth bank stays inside the popup:
    /// the handback is a Universal Link, and it is the phone's.
    Web { origin: String },
}

/// What closes the phone's web session. It carries no token, and it is allowed
/// to be a custom scheme because Plaid's own page is what opens it, inside the
/// session the app is already holding.
pub const APP_REDIRECT: &str = "meridian://linked";

/// The page on the web app's origin that closes the popup Link was opened in.
pub const WEB_REDIRECT_PATH: &str = "/linked.html";

impl LinkOpener {
    /// Who is asking, from the `Origin` the request carried.
    ///
    /// No origin is the phone, which is not a browser and sends none. An
    /// origin that is not the web app's is treated the same way rather than
    /// refused: it is never a browser the CORS layer let through, and the
    /// phone's redirect is one that goes nowhere for anybody else.
    pub fn of(origin: Option<&str>, web: &WebOrigins) -> Self {
        let Some(origin) = origin else {
            return LinkOpener::App;
        };

        if !web.allows(origin) {
            tracing::warn!(
                origin,
                "a bank sign-in was asked for from an origin that is not the web app's"
            );

            return LinkOpener::App;
        }

        LinkOpener::Web {
            origin: origin.to_string(),
        }
    }

    /// Where Plaid sends the browser when the flow ends, whether or not a bank
    /// was connected.
    pub fn completion_redirect(&self) -> String {
        match self {
            LinkOpener::App => APP_REDIRECT.to_string(),
            LinkOpener::Web { origin } => format!("{origin}{WEB_REDIRECT_PATH}"),
        }
    }

    /// Whether an OAuth bank may hand back to the app. Only the phone is an app
    /// a bank can hand back to, and telling Plaid otherwise strands somebody in
    /// their bank's app after they have signed in.
    pub fn hands_back(&self) -> bool {
        matches!(self, LinkOpener::App)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn web() -> WebOrigins {
        WebOrigins::parse("https://app.meridiancash.io,http://127.0.0.1:8080").unwrap()
    }

    #[test]
    fn the_phone_sends_no_origin_and_is_sent_back_to_the_app() {
        let opener = LinkOpener::of(None, &web());

        assert_eq!(opener.completion_redirect(), "meridian://linked");
        assert!(opener.hands_back());
    }

    #[test]
    fn the_web_app_is_sent_back_to_its_own_origin() {
        let opener = LinkOpener::of(Some("http://127.0.0.1:8080"), &web());

        assert_eq!(
            opener.completion_redirect(),
            "http://127.0.0.1:8080/linked.html"
        );
        assert!(!opener.hands_back());
    }

    /// The header is the caller's to write, so the list is what decides and
    /// the header only picks from it.
    #[test]
    fn an_origin_that_is_not_the_web_apps_is_never_a_redirect() {
        for origin in [
            "https://evil.example",
            "https://app.meridiancash.io.evil.example",
            "https://app.meridiancash.io/",
            "HTTPS://APP.MERIDIANCASH.IO",
            "null",
            "",
        ] {
            let opener = LinkOpener::of(Some(origin), &web());

            assert_eq!(
                opener.completion_redirect(),
                "meridian://linked",
                "`{origin}` must not reach Plaid",
            );
        }
    }
}
