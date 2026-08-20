//! Something to send back to a client.

#[derive(Debug)]
pub enum ActionClient {
    /// Answer with what is currently held.
    ///
    /// The snapshot is assembled by the runtime from state rather than carried
    /// in the action: it is a projection of everything the server knows, and
    /// copying it into an action would mean building it even when nobody is
    /// waiting for one.
    Serve,
}
