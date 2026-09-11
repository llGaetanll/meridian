//! Where the server is in its own life.

/// One way through, and both ends are still.
#[derive(Debug, PartialEq, Eq)]
pub enum Lifecycle {
    /// Up, and gathering who has paid. Nothing that would be decided from
    /// that is served until it is in: an empty table would answer every
    /// subscriber as somebody who never paid.
    ///
    /// `retry` is a startup read the store refused, asked again on the next
    /// tick rather than at once — a store that fails fast would otherwise be
    /// asked in a loop.
    Starting { retry: bool },

    /// Serving. Whether a credential can be minted is not this: that is
    /// whether there is a generator, and [`StateSignup`](crate::StateSignup)
    /// holds it.
    Running,

    /// A signal has arrived. What is outstanding finishes; nothing new starts
    /// and nothing new is minted.
    Stopping,

    /// Nothing is outstanding and nothing more will be. The runtime's loop
    /// ends here, because only the machine knows what it is owed.
    Stopped,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Lifecycle::Starting { retry: false }
    }
}
