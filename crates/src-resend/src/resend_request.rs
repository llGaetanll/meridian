use serde::Serialize;

/// One message.
///
/// `from` has to be an address on a domain verified in the Resend dashboard,
/// which is the one part of this that cannot be fixed in code.
#[derive(Debug, Clone, Serialize)]
pub struct Email {
    pub from: String,
    pub to: String,
    pub subject: String,

    /// Plain text. A sign-in code has nothing to lay out, and an HTML-only
    /// message is the one most likely to be filed as spam.
    pub text: String,
}

impl Email {
    pub fn new(
        from: impl Into<String>,
        to: impl Into<String>,
        subject: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            subject: subject.into(),
            text: text.into(),
        }
    }
}
