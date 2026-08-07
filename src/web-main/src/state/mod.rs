//! Reactive state. It calls into [`crate::io`] but performs no requests itself.

mod markets;

pub use markets::MarketsState;
