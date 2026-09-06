//! The HTTP half, behind two channels.
//!
//! It takes actions in and emits events out, and those channels are the only
//! way anything talks to it. What it owns is the pairing between a request and
//! its answer — the socket is axum's, and the `oneshot` a handler parks on is
//! what stands in for it here.
//!
//! The id names a *request* and not a connection. HTTP/1.1 keep-alive puts
//! several down one socket and HTTP/2 interleaves them, so a connection is not
//! the thing being answered.

use std::collections::HashMap;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::ActionRouter;
use crate::Client;
use crate::EventRouter;
use crate::RequestId;
use crate::WebOrigins;
use crate::router_server;

/// A different port from either front-end, so all three can run at once.
const PORT: u16 = 8082;

/// How many requests may be waiting to enter the loop.
///
/// Bounded, so a flood makes the handlers wait rather than the queue grow
/// without limit. This is the one place in the server where back pressure
/// belongs: at the door.
const REQUESTS_WAITING: usize = 256;

/// What the loop sends back: one answer, on the id that asked.
pub(crate) type ActionRx = UnboundedReceiver<(RequestId, ActionRouter)>;
pub(crate) type ActionTx = UnboundedSender<(RequestId, ActionRouter)>;

/// What the loop is told: one request, and who made it.
pub(crate) type EventRx = UnboundedReceiver<(RequestId, Client, EventRouter)>;
pub(crate) type EventTx = UnboundedSender<(RequestId, Client, EventRouter)>;

/// What a handler holds, and the whole of what it can do.
pub(crate) type RequestTx = mpsc::Sender<Request>;

type RequestRx = mpsc::Receiver<Request>;

/// A request that arrived, and the one place its answer may go.
///
/// The `oneshot` is consumed by sending, so a second answer to the same
/// request cannot compile.
pub(crate) struct Request {
    client: Client,
    event: EventRouter,
    response_tx: oneshot::Sender<ActionRouter>,
}

pub(crate) struct RuntimeRouter {
    /// Where [`RequestId`]s come from. At the edge, like the clock and the
    /// seed — the machine only relays them, so no decision depends on which
    /// number a request happened to draw.
    next_request: RequestId,

    /// Where each request's answer goes, by the id naming it.
    ///
    /// A plain map behind `&mut self`, because one task owns it. It is the
    /// whole of the request-to-answer pairing.
    parked: HashMap<RequestId, oneshot::Sender<ActionRouter>>,
}

impl RuntimeRouter {
    /// Serve until the loop lets go of its end.
    ///
    /// Two tasks: axum on one, and this one pairing requests with the answers
    /// that come back. They are separate because axum owns each socket for the
    /// life of its connection and must be polled independently of anything the
    /// machine is deciding.
    pub(crate) async fn run(
        mut action_rx: ActionRx,
        event_tx: EventTx,
        origins: WebOrigins,
    ) -> std::io::Result<()> {
        let (request_tx, mut request_rx): (RequestTx, RequestRx) = mpsc::channel(REQUESTS_WAITING);

        // Every interface rather than loopback, because a phone on the LAN is
        // one of the clients. The simulator shares the Mac's loopback and would
        // be happy either way; a real device has to reach the Mac by its
        // address on the network, and nothing answers there if we bind
        // loopback. This is a development server on a home network.
        let listener = TcpListener::bind(("0.0.0.0", PORT)).await?;

        tracing::info!(port = PORT, "server listening");

        // With connection info, so every line about a request can say which
        // client made it. A dev server is talked to by the web front-end on
        // loopback and a phone on the LAN at the same time, and untangling
        // their requests is most of what the log is for.
        let service = router_server::router(request_tx, &origins)
            .into_make_service_with_connect_info::<std::net::SocketAddr>();

        let (stop_tx, stop_rx) = oneshot::channel();

        let serving = tokio::spawn(async move {
            let served = axum::serve(listener, service)
                .with_graceful_shutdown(async move {
                    let _ = stop_rx.await;
                })
                .await;

            if let Err(e) = served {
                tracing::error!(error = %e, "the http server stopped");
            }
        });

        let mut router = Self {
            next_request: 0,
            parked: HashMap::new(),
        };

        loop {
            tokio::select! {
                Some(request) = request_rx.recv() => {
                    if event_tx.send(router.park(request)).is_err() {
                        break;
                    }
                }

                answer = action_rx.recv() => match answer {
                    Some((request_id, action)) => router.answer(request_id, action),

                    // The loop let go, which is how a shutdown reaches here.
                    None => break,
                },

                else => break,
            }
        }

        // Let go of the door and of everything parked on us *before* waiting
        // for axum to drain. A handler that arrives now is refused, and one
        // already parked is released with nothing — either is an answer. Held
        // open, they would be requests nobody will ever answer, and a graceful
        // shutdown that waits for them never finishes.
        drop(request_rx);
        drop(router);

        let _ = stop_tx.send(());

        let _ = serving.await;

        Ok(())
    }

    /// Name one request and keep hold of where its answer goes.
    fn park(&mut self, request: Request) -> (RequestId, Client, EventRouter) {
        self.next_request += 1;

        self.parked.insert(self.next_request, request.response_tx);

        (self.next_request, request.client, request.event)
    }

    /// Hand one answer to the handler parked on it.
    ///
    /// A missing entry is a caller that hung up, which is ordinary.
    fn answer(&mut self, request_id: RequestId, action: ActionRouter) {
        let Some(response_tx) = self.parked.remove(&request_id) else {
            tracing::debug!(request_id, "answered a route that is no longer waiting");

            return;
        };

        if response_tx.send(action).is_err() {
            tracing::debug!(request_id, "a route hung up before its answer arrived");
        }
    }
}

/// Ask the machine something on a handler's behalf, and wait for its answer.
///
/// The one thing a handler can do. It parks rather than spins: `.await` yields
/// the task, so a request in flight costs a map entry and nothing else.
///
/// `None` is an answer that never came. That is a bug rather than an outcome —
/// every router event owes exactly one action back — except during shutdown,
/// when the loop has let go and there is nobody left to ask.
pub(crate) async fn dispatch(
    requests: &RequestTx,
    client: Client,
    event: EventRouter,
) -> Option<ActionRouter> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = Request {
        client,
        event,
        response_tx,
    };

    if requests.send(request).await.is_err() {
        tracing::error!("a route asked something after the loop had stopped");

        return None;
    }

    match response_rx.await {
        Ok(action) => Some(action),

        Err(_) => {
            tracing::error!("a route asked something nothing answered");

            None
        }
    }
}
