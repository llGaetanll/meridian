//! The session token, as the browser keeps it.
//!
//! `localStorage` and not a cookie: the token is sent as a bearer header, and
//! the server sets nothing for a browser to keep. Nothing here decides when
//! to read, keep or forget it — the machine says, with `LoadSession`,
//! `SaveSession` and `ForgetSession`.

use web_sys::Storage;

/// Where the browser keeps it.
const TOKEN_KEY: &str = "meridian.token";

/// The token the browser is holding, if it is holding one.
pub fn load() -> Option<String> {
    storage()?
        .get_item(TOKEN_KEY)
        .ok()?
        .filter(|token| !token.is_empty())
}

/// Keep this token, so that the next visit opens signed in.
pub fn save(token: &str) {
    let saved = storage().map(|storage| storage.set_item(TOKEN_KEY, token));

    if !matches!(saved, Some(Ok(()))) {
        tracing::error!("the session could not be kept; the next visit opens on first run");
    }
}

/// Throw it away, so that the next visit opens on first run.
pub fn forget() {
    let forgotten = storage().map(|storage| storage.remove_item(TOKEN_KEY));

    if !matches!(forgotten, Some(Ok(()))) {
        tracing::error!("the session could not be forgotten; the next visit opens signed in");
    }
}

/// `None` where the browser has none to give: storage turned off, or a
/// private window that refuses it.
fn storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok()?
}
