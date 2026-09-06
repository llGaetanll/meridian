//! Something to send to a person.

#[derive(Debug)]
pub enum ActionEmail {
    /// Send this sign-in code to this address.
    ///
    /// The one action carrying a credential in the clear, which is why
    /// `docs/LOGGING.md` lists the action log among the places one lands.
    SendCode { email: String, code: String },
}
