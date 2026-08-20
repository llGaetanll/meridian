//! Something for the process itself to do.

#[derive(Debug)]
pub enum ActionProcess {
    /// Stop accepting connections and exit once in-flight work is done.
    Shutdown,

    /// This connection needs the user to sign in again, and no retry will fix
    /// it. Worth a loud line in the log: every sync until they do will fail.
    ReportLoginRequired,
}
