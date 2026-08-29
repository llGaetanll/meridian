//! Where a sign-in code actually goes.

use src_resend::ResendError;

/// How the server delivers an email.
///
/// Its own required variable rather than something inferred from whether
/// `RESEND_API_KEY` happens to be set. A key that is missing by accident and a
/// developer who meant to read codes off the log look identical from inside
/// the process, and only one of them should ever happen in front of a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailSend {
    /// Send it, through Resend.
    Resend,

    /// Write it to the log and send nothing.
    ///
    /// For development, and it is a real choice rather than a fallback: the
    /// code appears at `WARN` with a line saying no email was sent, so a
    /// server accidentally running this way says so on every sign-in.
    Log,
}

impl EmailSend {
    pub fn from_env() -> Result<Self, ResendError> {
        let value = std::env::var("EMAIL_SEND").map_err(|_| {
            ResendError::Config(String::from(
                "EMAIL_SEND is not set — it must be `resend` or `log`",
            ))
        })?;

        Self::parse(&value)
    }

    pub fn parse(value: &str) -> Result<Self, ResendError> {
        match value {
            "resend" => Ok(EmailSend::Resend),
            "log" => Ok(EmailSend::Log),
            other => Err(ResendError::Config(format!(
                "EMAIL_SEND is `{other}` — it must be `resend` or `log`",
            ))),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            EmailSend::Resend => "resend",
            EmailSend::Log => "log",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_ways_parse() {
        assert_eq!(EmailSend::parse("resend").unwrap(), EmailSend::Resend);
        assert_eq!(EmailSend::parse("log").unwrap(), EmailSend::Log);
    }

    #[test]
    fn anything_else_is_an_error() {
        for value in ["", "Resend", "smtp", "none", "dev"] {
            assert!(EmailSend::parse(value).is_err(), "`{value}` must not parse");
        }
    }
}
