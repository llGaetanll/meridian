//! What a request came to, when it did not come to an answer.

use crate::Unpaid;

/// Why a source could not be read.
///
/// The runtime classifies what it got; the machine decides what to do about it
/// and words what the user reads. A sentence built at the edge is a sentence
/// no `eval!` test can reach, and one each platform writes its own version of.
///
/// Two of these are not failures to report but places to go: a session the
/// server does not know is first run, and a session that has not paid is the
/// paywall.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum LoadFailure {
    /// Nothing came back. No signal, aeroplane mode, or a server that is not
    /// running.
    Offline,

    /// The server does not know this session. **401.**
    SignedOut,

    /// The session is good, and the app is not paid for. **402.**
    Unpaid { reason: Unpaid },

    /// Asked for too much, too fast. **429.**
    TooMany,

    /// Asked to subscribe by somebody the server already has a subscription
    /// for, or is starting one for. **409.**
    ///
    /// Not a failure either: there is nothing to buy, and whether the app
    /// opens is what asking for a snapshot answers.
    Subscribed,

    /// Any other answer, which is the server's problem rather than this
    /// session's.
    Server { status: u16 },

    /// An answer arrived and could not be read.
    Unreadable,
}
