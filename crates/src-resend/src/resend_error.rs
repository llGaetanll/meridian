#[derive(Debug)]
pub enum ResendError {
    /// The request never made it there, or never made it back.
    Request(reqwest::Error),

    /// Resend answered, and said no.
    Resend { status: u16, message: String },

    /// The environment did not say something it had to say.
    Config(String),
}

impl From<reqwest::Error> for ResendError {
    fn from(err: reqwest::Error) -> Self {
        ResendError::Request(err)
    }
}

impl std::fmt::Display for ResendError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ResendError::Request(e) => write!(f, "Network error: {e}"),
            ResendError::Resend { status, message } => {
                write!(f, "Resend answered {status}: {message}")
            }
            ResendError::Config(e) => write!(f, "Configuration error: {e}"),
        }
    }
}

impl std::error::Error for ResendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ResendError::Request(e) => Some(e),
            ResendError::Resend { .. } | ResendError::Config(_) => None,
        }
    }
}
