//! What Stripe says when it refuses, turned into a [`StripeError`].

use serde::Deserialize;

use crate::StripeError;

/// The envelope every Stripe failure arrives in.
#[derive(Debug, Deserialize)]
struct Envelope {
    error: Rejection,
}

/// Stripe's account of one failure. Every field is optional because which of
/// them arrives depends on the kind, and a missing one is not a parse failure.
#[derive(Debug, Deserialize)]
struct Rejection {
    /// `card_error`, `invalid_request_error`, `api_error`.
    r#type: Option<String>,

    /// The specific one: `card_declined`, `resource_missing`. Finer than
    /// `type` and the one worth branching on where there is a branch.
    code: Option<String>,

    /// Meant for a human, and occasionally for the cardholder.
    message: Option<String>,
}

/// Read a refusal. A body that is not one still produces an error — the status
/// already said this failed, so the parse is about what to call it.
pub(crate) fn rejection(status: u16, body: &str) -> StripeError {
    let Ok(envelope) = serde_json::from_str::<Envelope>(body) else {
        return StripeError::Stripe {
            status,
            kind: None,
            message: body.to_string(),
        };
    };

    let error = envelope.error;

    StripeError::Stripe {
        status,
        kind: error.code.or(error.r#type),
        message: error
            .message
            .unwrap_or_else(|| "Stripe gave no message".to_string()),
    }
}
