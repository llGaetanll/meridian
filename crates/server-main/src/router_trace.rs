//! Every request a client makes of us, logged as a pair.
//!
//! The other half of `docs/LOGGING.md`'s rule that everything crossing the
//! network is logged. Calls we make out to Plaid, Yahoo and the rest have been
//! logged since the beginning; calls the phone makes *in* were not, which left
//! the log with no record of the traffic the app is actually built on. "The
//! screen was empty" could not be read back to whether the phone asked, what
//! it asked for, or what it was told.
//!
//! A layer rather than a line in each handler, because a handler that forgets
//! is exactly the gap this closes, and because half these answers — a 401 from
//! the extractor, a 404, a body that would not deserialize — are given before
//! any handler runs.

use std::net::SocketAddr;

use api_trace::Served;
use axum::body::HttpBody;
use axum::extract::ConnectInfo;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::Response;

/// Log a request in and its answer out.
pub(crate) async fn trace(request: Request, next: Next) -> Response {
    // Whoever the socket says asked. Unnamed only if the server was built
    // without connection info, which it is not.
    let client = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.to_string())
        .unwrap_or_else(|| String::from("unknown"));

    let method = request.method().clone();

    // Path and query, which is what was asked for. The host is this process.
    let url = request
        .uri()
        .path_and_query()
        .map(ToString::to_string)
        .unwrap_or_else(|| request.uri().path().to_string());

    let served = Served::received(module_path!(), client, method, url);

    let response = next.run(request).await;

    // Known without touching the body: everything this server answers with is
    // already a buffer by the time it gets here, so the hint is exact and
    // reading it costs nothing. Streaming one would say `None` rather than
    // lie, which is why this is not an `unwrap_or(0)`.
    let bytes = response.body().size_hint().exact();

    served.answered(response.status().as_u16(), bytes);

    response
}
