#[derive(Debug)]
pub enum PError {
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

impl From<reqwest::Error> for PError {
    fn from(err: reqwest::Error) -> Self {
        PError::ReqwestError(err)
    }
}

impl From<serde_json::Error> for PError {
    fn from(err: serde_json::Error) -> Self {
        PError::JsonError(err)
    }
}

impl std::fmt::Display for PError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PError::ReqwestError(e) => write!(f, "Network error: {}", e),
            PError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            PError::TooManyRequests => write!(f, "Rate limit exceeded"),
            PError::InvalidUrl(e) => write!(f, "Invalid url: {}", e),
            PError::Plaid {
                error_code,
                error_message,
            } => write!(f, "Plaid error {}: {}", error_code, error_message),
        }
    }
}

impl std::error::Error for PError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PError::ReqwestError(e) => Some(e),
            PError::JsonError(e) => Some(e),
            PError::TooManyRequests | PError::InvalidUrl(_) | PError::Plaid { .. } => None,
        }
    }
}
