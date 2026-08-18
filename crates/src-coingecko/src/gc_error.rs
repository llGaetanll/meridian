#[derive(Debug)]
pub enum GcError {
    // Network/HTTP errors
    ReqwestError(reqwest::Error),

    // JSON parsing errors
    JsonError(serde_json::Error),

    // Rate limiting
    TooManyRequests,
}

impl From<reqwest::Error> for GcError {
    fn from(err: reqwest::Error) -> Self {
        GcError::ReqwestError(err)
    }
}

impl From<serde_json::Error> for GcError {
    fn from(err: serde_json::Error) -> Self {
        GcError::JsonError(err)
    }
}

impl std::fmt::Display for GcError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            GcError::ReqwestError(e) => write!(f, "Network error: {}", e),
            GcError::JsonError(e) => write!(f, "JSON parsing error: {}", e),
            GcError::TooManyRequests => write!(f, "Rate limit exceeded"),
        }
    }
}

impl std::error::Error for GcError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            GcError::ReqwestError(e) => Some(e),
            GcError::JsonError(e) => Some(e),
            GcError::TooManyRequests => None,
        }
    }
}
