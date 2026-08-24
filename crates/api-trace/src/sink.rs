//! Where finished lines go.
//!
//! A sink takes one whole line and is done with it. Nothing here formats, and
//! nothing here fails loudly: logging must never be the reason the app breaks.

use std::collections::VecDeque;
use std::sync::Mutex;

use crate::style::Paint;

pub trait Sink: Send + Sync + 'static {
    fn write(&self, line: &str);

    /// Whether this sink wants colour. Anything that will be read back as
    /// text later — a file, the buffer we ask users to send us — does not.
    fn paint(&self) -> Paint {
        Paint::Off
    }
}

/// The developer console: stdout on native, the browser console on the web.
///
/// The only sink a human watches live, and so the only one that is coloured.
pub struct Console {
    paint: Paint,
}

impl Console {
    pub fn new() -> Self {
        Self {
            paint: Paint::console(),
        }
    }
}

impl Sink for Console {
    #[cfg(target_arch = "wasm32")]
    fn write(&self, line: &str) {
        web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(line));
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn write(&self, line: &str) {
        println!("{line}");
    }

    fn paint(&self) -> Paint {
        self.paint
    }
}

/// The last `capacity` lines, kept so they can be handed to us after something
/// has already gone wrong. The oldest line is dropped to make room.
pub struct Buffer {
    lines: Mutex<VecDeque<String>>,
    capacity: usize,
}

impl Buffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            lines: Mutex::new(VecDeque::with_capacity(capacity.min(1024))),
            capacity,
        }
    }

    /// Everything held, oldest first, as one blob ready to be written out.
    pub fn dump(&self) -> String {
        let Ok(lines) = self.lines.lock() else {
            return String::new();
        };

        let mut out = lines.iter().cloned().collect::<Vec<_>>().join("\n");
        out.push('\n');
        out
    }
}

impl Sink for Buffer {
    fn write(&self, line: &str) {
        let Ok(mut lines) = self.lines.lock() else {
            return;
        };

        if lines.len() == self.capacity {
            lines.pop_front();
        }

        lines.push_back(line.to_string());
    }
}

/// A file on disk, appended to and flushed line by line.
///
/// Flushing every line costs us nothing at the rate this app logs, and it is
/// what makes the file worth having: a crash keeps whatever came before it.
#[cfg(not(target_arch = "wasm32"))]
pub struct File {
    handle: Mutex<std::fs::File>,
}

#[cfg(not(target_arch = "wasm32"))]
impl File {
    pub fn append(path: &std::path::Path) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let handle = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        Ok(Self {
            handle: Mutex::new(handle),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Sink for File {
    fn write(&self, line: &str) {
        use std::io::Write as _;

        let Ok(mut handle) = self.handle.lock() else {
            return;
        };

        let _ = writeln!(handle, "{line}");
        let _ = handle.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_keeps_the_most_recent_lines() {
        let buffer = Buffer::new(2);

        buffer.write("one");
        buffer.write("two");
        buffer.write("three");

        assert_eq!(buffer.dump(), "two\nthree\n");
    }
}
