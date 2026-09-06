//! Every request a client makes of us, logged as a pair.
//!
//! A layer rather than a line in each handler, because half these answers — a
//! 401 from the extractor, a 404, a body that would not deserialize — are
//! given before any handler runs.
//!
//! Deployed, Caddy terminates TLS and the socket is always the proxy, so the
//! caller has to be worked out from what the proxy says. [`client_of`] is
//! where that happens.

use std::net::IpAddr;
use std::net::SocketAddr;

use api_trace::Served;
use axum::body::HttpBody;
use axum::extract::ConnectInfo;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

use crate::ClientIp;

/// Log a request in and its answer out.
pub(crate) async fn trace(mut request: Request, next: Next) -> Response {
    let client = client_of(&request);

    // Worked out once and handed on, so the handler that rate limits a request
    // and the line that logs it divide callers the same way.
    request.extensions_mut().insert(client.clone());

    let method = request.method().clone();

    let url = request
        .uri()
        .path_and_query()
        .map(ToString::to_string)
        .unwrap_or_else(|| request.uri().path().to_string());

    let served = Served::received(module_path!(), client.to_string(), method, url);

    let response = next.run(request).await;

    // Everything this server answers with is already a buffer, so the hint is
    // exact. A streaming body says `None` rather than lying, which is why this
    // is not an `unwrap_or(0)`.
    let bytes = response.body().size_hint().exact();

    served.answered(response.status().as_u16(), bytes);

    response
}

/// Who asked, as well as it can be known.
///
/// The socket peer, unless that peer is a proxy of ours, in which case what the
/// proxy says.
///
/// **The address only, never the port.** A caller gets a fresh port on every
/// connection, so a budget counted by address and port is a budget per request.
pub(crate) fn client_of(request: &Request) -> ClientIp {
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| *addr);

    // Unnamed only if the server was built without connection info, which it
    // is not. They share one bucket rather than escaping the limit.
    let Some(peer) = peer else {
        return ClientIp::unknown();
    };

    if is_local(peer.ip())
        && let Some(forwarded) = forwarded_for(request)
    {
        return ClientIp::new(forwarded);
    }

    ClientIp::new(peer.ip().to_string())
}

/// What the proxy says the caller's address is, if it says anything usable.
///
/// The **last** entry, not the first. Caddy appends the address it actually saw
/// to whatever the caller had already put in the header, so everything left of
/// the rightmost is a claim the caller made about itself.
///
/// Parsed rather than passed through: unparsed, a caller could put spaces and
/// `=` into `client=` and forge fields on the log line that carries it.
fn forwarded_for(request: &Request) -> Option<String> {
    let header = request.headers().get("x-forwarded-for")?.to_str().ok()?;

    let last: IpAddr = header.rsplit(',').next()?.trim().parse().ok()?;

    Some(last.to_string())
}

/// Whether an address is on the near side of the proxy.
///
/// Loopback and the private ranges. It fails safe: a request from a public
/// address has its `X-Forwarded-For` ignored, so exposing the server directly
/// one day does not silently turn a spoofable header into the thing rate limits
/// are counted by.
fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),

        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            // A v4 peer on a dual-stack socket, judged as what it is.
            Some(v4) => is_local(IpAddr::V4(v4)),

            // fc00::/7 unique local and fe80::/10 link local, spelled out
            // because `Ipv6Addr`'s own predicates for them are still unstable.
            None => {
                let head = v6.segments()[0];

                v6.is_loopback() || head & 0xfe00 == 0xfc00 || head & 0xffc0 == 0xfe80
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A request from `peer`, carrying `forwarded` as `X-Forwarded-For`.
    fn request(peer: &str, forwarded: Option<&str>) -> Request {
        let mut builder = Request::builder().uri("/snapshot");

        if let Some(forwarded) = forwarded {
            builder = builder.header("x-forwarded-for", forwarded);
        }

        let mut request = builder
            .body(axum::body::Body::empty())
            .expect("a literal request");

        let peer: SocketAddr = peer.parse().expect("a literal address");
        request.extensions_mut().insert(ConnectInfo(peer));

        request
    }

    /// The address, and not the port it happened to arrive on. A budget
    /// counted by address and port is a budget per request.
    #[test]
    fn names_the_socket_when_nothing_is_forwarded() {
        assert_eq!(
            client_of(&request("192.168.1.24:53112", None)),
            ClientIp::new("192.168.1.24"),
        );
    }

    #[test]
    fn names_the_forwarded_client_behind_the_proxy() {
        let request = request("172.18.0.4:38326", Some("203.0.113.7"));

        assert_eq!(client_of(&request), ClientIp::new("203.0.113.7"));
    }

    /// The caller sent a header of its own and Caddy appended what it saw. The
    /// left half is the caller's claim about itself and is not to be believed.
    #[test]
    fn believes_the_proxy_over_the_caller() {
        let request = request("172.18.0.4:38326", Some("1.2.3.4, 203.0.113.7"));

        assert_eq!(client_of(&request), ClientIp::new("203.0.113.7"));
    }

    /// A public peer is not a proxy of ours, so its header is only a claim.
    #[test]
    fn ignores_a_forwarded_header_from_outside() {
        let request = request("203.0.113.7:41000", Some("10.0.0.1"));

        assert_eq!(client_of(&request), ClientIp::new("203.0.113.7"));
    }

    /// Anything that is not an address is dropped rather than logged, so a
    /// caller cannot write its own fields into the line that names it.
    #[test]
    fn falls_back_when_the_header_is_not_an_address() {
        let request = request("172.18.0.4:38326", Some("id=1 client=someone-else"));

        assert_eq!(client_of(&request), ClientIp::new("172.18.0.4"));
    }

    /// A v6 caller is counted by the block delegated to them, not by the
    /// address they happened to send from — that one is theirs to change, and
    /// there are eighteen quintillion of them.
    #[test]
    fn buckets_a_forwarded_v6_caller_by_its_prefix() {
        let forwarded = Some("2001:db8:1234:5678:aaaa:bbbb:cccc:dddd");
        let request = request("172.18.0.4:38326", forwarded);

        assert_eq!(client_of(&request).to_string(), "2001:db8:1234:5678::/64");
    }

    #[test]
    fn reads_a_v4_peer_mapped_onto_a_v6_socket() {
        let request = request("[::ffff:172.18.0.4]:38326", Some("203.0.113.7"));

        assert_eq!(client_of(&request), ClientIp::new("203.0.113.7"));
    }
}
