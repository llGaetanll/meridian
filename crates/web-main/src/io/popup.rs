//! The window another party's page is shown in — Plaid's bank sign-in, or
//! Stripe's Checkout.

use std::time::Duration;

use web_sys::Window;

use crate::io::clock;

/// How often to look at whether the window is still there. The browser says
/// nothing when somebody closes one, so it has to be asked.
const LOOK_EVERY: Duration = Duration::from_millis(400);

/// A window of its own, the browser's answer to the phone's sheet.
///
/// Opened empty and pointed at the page afterwards, because a browser only
/// lets a page open a window from inside the click that asked for it — and
/// the address is not known until the server answers, which is well after
/// the click has returned.
pub struct Popup {
    window: Window,
}

impl Popup {
    /// Open an empty window. `None` is a browser that would not: a popup
    /// blocker, or a call that did not come from a click.
    pub fn open() -> Option<Self> {
        let opened = web_sys::window()?.open_with_url_and_target_and_features(
            "",
            "meridian-bank",
            "popup,width=480,height=760",
        );

        match opened {
            Ok(Some(window)) => Some(Self { window }),
            Ok(None) | Err(_) => None,
        }
    }

    /// Put the party's page in it.
    pub fn show(&self, url: &str) {
        if self.window.location().set_href(url).is_err() {
            tracing::error!("the window would not take the page");
        }
    }

    /// Wait until it is gone, and answer with the address it came home to.
    ///
    /// Either the person closed it — `None` — or the page finished and sent
    /// the window back to this app's own origin, which this closes, since
    /// there is nothing left in it to show. Which of our pages it came home
    /// to is the one thing the window itself says: for Checkout it is how
    /// paid and backed-out are told apart. What a bank session came to is
    /// `/link/finish`'s to say, so those callers read nothing from it.
    pub async fn gone(&self) -> Option<String> {
        let home = web_sys::window().and_then(|window| window.location().origin().ok());

        loop {
            if self.window.closed().unwrap_or(true) {
                return None;
            }

            // Readable only while it shows a page of this origin. The other
            // party's is another, and reading its address throws.
            let back = match (&home, self.window.location().href()) {
                (Some(home), Ok(href)) if href.starts_with(home.as_str()) => Some(href),
                _ => None,
            };

            if let Some(href) = back {
                self.close();

                return Some(href);
            }

            clock::sleep(LOOK_EVERY).await;
        }
    }

    /// Close it, whatever it is showing.
    pub fn close(&self) {
        let _ = self.window.close();
    }
}
