//! Bars, kept so they are fetched once.
//!
//! Requests are windows of time, not the ranges an API happens to offer, and
//! what has been fetched is tracked as coverage rather than guessed at from
//! the points held. Between them, those two make the obvious savings fall out:
//! a window already held costs nothing, a window overlapping one costs the
//! part that hangs off, and a window spanning two costs nothing at all.
//!
//! Nothing in here knows the time, the platform, or any particular API. A
//! [`Source`] is the whole of what the outside world looks like from here.

mod bars;
mod coverage;
mod interval;
mod source;
mod store;

pub use bars::Bars;
pub use coverage::Coverage;
pub use interval::Interval;
pub use source::Fetched;
pub use source::Source;
pub use store::Store;
