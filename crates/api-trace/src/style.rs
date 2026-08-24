//! ANSI colour, for the sinks that can show it.
//!
//! A log file and the in-memory buffer both get read as text later on, so
//! escape codes in them are noise. Only the console asks for colour, and only
//! when it is a terminal that will render it — see [`Paint::console`].

use tracing::Level;

const RESET: &str = "\x1b[0m";

pub const DIM: &str = "\x1b[2m";
pub const BOLD: &str = "\x1b[1m";
pub const TARGET: &str = "\x1b[36m";

/// Whether a rendered line carries escape codes.
///
/// Lines are rendered once per variant in use, not once per sink, so the
/// console and the file do not pay for each other.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    Off,
    Ansi,
}

impl Paint {
    /// What the console should use: colour only for a terminal that asked for
    /// it. A redirected stdout is a file by another name, and `NO_COLOR` is
    /// the user saying no.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn console() -> Self {
        use std::io::IsTerminal;

        let wanted = std::env::var_os("NO_COLOR").is_none();

        if wanted && std::io::stdout().is_terminal() {
            Self::Ansi
        } else {
            Self::Off
        }
    }

    /// The browser console prints escape codes rather than acting on them.
    #[cfg(target_arch = "wasm32")]
    pub fn console() -> Self {
        Self::Off
    }

    pub fn write(self, out: &mut String, code: &str, text: &str) {
        if text.is_empty() {
            return;
        }

        self.start(out, code);
        out.push_str(text);
        self.end(out);
    }

    /// For a run of text that is built up in pieces, so it takes one pair of
    /// escape codes rather than one per piece.
    pub fn start(self, out: &mut String, code: &str) {
        if self == Self::Ansi {
            out.push_str(code);
        }
    }

    pub fn end(self, out: &mut String) {
        if self == Self::Ansi {
            out.push_str(RESET);
        }
    }
}

/// One colour per level, so the eye finds the trouble without reading.
pub fn level(level: &Level) -> &'static str {
    match *level {
        Level::ERROR => "\x1b[1;31m",
        Level::WARN => "\x1b[33m",
        Level::INFO => "\x1b[32m",
        Level::DEBUG => "\x1b[34m",
        Level::TRACE => "\x1b[35m",
    }
}
