//! The ways a trip to Yahoo comes back with nothing.

/// Why a fetch served no bars. Three different sentences, because the machine
/// does three different things about them: back off, stop asking, or note it
/// and move on.
#[derive(Debug)]
pub enum YahooFault {
    /// Yahoo answered 429. The one signal there is about the rate limit,
    /// which Yahoo never states.
    RateLimited,

    /// Yahoo understood the request and would not serve it — an interval it
    /// will not go that far back for, or a symbol it does not know. Asking
    /// again buys the same answer.
    Refused { code: String, description: String },

    /// Everything else: the network, a timeout, a shape this build did not
    /// expect.
    Failed { message: String },
}
