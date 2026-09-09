//! What one Link session came to, as a poll found it.

/// One session's answer inside an [`EventSourcePlaid::LinksPolled`](crate::EventSourcePlaid).
///
/// `finished` and `public_tokens` are separate facts. A session with no token
/// that has not finished is somebody still typing their password; a session
/// with no token that *has* finished is somebody who closed the sheet, which
/// is an ordinary outcome and not a failure.
#[derive(Debug)]
pub struct LinkPoll {
    pub link_token: String,
    pub finished: bool,
    pub public_tokens: Vec<String>,
}
