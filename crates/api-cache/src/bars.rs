use std::ops::Range;

use api_common::ChartData;
use api_common::Price;
use api_common::Timestamp;

/// A run of bars, held column by column and sorted by time.
///
/// This is what the cache stores: whatever a source sent, at the interval it
/// sent it, with nothing thrown away. Anything a chart wants — a close series,
/// a volume series, a thinner version of either — is derived from it on the
/// way out. Squashing on the way in would make a year's worth of bars useless
/// for drawing a month of them.
#[derive(Debug, Clone, Default)]
pub struct Bars {
    ts: Vec<Timestamp>,
    open: Vec<Price>,
    close: Vec<Price>,
    volume: Vec<Price>,
}

impl Bars {
    pub fn push(&mut self, ts: Timestamp, open: Price, close: Price, volume: Price) {
        self.ts.push(ts);
        self.open.push(open);
        self.close.push(close);
        self.volume.push(volume);
    }

    pub fn len(&self) -> usize {
        self.ts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ts.is_empty()
    }

    pub fn timestamps(&self) -> &[Timestamp] {
        &self.ts
    }

    /// Closing prices, which is what a price chart draws.
    pub fn closes(&self) -> ChartData {
        self.ts
            .iter()
            .copied()
            .zip(self.close.iter().copied())
            .collect()
    }

    /// Volume for exactly the bars [`Self::closes`] returns, so the two stay
    /// index-aligned however many sessions the window straddles.
    pub fn volumes(&self) -> ChartData {
        self.ts
            .iter()
            .copied()
            .zip(self.volume.iter().copied())
            .collect()
    }

    /// The opening price of the first bar — the open of the stretch on show.
    pub fn first_open(&self) -> Option<Price> {
        self.open.first().copied()
    }

    pub fn last_close(&self) -> Option<Price> {
        self.close.last().copied()
    }

    pub fn range_time(&self) -> Option<Range<Timestamp>> {
        Some(*self.ts.first()?..*self.ts.last()?)
    }

    /// The bars falling inside `window`, half-open as everywhere else.
    pub fn window(&self, window: &Range<Timestamp>) -> Self {
        let start = self.ts.partition_point(|&ts| ts < window.start);
        let end = self.ts.partition_point(|&ts| ts < window.end);

        Self {
            ts: self.ts[start..end].to_vec(),
            open: self.open[start..end].to_vec(),
            close: self.close[start..end].to_vec(),
            volume: self.volume[start..end].to_vec(),
        }
    }

    /// Fold `other` in, keeping both sorted and free of repeats.
    ///
    /// Where the two hold the same bar, `other` wins: it is the newer word on
    /// it, and the bar at the live edge of a series is revised until it
    /// closes.
    pub fn merge(&mut self, other: Self) {
        if other.is_empty() {
            return;
        }

        if self.is_empty() {
            *self = other;
            return;
        }

        let mut merged = Self::default();
        let (mut i, mut j) = (0, 0);

        while i < self.len() || j < other.len() {
            let take_other = match (self.ts.get(i), other.ts.get(j)) {
                (Some(&mine), Some(&theirs)) => {
                    // Equal timestamps: take theirs and drop mine.
                    if mine == theirs {
                        i += 1;
                    }

                    theirs <= mine
                }
                (None, Some(_)) => true,
                _ => false,
            };

            let (from, at) = if take_other {
                (&other, &mut j)
            } else {
                (&*self, &mut i)
            };

            merged.push(
                from.ts[*at],
                from.open[*at],
                from.close[*at],
                from.volume[*at],
            );
            *at += 1;
        }

        *self = merged;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bars(points: &[(Timestamp, Price)]) -> Bars {
        let mut bars = Bars::default();

        for &(ts, px) in points {
            bars.push(ts, px, px, px);
        }

        bars
    }

    fn closes(bars: &Bars) -> Vec<(Timestamp, Price)> {
        bars.closes().iter().collect()
    }

    fn merged(left: &[(Timestamp, Price)], right: &[(Timestamp, Price)]) -> Bars {
        let mut out = bars(left);
        out.merge(bars(right));

        out
    }

    #[test]
    fn merges_two_runs_in_order() {
        let out = merged(&[(10, 1.0), (30, 3.0)], &[(20, 2.0), (40, 4.0)]);

        assert_eq!(closes(&out), [(10, 1.0), (20, 2.0), (30, 3.0), (40, 4.0)]);
    }

    #[test]
    fn appends_a_later_run() {
        let out = merged(&[(10, 1.0), (20, 2.0)], &[(30, 3.0)]);

        assert_eq!(closes(&out), [(10, 1.0), (20, 2.0), (30, 3.0)]);
    }

    /// The live bar is revised until it closes, so a refetch of it wins.
    #[test]
    fn the_newer_bar_wins() {
        let out = merged(&[(10, 1.0), (20, 2.0)], &[(20, 9.0), (30, 3.0)]);

        assert_eq!(closes(&out), [(10, 1.0), (20, 9.0), (30, 3.0)]);
    }

    #[test]
    fn merging_with_nothing_keeps_everything() {
        assert_eq!(closes(&merged(&[], &[(10, 1.0)])), [(10, 1.0)]);
        assert_eq!(closes(&merged(&[(10, 1.0)], &[])), [(10, 1.0)]);
    }

    #[test]
    fn takes_the_bars_inside_the_window() {
        let out = bars(&[(10, 1.0), (20, 2.0), (30, 3.0), (40, 4.0)]);

        assert_eq!(closes(&out.window(&(20..40))), [(20, 2.0), (30, 3.0)]);
        assert_eq!(closes(&out.window(&(0..100))), closes(&out));
        assert!(out.window(&(50..60)).is_empty());
    }

    /// The charts pair a volume bar with its price point by index.
    #[test]
    fn prices_and_volume_stay_aligned() {
        let out = merged(&[(10, 1.0), (30, 3.0)], &[(20, 2.0)]);

        assert_eq!(out.closes().len(), out.volumes().len());
        assert_eq!(out.closes().timestamps(), out.volumes().timestamps());
    }
}
