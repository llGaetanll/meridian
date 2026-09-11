//! What goes back when a request is refused.

use axum::Json;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::response::Response;

use server_wire::WireUnpaid;

/// A refusal, and whatever the status code cannot say on its own.
///
/// Only the 402 carries anything. A 401 and a 429 are wholly said by their
/// code — sign in, or wait — while "you have not paid" is one code and several
/// screens, and which one is a fact only the server holds.
#[derive(Debug)]
pub struct Refusal {
    status: StatusCode,

    /// Present exactly on a [`StatusCode::PAYMENT_REQUIRED`].
    unpaid: Option<WireUnpaid>,
}

impl Refusal {
    /// Signed in, and not paid for. **402.**
    pub fn unpaid(unpaid: WireUnpaid) -> Self {
        Self {
            status: StatusCode::PAYMENT_REQUIRED,
            unpaid: Some(unpaid),
        }
    }
}

/// Every other refusal, so a route still writes `Err(StatusCode::NOT_FOUND)`
/// and a `?` on a `StatusCode` still works.
impl From<StatusCode> for Refusal {
    fn from(status: StatusCode) -> Self {
        debug_assert_ne!(
            status,
            StatusCode::PAYMENT_REQUIRED,
            "a 402 carries why, and this one would go out without it",
        );

        Self {
            status,
            unpaid: None,
        }
    }
}

impl IntoResponse for Refusal {
    fn into_response(self) -> Response {
        match self.unpaid {
            Some(unpaid) => (self.status, Json(unpaid)).into_response(),
            None => self.status.into_response(),
        }
    }
}
