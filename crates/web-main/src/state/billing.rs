//! Whether this session has paid, and so which screen the app is.

use leptos::prelude::*;

use crate::io::BillingStatus;
use crate::io::Meridian;
use crate::io::Refusal;
use crate::io::token;

/// Where a session stands, as far as the server has been asked.
#[derive(Debug, Clone, PartialEq)]
pub enum Standing {
    /// Still asking, or nothing to ask about. There is no sign-in on the web
    /// yet, so a browser holding no token falls through to the app rather
    /// than to a screen that does not exist.
    Unknown,

    /// The app.
    Entitled,

    /// The paywall, and what the server said, to draw it from. The status is
    /// absent where the paywall was reached by a 402 on some other route
    /// rather than by asking.
    Unpaid(Option<BillingStatus>),
}

/// What the app knows about this session's subscription.
#[derive(Clone, Copy)]
pub struct BillingState {
    status: LocalResource<Result<BillingStatus, Refusal>>,
}

impl BillingState {
    pub fn new() -> Self {
        let status = LocalResource::new(|| async {
            let token = token().ok_or(Refusal::Unauthorized)?;

            Meridian::new()
                .map_err(Refusal::Failed)?
                .billing_status(&token)
                .await
        });

        Self { status }
    }

    /// Read inside a view, so the screen changes when the answer lands.
    pub fn standing(&self) -> Standing {
        match self.status.get() {
            None => Standing::Unknown,

            Some(Ok(status)) if status.entitled => Standing::Entitled,
            Some(Ok(status)) => Standing::Unpaid(Some(status)),

            // A 402 is the paywall wherever it comes from. A 401 is a sign-in
            // screen and anything else is a failure, and neither of those is
            // this screen's business.
            Some(Err(Refusal::PaymentRequired)) => Standing::Unpaid(None),
            Some(Err(_)) => Standing::Unknown,
        }
    }
}
