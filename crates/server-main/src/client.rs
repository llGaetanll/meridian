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
use std::net::IpAddr;
use std::net::Ipv6Addr;

use uuid::Uuid;

/// A caller, named as narrowly as one honestly can be.
///
/// A string rather than an `IpAddr` because it is a bucket key and nothing
/// else — the machine hashes it and never looks inside. Whether an address is
/// to be *believed* is still settled at the edge, in `router_trace`, which
/// parses a forwarded one before trusting it, so an attacker cannot name their
/// own bucket any more than they can forge a field in the log line.
///
/// It carries no port. The port is a different number on every connection, so
/// a limit counted by address and port is not a limit at all.
///
/// **An IPv6 caller is held as their /64, not their address.** v6 is not
/// handed out an address at a time: an ISP delegates a /64 to a customer, and
/// the eighteen quintillion addresses inside it are free to move between — the
/// operating system already rotates through them on a timer, with no help from
/// anyone. Counted whole, a v6 caller has a fresh budget per request and the
/// limit is decoration; counted as the /64, they have one budget, which is
/// what a v4 caller gets for their one address. Anything wider — the /56 or
/// /48 an ISP may delegate to a site — would start charging one customer for
/// what their neighbour did.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClientIp(String);

impl ClientIp {
    /// The bucket the address a request came from belongs to.
    ///
    /// Anything that is not an address is kept as it stands, which is
    /// [`unknown`](Self::unknown) and nothing else: the edge parses before it
    /// builds one of these.
    pub fn new(address: impl Into<String>) -> Self {
        let address = address.into();

        match address.parse::<IpAddr>() {
            Ok(ip) => Self(bucket_of(ip)),
            Err(_) => Self(address),
        }
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

/// The smallest block of addresses a caller cannot get more of for nothing.
///
/// One address over v4, where a second one has to be bought. The /64 over v6,
/// where the rest of the address is the caller's to choose.
fn bucket_of(ip: IpAddr) -> String {
    let v6 = match ip {
        IpAddr::V4(v4) => return v4.to_string(),
        IpAddr::V6(v6) => v6,
    };

    // A v4 peer arriving on a dual-stack socket, judged as what it is. Masking
    // it instead would be a disaster in the other direction: every v4 address
    // in existence maps into `::ffff:0:0/96`, so a /64 of that is one bucket
    // holding the whole v4 internet.
    if let Some(v4) = v6.to_ipv4_mapped() {
        return v4.to_string();
    }

    let [a, b, c, d, ..] = v6.segments();

    format!("{}/64", Ipv6Addr::new(a, b, c, d, 0, 0, 0, 0))
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A v4 address is one caller's whole allocation, so it is kept whole.
    #[test]
    fn a_v4_address_is_its_own_bucket() {
        assert_eq!(ClientIp::new("203.0.113.7").to_string(), "203.0.113.7");
    }

    /// The bug this exists for: the host half of a v6 address is the caller's
    /// to pick, so counting it hands out a budget per request.
    #[test]
    fn v6_addresses_in_one_prefix_share_a_bucket() {
        assert_eq!(
            ClientIp::new("2001:db8:1234:5678::1"),
            ClientIp::new("2001:db8:1234:5678:aaaa:bbbb:cccc:dddd"),
        );
    }

    #[test]
    fn v6_addresses_in_different_prefixes_do_not() {
        assert_ne!(
            ClientIp::new("2001:db8:1234:5678::1"),
            ClientIp::new("2001:db8:1234:9999::1"),
        );
    }

    /// Named as the prefix it is, so a log line cannot be read as an address
    /// nobody ever called from.
    #[test]
    fn a_v6_bucket_says_it_is_a_prefix() {
        assert_eq!(
            ClientIp::new("2001:db8:1234:5678:aaaa:bbbb:cccc:dddd").to_string(),
            "2001:db8:1234:5678::/64",
        );
    }

    /// The trap in the other direction. Every v4 address maps into
    /// `::ffff:0:0/96`, so masking these to a /64 would put the entire v4
    /// internet in one bucket.
    #[test]
    fn a_v4_peer_on_a_v6_socket_is_not_masked() {
        assert_eq!(
            ClientIp::new("::ffff:203.0.113.7").to_string(),
            "203.0.113.7"
        );

        assert_ne!(
            ClientIp::new("::ffff:203.0.113.7"),
            ClientIp::new("::ffff:198.51.100.4"),
        );
    }

    /// And it is the same caller either way round, or a dual-stack socket
    /// would buy one machine two budgets.
    #[test]
    fn a_v4_peer_is_one_caller_however_it_arrives() {
        assert_eq!(
            ClientIp::new("::ffff:203.0.113.7"),
            ClientIp::new("203.0.113.7"),
        );
    }

    #[test]
    fn an_unknown_caller_is_left_alone() {
        assert_eq!(ClientIp::unknown().to_string(), "unknown");
    }
}
