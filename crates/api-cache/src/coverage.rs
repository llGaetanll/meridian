use std::ops::Range;

use api_common::Timestamp;

/// The stretches of time a series is known over, as opposed to the points it
/// holds.
///
/// The two are not the same thing and cannot be inferred from one another: a
/// market is shut more often than it is open, so a window with no points in it
/// is just as likely to be fully known as never asked for. Only this tells
/// them apart, and it is what stops the same empty weekend being fetched
/// again every time it is looked at.
///
/// Spans are half-open, sorted, and never touch: adding one that meets or
/// overlaps another swallows both into one.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    spans: Vec<Range<Timestamp>>,
}

impl Coverage {
    /// Record `span` as known. Empty spans are no news and are dropped.
    pub fn add(&mut self, span: Range<Timestamp>) {
        if span.start >= span.end {
            return;
        }

        let mut merged = span;
        let mut spans = Vec::with_capacity(self.spans.len() + 1);

        for span in self.spans.drain(..) {
            // Touching counts as overlapping: [0, 5) and [5, 9) is [0, 9).
            if span.end < merged.start || span.start > merged.end {
                spans.push(span);
            } else {
                merged.start = merged.start.min(span.start);
                merged.end = merged.end.max(span.end);
            }
        }

        spans.push(merged);
        spans.sort_by_key(|span| span.start);

        self.spans = spans;
    }

    /// The parts of `want` not known yet, in order. Empty means the window can
    /// be answered without asking anyone.
    pub fn missing(&self, want: &Range<Timestamp>) -> Vec<Range<Timestamp>> {
        if want.start >= want.end {
            return Vec::new();
        }

        let mut missing = Vec::new();
        let mut at = want.start;

        for span in &self.spans {
            if span.end <= at {
                continue;
            }

            if span.start >= want.end {
                break;
            }

            if span.start > at {
                missing.push(at..span.start);
            }

            at = span.end;

            if at >= want.end {
                break;
            }
        }

        if at < want.end {
            missing.push(at..want.end);
        }

        missing
    }

    pub fn covers(&self, want: &Range<Timestamp>) -> bool {
        self.missing(want).is_empty()
    }

    pub fn spans(&self) -> &[Range<Timestamp>] {
        &self.spans
    }
}

#[cfg(test)]
// These tests compare a list of spans against a literal list of ranges, which
// is exactly what it looks like. Clippy reads `[a..b]` as a botched attempt at
// a range of values instead.
#[allow(clippy::single_range_in_vec_init)]
mod tests {
    use super::*;

    fn coverage(spans: &[Range<Timestamp>]) -> Coverage {
        let mut coverage = Coverage::default();

        for span in spans {
            coverage.add(span.clone());
        }

        coverage
    }

    /// The whole point: two windows fetched back to back answer the window
    /// that spans them both, with no third request.
    #[test]
    fn abutting_spans_answer_the_whole() {
        let coverage = coverage(&[10..20, 20..30]);

        assert_eq!(coverage.spans(), [10..30]);
        assert!(coverage.covers(&(10..30)));
        assert!(coverage.missing(&(10..30)).is_empty());
    }

    #[test]
    fn overlapping_spans_collapse() {
        assert_eq!(coverage(&[10..25, 20..30]).spans(), [10..30]);
        assert_eq!(coverage(&[10..30, 15..20]).spans(), [10..30]);
    }

    #[test]
    fn a_span_can_bridge_two_others() {
        assert_eq!(coverage(&[0..10, 20..30, 8..22]).spans(), [0..30]);
    }

    #[test]
    fn disjoint_spans_stay_apart() {
        assert_eq!(coverage(&[0..10, 20..30]).spans(), [0..10, 20..30]);
    }

    #[test]
    fn reports_the_holes() {
        let coverage = coverage(&[10..20, 30..40]);

        assert_eq!(coverage.missing(&(0..50)), [0..10, 20..30, 40..50]);
        assert_eq!(coverage.missing(&(15..35)), [20..30]);
        assert_eq!(coverage.missing(&(20..30)), [20..30]);
        assert_eq!(coverage.missing(&(12..18)), []);
    }

    /// A window running past what is known asks only for the tail — which is
    /// what makes a chart of the last day cost one bar, not a day of them.
    #[test]
    fn asks_only_for_the_tail() {
        assert_eq!(coverage(&[0..100]).missing(&(50..120)), [100..120]);
    }

    #[test]
    fn empty_windows_want_nothing() {
        assert!(coverage(&[]).missing(&(10..10)).is_empty());
        assert!(coverage(&[]).covers(&(10..10)));
    }

    #[test]
    fn knowing_nothing_wants_everything() {
        assert_eq!(coverage(&[]).missing(&(0..50)), [0..50]);
    }
}
