//! Who is asking.
//!
//! Two things, because a request has two identities and a rate limit needs
//! both. The address is who opened the socket, and it is all we know about an
//! unauthenticated caller. The user is who the bearer token signs in, and it
//! is the one a caller cannot get more of by finding another machine to call
//! from — which is what makes a botnet a different problem from a loop.
//!
//! Both are minted at the edge and carried in on
//! [`Event::FromRouter`](crate::Event), the same way
//! [`RequestId`](crate::RequestId) is. The machine only ever buckets by them:
//! it never parses one, never compares two for ordering, and never decides
//! anything differently because of which address a request came from.

use std::fmt;

use uuid::Uuid;

/// A caller's address, as well as it can be known.
///
/// A string rather than an `IpAddr` because it is a bucket key and nothing
/// else — the machine hashes it and never looks inside. The parsing that
/// matters has already happened by the time one exists: `router_trace` builds
/// these, and it parses a forwarded address before believing it, so an
/// attacker cannot name their own bucket any more than they can forge a field
/// in the log line.
///
/// It carries no port. The port is a different number on every connection, so
/// a limit counted by address and port is not a limit at all.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientIp(String);

impl ClientIp {
    /// The address a request came from.
    pub fn new(address: impl Into<String>) -> Self {
        Self(address.into())
    }

    /// A caller whose address we do not know.
    ///
    /// Only reachable if the server were built without connection info, which
    /// it is not. They share one bucket rather than getting none, which is the
    /// safe direction to be wrong in: a nameless flood is still counted.
    pub fn unknown() -> Self {
        Self(String::from("unknown"))
    }
}

impl fmt::Display for ClientIp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Everything a rate limit divides callers by.
///
/// Both halves are counted, and a request has to be under both to be served.
/// One without the other leaves a hole: an address alone is spread across a
/// botnet, and a session alone is spread across sign-ups.
#[derive(Debug, Clone)]
pub struct Client {
    /// Where the request came from.
    pub ip: ClientIp,

    /// Who it is, if the route authenticated one.
    ///
    /// `None` on the routes that have no session yet — signing in, and Plaid
    /// calling us — where the address is the only thing there is to count by.
    pub user: Option<Uuid>,
}

impl Client {
    pub fn new(ip: ClientIp, user: Option<Uuid>) -> Self {
        Self { ip, user }
    }

    /// A caller we have not authenticated.
    pub fn anonymous(ip: ClientIp) -> Self {
        Self { ip, user: None }
    }
}
