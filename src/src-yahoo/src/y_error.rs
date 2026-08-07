#[derive(Debug)]
pub enum YError {
    // Network/HTTP errors
    ReqwestError(reqwest::Error),

    // JSON parsing errors
    JsonError(serde_json::Error),

    // Rate limiting
    TooManyRequests,

    // Malformed base url or request path
    InvalidUrl(String),
}

impl From<reqwest::Error> for YError {
    fn from(err: reqwest::Error) -> Self {
        YError::ReqwestError(err)
    }
}

impl From<serde_json::Error> for YError {
    fn from(err: serde_json::Error) -> Self {
        YError::JsonError(err)
    }
}

impl std::fmt::Display for YError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            YError::ReqwestError(e) => write!(f, "Network error: {}", e),
            YError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            YError::TooManyRequests => write!(f, "Rate limit exceeded"),
            YError::InvalidUrl(e) => write!(f, "Invalid url: {}", e),
        }
    }
}

impl std::error::Error for YError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            YError::ReqwestError(e) => Some(e),
            YError::JsonError(e) => Some(e),
            YError::TooManyRequests | YError::InvalidUrl(_) => None,
        }
    }
}
