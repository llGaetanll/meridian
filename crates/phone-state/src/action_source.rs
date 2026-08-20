//! Something for a source to do.

#[derive(Debug, uniffi::Enum)]
pub enum ActionSource {
    /// Go and get everything. `force` skips whatever cache sits in the way,
    /// which is what pull-to-refresh means.
    LoadSnapshot { force: bool },
}
