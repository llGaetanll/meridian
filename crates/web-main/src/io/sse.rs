//! Server-Sent Events, split by hand.

/// The sync's statuses, read out of a body that arrives in pieces.
///
/// A browser's `EventSource` cannot send an `Authorization` header, so the
/// stream is read as an ordinary response and split here, which is most of
/// what the format is. A chunk can end anywhere — mid-line, mid-character —
/// so bytes are held until their line is complete.
#[derive(Debug, Default)]
pub struct Sse {
    /// The start of a line whose end has not arrived yet.
    partial: Vec<u8>,

    /// The kind of message the last `event:` line named. Kept across lines
    /// because it arrives before the data it is about.
    kind: String,
}

impl Sse {
    /// Take in the next piece of the body, and push every status it
    /// completed.
    pub fn feed(&mut self, chunk: &[u8], statuses: &mut Vec<String>) {
        for &byte in chunk {
            if byte != b'\n' {
                self.partial.push(byte);

                continue;
            }

            let line = std::mem::take(&mut self.partial);
            let line = String::from_utf8_lossy(&line);
            let line = line.strip_suffix('\r').unwrap_or(&line);

            self.line(line, statuses);
        }
    }

    fn line(&mut self, line: &str, statuses: &mut Vec<String>) {
        // A blank line ends a message, and the kind goes with it.
        if line.is_empty() {
            self.kind.clear();

            return;
        }

        // The keep-alive, so that a quiet connection is not reaped.
        if line.starts_with(':') {
            return;
        }

        if let Some(kind) = field("event", line) {
            self.kind = kind.to_string();

            return;
        }

        let Some(json) = field("data", line) else {
            return;
        };

        // Dispatched on the name, so a kind added later is skipped rather
        // than parsed as a status it cannot be read as.
        if self.kind.is_empty() || self.kind == "sync" {
            statuses.push(json.to_string());
        }

        self.kind.clear();
    }
}

/// The value of one field, or `None` if the line is not that field. The space
/// after the colon is optional and is dropped.
fn field<'a>(name: &str, line: &'a str) -> Option<&'a str> {
    let value = line.strip_prefix(name)?.strip_prefix(':')?;

    Some(value.strip_prefix(' ').unwrap_or(value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(chunks: &[&str]) -> Vec<String> {
        let mut sse = Sse::default();
        let mut statuses = Vec::new();

        for chunk in chunks {
            sse.feed(chunk.as_bytes(), &mut statuses);
        }

        statuses
    }

    #[test]
    fn a_status_is_its_data_line() {
        assert_eq!(
            read(&["event: sync\ndata: {\"stage\":\"idle\"}\n\n"]),
            ["{\"stage\":\"idle\"}"],
        );
    }

    #[test]
    fn a_line_cut_between_chunks_is_read_whole() {
        assert_eq!(read(&["data: {\"st", "age\":1}\n\n"]), ["{\"stage\":1}"]);
    }

    #[test]
    fn a_character_cut_between_chunks_is_read_whole() {
        let bytes = "data: Café\n".as_bytes();
        let mut sse = Sse::default();
        let mut statuses = Vec::new();

        // `é` is the tenth and eleventh bytes.
        sse.feed(&bytes[..10], &mut statuses);
        sse.feed(&bytes[10..], &mut statuses);

        assert_eq!(statuses, ["Café"]);
    }

    #[test]
    fn keep_alives_and_other_kinds_are_skipped() {
        assert_eq!(
            read(&[": keep-alive\n\nevent: other\ndata: no\n\ndata: yes\n\n"]),
            ["yes"],
        );
    }

    #[test]
    fn carriage_returns_end_lines_too() {
        assert_eq!(read(&["event:sync\r\ndata:{}\r\n\r\n"]), ["{}"]);
    }

    #[test]
    fn a_line_with_no_end_yet_is_held() {
        assert!(read(&["data: {}"]).is_empty());
    }
}
