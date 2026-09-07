//! Where the server is in its own life.

/// One way through, and both ends are still.
#[derive(Debug, Default, PartialEq, Eq)]
pub enum Lifecycle {
    /// Serving. Whether a credential can be minted is not this: that is
    /// whether there is a generator, and [`StateSignup`](crate::StateSignup)
    /// holds it.
    #[default]
    Running,

    /// A signal has arrived. What is outstanding finishes; nothing new starts
    /// and nothing new is minted.
    Stopping,

    /// Nothing is outstanding and nothing more will be. The runtime's loop
    /// ends here, because only the machine knows what it is owed.
    Stopped,
}
