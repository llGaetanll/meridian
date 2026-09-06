//! The HTTP half, behind a few channels.
//!
//! What it owns is the pairing between a request and its answer — the socket
//! is axum's, and the `oneshot` a handler parks on stands in for it here.
//!
//! The id names a *request* and not a connection: keep-alive puts several down
//! one socket and HTTP/2 interleaves them. The exception is a stream, which is
//! one response the machine writes to many times; that gets a [`ConnId`] of
//! its own, because the request it arrived on is answered and gone long before
//! the last message is written.

use std::collections::HashMap;

use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::ActionRouter;
use crate::Client;
use crate::ConnId;
use crate::Event;
use crate::EventRouter;
use crate::EventSocket;
use crate::RequestId;
use crate::WebOrigins;
use crate::router_server;
use server_wire::WireSyncStatus;

/// A different port from either front-end, so all three can run at once.
const PORT: u16 = 8082;

/// How many requests may be waiting to enter the loop.
///
/// Bounded, so a flood makes the handlers wait rather than the queue grow
/// without limit. Back pressure belongs at the door.
const REQUESTS_WAITING: usize = 256;

/// What the loop sends back to the HTTP half.
///
/// Both kinds down one channel, because their order matters: a push decided in
/// the same turn as the answer that opened the stream must not overtake it.
#[derive(Debug)]
pub(crate) enum Answer {
    /// The one answer a parked request is owed.
    ToRoute {
        request_id: RequestId,
        action: ActionRouter,
    },

    /// One more message for a stream that is already open.
    ToSocket {
        conn_id: ConnId,
        status: WireSyncStatus,
    },
}

pub(crate) type ActionRx = UnboundedReceiver<Answer>;
pub(crate) type ActionTx = UnboundedSender<Answer>;

/// What the loop is told. Every producer in the runtime sends on one of
/// these: the source pools, a fetch reporting its stages, and this.
pub(crate) type EventRx = UnboundedReceiver<Event>;
pub(crate) type EventTx = UnboundedSender<Event>;

/// Where a stream says it has gone. Unbounded and its own channel: a
/// connection closes while nobody is asking anything, and the drop that
/// reports it cannot wait for room.
type CloseTx = UnboundedSender<ConnId>;
type CloseRx = UnboundedReceiver<ConnId>;

type RequestTx = mpsc::Sender<Request>;
type RequestRx = mpsc::Receiver<Request>;

/// What a handler holds, and the whole of what it can do: ask the loop
/// something, and say when a stream it was given has gone.
#[derive(Clone)]
pub(crate) struct ToLoop {
    requests: RequestTx,
    closes: CloseTx,
}

/// A request that arrived, and the one place its answer may go.
///
/// The `oneshot` is consumed by sending, so a second answer to the same
/// request cannot compile.
pub(crate) struct Request {
    client: Client,
    asked: Asked,
    response_tx: oneshot::Sender<ActionRouter>,
}

/// What a handler came for.
///
/// A stream is asked for without an id, because the id is minted here beside
/// the request's own — at the edge, like the clock and the seed.
enum Asked {
    Answer(EventRouter),
    Stream {
        token_sha256: Vec<u8>,
        updates_tx: UnboundedSender<WireSyncStatus>,
    },
}

/// Held for the life of a stream's response, so the loop hears that a
/// connection has gone rather than finding out on the next write.
///
/// Which matters twice over: the map of sinks would otherwise grow by one for
/// every connection ever made, and a caller who backgrounded ten times would
/// be refused for holding ten streams it had already dropped.
pub(crate) struct Closing {
    conn_id: ConnId,
    closes: CloseTx,
}

impl Drop for Closing {
    fn drop(&mut self) {
        let _ = self.closes.send(self.conn_id);
    }
}

/// What came of asking to watch.
pub(crate) enum Watched {
    /// The stream is open: where things stand, and everything after it.
    Open {
        status: WireSyncStatus,
        updates: UnboundedReceiver<WireSyncStatus>,
        closing: Closing,
    },

    /// It is not, and this is what the caller is told instead. `None` is an
    /// answer that never came, which is a shutdown.
    Refused(Option<ActionRouter>),
}

pub(crate) struct RuntimeRouter {
    /// Where [`RequestId`]s come from. At the edge, like the clock and the
    /// seed, so no decision depends on which number a request happened to draw.
    next_request: RequestId,

    /// Where [`ConnId`]s come from, for the same reason.
    next_conn: ConnId,

    /// Where each request's answer goes, by the id naming it. A plain map
    /// behind `&mut self`, because one task owns it.
    parked: HashMap<RequestId, oneshot::Sender<ActionRouter>>,

    /// Where each open stream's messages go.
    streams: HashMap<ConnId, UnboundedSender<WireSyncStatus>>,

    /// A stream asked for and not yet decided, by the request that asked. It
    /// moves into [`streams`](Self::streams) if the machine accepts it, and is
    /// dropped unopened if it does not.
    opening: HashMap<RequestId, (ConnId, UnboundedSender<WireSyncStatus>)>,
}

impl RuntimeRouter {
    /// Serve until the loop lets go of its end.
    ///
    /// Two tasks, because axum owns each socket for the life of its connection
    /// and must be polled independently of anything the machine is deciding.
    pub(crate) async fn run(
        mut action_rx: ActionRx,
        event_tx: EventTx,
        origins: WebOrigins,
    ) -> std::io::Result<()> {
        let (request_tx, mut request_rx): (RequestTx, RequestRx) = mpsc::channel(REQUESTS_WAITING);
        let (close_tx, mut close_rx): (CloseTx, CloseRx) = mpsc::unbounded_channel();

        // Every interface rather than loopback: a real phone reaches the Mac
        // by its address on the network.
        let listener = TcpListener::bind(("0.0.0.0", PORT)).await?;

        tracing::info!(port = PORT, "server listening");

        let to_loop = ToLoop {
            requests: request_tx,
            closes: close_tx,
        };

        // With connection info, so a line about a request can say who made it.
        let service = router_server::router(to_loop, &origins)
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
            next_conn: 0,
            parked: HashMap::new(),
            streams: HashMap::new(),
            opening: HashMap::new(),
        };

        loop {
            tokio::select! {
                Some(request) = request_rx.recv() => {
                    if event_tx.send(router.park(request)).is_err() {
                        break;
                    }
                }

                Some(conn_id) = close_rx.recv() => {
                    if event_tx.send(router.closed(conn_id)).is_err() {
                        break;
                    }
                }

                answer = action_rx.recv() => match answer {
                    Some(Answer::ToRoute { request_id, action }) => {
                        // A stream accepted for a caller that is already gone
                        // closes in the turn it opened, or the machine holds a
                        // slot for it and this holds a sink nothing reads.
                        if let Some(event) = router.answer(request_id, action)
                            && event_tx.send(event).is_err()
                        {
                            break;
                        }
                    }

                    Some(Answer::ToSocket { conn_id, status }) => {
                        router.push(conn_id, status);
                    }

                    // The loop let go, which is how a shutdown reaches here.
                    None => break,
                },

                else => break,
            }
        }

        // Let go of the door and of everything parked on us *before* waiting
        // for axum to drain, or a graceful shutdown waits forever on requests
        // nobody will ever answer. Dropping the sinks is what ends the streams,
        // which never finish on their own.
        drop(request_rx);
        drop(router);

        let _ = stop_tx.send(());

        let _ = serving.await;

        Ok(())
    }

    /// Name one request and keep hold of where its answer goes.
    fn park(&mut self, request: Request) -> Event {
        self.next_request += 1;

        let request_id = self.next_request;

        self.parked.insert(request_id, request.response_tx);

        let event = match request.asked {
            Asked::Answer(event) => event,

            Asked::Stream {
                token_sha256,
                updates_tx,
            } => {
                self.next_conn += 1;

                self.opening
                    .insert(request_id, (self.next_conn, updates_tx));

                EventRouter::SyncWatchRequested {
                    conn_id: self.next_conn,
                    token_sha256,
                }
            }
        };

        Event::FromRouter {
            request_id,
            client: request.client,
            event,
        }
    }

    /// Hand one answer to the handler parked on it.
    ///
    /// A missing entry is a caller that hung up, which is ordinary. What is
    /// not ordinary is that happening to a stream the machine has just
    /// accepted: there is no response left to drop, so nothing else would ever
    /// report the connection gone. That is the event this hands back.
    fn answer(&mut self, request_id: RequestId, action: ActionRouter) -> Option<Event> {
        // A stream the machine did not accept is never opened: dropping the
        // sink here is what ends the empty response the handler would hold.
        let opened = match self.opening.remove(&request_id) {
            Some((conn_id, updates_tx)) if matches!(action, ActionRouter::Watching { .. }) => {
                self.streams.insert(conn_id, updates_tx);

                Some(conn_id)
            }

            _ => None,
        };

        let Some(response_tx) = self.parked.remove(&request_id) else {
            tracing::debug!(request_id, "answered a route that is no longer waiting");

            return opened.map(|conn_id| self.closed(conn_id));
        };

        if response_tx.send(action).is_err() {
            tracing::debug!(request_id, "a route hung up before its answer arrived");

            return opened.map(|conn_id| self.closed(conn_id));
        }

        None
    }

    /// Write one more message to an open stream.
    fn push(&mut self, conn_id: ConnId, status: WireSyncStatus) {
        let Some(updates_tx) = self.streams.get(&conn_id) else {
            tracing::debug!(conn_id, "pushed to a stream that is not open");

            return;
        };

        // The other end is gone and the drop that says so has not been read
        // yet. Dropped here as well, so the map does not hold a dead sink
        // between now and then.
        if updates_tx.send(status).is_err() {
            self.streams.remove(&conn_id);
        }
    }

    /// A stream's response was dropped, so the connection is over.
    fn closed(&mut self, conn_id: ConnId) -> Event {
        self.streams.remove(&conn_id);

        Event::FromSocket {
            event: EventSocket::Closed { conn_id },
        }
    }
}

/// Ask the machine something on a handler's behalf, and wait for its answer.
///
/// `None` is an answer that never came — a bug rather than an outcome, since
/// every router event owes exactly one action back, except during shutdown.
pub(crate) async fn dispatch(
    to_loop: &ToLoop,
    client: Client,
    event: EventRouter,
) -> Option<ActionRouter> {
    ask(to_loop, client, Asked::Answer(event)).await
}

/// Ask to be written to for as long as this caller's sync goes on.
///
/// The same question as any other until it is answered: a caller with no
/// session is refused here, before a byte of the response has gone out.
pub(crate) async fn watch(to_loop: &ToLoop, client: Client, token_sha256: Vec<u8>) -> Watched {
    let (updates_tx, updates) = mpsc::unbounded_channel();

    let asked = Asked::Stream {
        token_sha256,
        updates_tx,
    };

    match ask(to_loop, client, asked).await {
        Some(ActionRouter::Watching { conn_id, status }) => Watched::Open {
            status,
            updates,
            closing: Closing {
                conn_id,
                closes: to_loop.closes.clone(),
            },
        },

        refused => Watched::Refused(refused),
    }
}

async fn ask(to_loop: &ToLoop, client: Client, asked: Asked) -> Option<ActionRouter> {
    let (response_tx, response_rx) = oneshot::channel();

    let request = Request {
        client,
        asked,
        response_tx,
    };

    if to_loop.requests.send(request).await.is_err() {
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
