//! Logging for the whole app.
//!
//! Two rules hold everywhere:
//!
//! - **One event is one line.** Logs are read with `grep` and sent to us in a
//!   chat window; a stack of wrapped lines is neither.
//! - **Every call to an external API is logged**, request and response both.
//!   A bug we cannot reproduce is a bug we have to fix from the log alone, and
//!   we may not get a second run to collect it.
//!
//! Lines look like this:
//!
//! ```text
//! 2026-08-07T18:42:03.401Z 6e917a6 INFO  api quote{symbol=SPY range=1D} -> request id=1 source=yahoo method=GET url=/v8/finance/chart/SPY?range=1d&interval=1m
//! 2026-08-07T18:42:03.813Z 6e917a6 INFO  api quote{symbol=SPY range=1D} <- response id=1 source=yahoo status=200 bytes=91233 ms=412
//! ```
//!
//! `6e917a6` is the commit the build came from, and `quote{…}` is the span the
//! event happened in, so a response can be read back to the thing the user did.
//!
//! Every line this app writes is listed in `docs/LOGGING.md`, and changing one
//! means changing that list too.
//!
//! # Getting the logs off a user's machine
//!
//! Native builds write straight to the file named in [`Config::file`]. The web
//! build cannot: a browser tab has no file to append to. It keeps the last
//! [`Config::buffer`] lines in memory instead, and [`download`] saves them —
//! reachable from the devtools console as `wasmBindings.meridian_logs()`.

mod call;
mod clock;
mod layer;
mod line;
mod sink;

use std::sync::Arc;
use std::sync::OnceLock;

use tracing::level_filters::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub use call::ApiCall;
pub use call::TARGET as API;
pub use tracing::Level;

use crate::layer::OneLine;
use crate::sink::Buffer;
use crate::sink::Console;
use crate::sink::Sink;

/// The commit this build came from, on every line. A log is read long after
/// the build that wrote it, often against a working tree that has moved on.
pub const SHA: &str = env!("MERIDIAN_SHA");

static BUFFER: OnceLock<Arc<Buffer>> = OnceLock::new();

pub struct Config {
    /// Nothing below this level is recorded at all.
    pub level: Level,
    /// How many lines to keep in memory for [`dump`]. Zero turns it off.
    pub buffer: usize,
    /// A file to append lines to. Created if missing, along with its parent.
    #[cfg(not(target_arch = "wasm32"))]
    pub file: Option<std::path::PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            level: Level::INFO,
            // A few thousand lines is minutes of use at the rate this app
            // logs, and a rounding error next to the wasm bundle.
            buffer: 4096,
            #[cfg(not(target_arch = "wasm32"))]
            file: Some(std::path::PathBuf::from("logs/meridian.log")),
        }
    }
}

/// Start logging. Call this first, before anything worth logging happens.
///
/// Calling it twice is a no-op rather than a failure: the second subscriber
/// would be ignored anyway, and losing the app to a logging mistake is not a
/// trade worth making.
pub fn init(config: Config) {
    let mut sinks: Vec<Box<dyn Sink>> = vec![Box::new(Console)];

    if config.buffer > 0 {
        let buffer = BUFFER
            .get_or_init(|| Arc::new(Buffer::new(config.buffer)))
            .clone();

        sinks.push(Box::new(BufferSink(buffer)));
    }

    #[cfg(not(target_arch = "wasm32"))]
    let file_error = match config.file.as_deref().map(sink::File::append) {
        Some(Ok(file)) => {
            sinks.push(Box::new(file));
            None
        }
        Some(Err(e)) => {
            // `Some(Err(_))` only arises when `config.file` was `Some`, so the
            // path is always there.
            let Some(path) = config.file.as_ref() else {
                unreachable!("the error came from opening this very path")
            };

            Some(format!("{}: {e}", path.display()))
        }
        None => None,
    };

    let installed = tracing_subscriber::registry()
        .with(LevelFilter::from_level(config.level))
        .with(OneLine::new(sinks))
        .try_init()
        .is_ok();

    if !installed {
        return;
    }

    #[cfg(not(target_arch = "wasm32"))]
    if let Some(e) = file_error {
        // The console still has everything; say what we lost and carry on.
        tracing::warn!(target: "trace", error = %e, "log file unavailable");
    }

    panic_hook();
}

/// Panics go through the log too, so the last line before a crash is in the
/// same file as everything that led to it.
fn panic_hook() {
    let previous = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();

        tracing::error!(target: "panic", at = location, message = %info, "panicked");

        previous(info);
    }));
}

/// Everything still held in memory, oldest line first.
pub fn dump() -> String {
    BUFFER.get().map(|b| b.dump()).unwrap_or_default()
}

struct BufferSink(Arc<Buffer>);

impl Sink for BufferSink {
    fn write(&self, line: &str) {
        self.0.write(line);
    }
}

/// Save the buffered log as a file the user can send us.
///
/// Exported to JS as `meridian_logs`, so it can be run from the devtools
/// console without the app needing a button for it.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(js_name = meridian_logs)]
pub fn download() {
    use wasm_bindgen::JsCast;
    use wasm_bindgen::JsValue;

    let text = dump();

    let Some(document) = web_sys::window().and_then(|w| w.document()) else {
        return;
    };

    let parts = js_sys::Array::of1(&JsValue::from_str(&text));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("text/plain");

    let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &options) else {
        return;
    };

    let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) else {
        return;
    };

    let anchor = document
        .create_element("a")
        .ok()
        .and_then(|e| e.dyn_into::<web_sys::HtmlAnchorElement>().ok());

    if let Some(anchor) = anchor {
        anchor.set_href(&url);
        anchor.set_download(&format!("meridian-{}.log", clock::now()));
        anchor.click();
    }

    let _ = web_sys::Url::revoke_object_url(&url);
}
