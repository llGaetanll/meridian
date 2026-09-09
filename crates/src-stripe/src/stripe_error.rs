/// What can go wrong on the way to Stripe and back.
#[derive(Debug)]
pub enum StripeError {
    /// The request never made it there, or never made it back.
    Request(reqwest::Error),

    /// Stripe answered, and said no. `kind` is its own name for the failure —
    /// `card_declined` and the rest — which is the part worth branching on.
    Stripe {
        status: u16,
        kind: Option<String>,
        message: String,
    },

    /// The environment did not say something it had to say.
    Config(String),
}

impl From<reqwest::Error> for StripeError {
    fn from(err: reqwest::Error) -> Self {
        StripeError::Request(err)
    }
}

impl std::fmt::Display for StripeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            StripeError::Request(e) => write!(f, "Network error: {e}"),
            StripeError::Stripe {
                status,
                kind: Some(kind),
                message,
            } => write!(f, "Stripe answered {status} ({kind}): {message}"),
            StripeError::Stripe {
                status,
                kind: None,
                message,
            } => write!(f, "Stripe answered {status}: {message}"),
            StripeError::Config(e) => write!(f, "Configuration error: {e}"),
        }
    }
}

impl std::error::Error for StripeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StripeError::Request(e) => Some(e),
            StripeError::Stripe { .. } | StripeError::Config(_) => None,
        }
    }
}
