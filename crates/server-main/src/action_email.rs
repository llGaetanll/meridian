//! Something to send to a person.

#[derive(Debug)]
pub enum ActionEmail {
    /// Send this sign-in code to this address.
    ///
    /// Carries the code in the clear, because the runtime has to put it in a
    /// message somebody reads. It is the one action that does, and it is why
    /// `docs/LOGGING.md` lists the action log among the places a credential
    /// currently lands.
    SendCode { email: String, code: String },
}
