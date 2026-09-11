//! The window Plaid's page is shown in.

use std::time::Duration;

use web_sys::Window;

use crate::io::clock;

/// How often to look at whether the window is still there. The browser says
/// nothing when somebody closes one, so it has to be asked.
const LOOK_EVERY: Duration = Duration::from_millis(400);

/// A window of its own, the browser's answer to the phone's sheet.
///
/// Opened empty and pointed at Plaid afterwards, because a browser only lets
/// a page open a window from inside the click that asked for it — and the
/// address is not known until `/link/start` answers, which is well after the
/// click has returned.
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

    /// Put Plaid's page in it.
    pub fn show(&self, url: &str) {
        if self.window.location().set_href(url).is_err() {
            tracing::error!("the bank's window would not take Plaid's page");
        }
    }

    /// Wait until it is gone.
    ///
    /// Either the person closed it, or Plaid's page finished and sent it back
    /// to this app's own address — which this closes, since there is nothing
    /// left in it to show. The redirect that ends a Link session is the
    /// server's choice; both endings are the same fact here, and what the
    /// session came to is `/link/finish`'s to say.
    pub async fn gone(&self) {
        let home = web_sys::window().and_then(|window| window.location().origin().ok());

        loop {
            if self.window.closed().unwrap_or(true) {
                return;
            }

            // Readable only while it shows a page of this origin. Plaid's is
            // another, and reading its address throws.
            let back = match (&home, self.window.location().href()) {
                (Some(home), Ok(href)) => href.starts_with(home.as_str()),
                _ => false,
            };

            if back {
                self.close();

                return;
            }

            clock::sleep(LOOK_EVERY).await;
        }
    }

    /// Close it, whatever it is showing.
    pub fn close(&self) {
        let _ = self.window.close();
    }
}
