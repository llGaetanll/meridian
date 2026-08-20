#[derive(Debug)]
pub enum PlaidError {
    // Network/HTTP errors
    ReqwestError(reqwest::Error),

    // JSON parsing errors
    JsonError(serde_json::Error),

    // Rate limiting
    TooManyRequests,

    // Malformed base url or request path
    InvalidUrl(String),

    /// Plaid answered with an error of its own. It uses the body rather than
    /// the status to say what went wrong, and `error_code` is the part worth
    /// matching on — `ITEM_LOGIN_REQUIRED` means the user has to reconnect,
    /// and no amount of retrying will fix it.
    Plaid {
        error_code: String,
        error_message: String,
    },
}

impl From<reqwest::Error> for PlaidError {
    fn from(err: reqwest::Error) -> Self {
        PlaidError::ReqwestError(err)
    }
}

impl From<serde_json::Error> for PlaidError {
    fn from(err: serde_json::Error) -> Self {
        PlaidError::JsonError(err)
    }
}

impl std::fmt::Display for PlaidError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PlaidError::ReqwestError(e) => write!(f, "Network error: {}", e),
            PlaidError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            PlaidError::TooManyRequests => write!(f, "Rate limit exceeded"),
            PlaidError::InvalidUrl(e) => write!(f, "Invalid url: {}", e),
            PlaidError::Plaid {
                error_code,
                error_message,
            } => write!(f, "Plaid error {}: {}", error_code, error_message),
        }
    }
}

impl std::error::Error for PlaidError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PlaidError::ReqwestError(e) => Some(e),
            PlaidError::JsonError(e) => Some(e),
            PlaidError::TooManyRequests | PlaidError::InvalidUrl(_) | PlaidError::Plaid { .. } => {
                None
            }
        }
    }
}
