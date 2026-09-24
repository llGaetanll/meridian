//! How much history the chart's range buttons ask for.

use std::ops::Range;

use api_cache::Interval;
use api_common::Timestamp;

/// How much history to ask for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartRange {
    Day,
    Week,
    Month,
    Year,
    FiveYears,
    Max,
}

const DAY: u64 = 86_400;

impl ChartRange {
    pub const ALL: [Self; 6] = [
        Self::Day,
        Self::Week,
        Self::Month,
        Self::Year,
        Self::FiveYears,
        Self::Max,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::Day => "1D",
            Self::Week => "1W",
            Self::Month => "1M",
            Self::Year => "1Y",
            Self::FiveYears => "5Y",
            Self::Max => "All",
        }
    }

    /// The stretch of time the range covers, ending now.
    ///
    /// Windows are absolute rather than relative — "the last month" is a
    /// different month every day, and two of them cannot be merged, while
    /// two spans of seconds always can.
    pub fn window(&self, now: Timestamp) -> Range<Timestamp> {
        // It ends at the last bar to have closed rather than at this instant.
        // The bar still forming is revised until it closes, so asking for it
        // is asking for something that cannot be settled — and a window that
        // can never be settled is a window fetched again every time it is
        // looked at, which is the one thing worth avoiding here. What it costs
        // is a chart that stops a bar short of now: a minute on a day, a week
        // on thirty years.
        let end = self.interval().last_closed(now);

        let back = match self {
            Self::Day => DAY,
            Self::Week => 7 * DAY,
            Self::Month => 30 * DAY,
            Self::Year => 365 * DAY,
            Self::FiveYears => 5 * 365 * DAY,
            // The source clamps to the first trade it has, whatever we ask.
            Self::Max => return Timestamp::EPOCH..end,
        };

        now.saturating_sub_secs(back)..end
    }

    /// The finest interval that covers the range without asking for points so
    /// far past what a chart can show that we are only paying for bytes.
    pub fn interval(&self) -> Interval {
        match self {
            Self::Day => Interval::Min1,
            Self::Week => Interval::Min2,
            Self::Month => Interval::Min5,
            Self::Year => Interval::Hour1,
            Self::FiveYears => Interval::Day1,
            Self::Max => Interval::Week1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: Timestamp = Timestamp::from_secs(1_786_000_000);

    /// A window ends at the last bar to have *closed*, not at this instant —
    /// see `window`. `NOW` is not minute-aligned, so the end is a little
    /// before it, and asserting on `NOW` itself would be asserting the bug
    /// that comment exists to prevent.
    #[test]
    fn windows_end_at_the_last_closed_bar_and_reach_back() {
        let day = ChartRange::Day.window(NOW);

        assert_eq!(day.start, NOW.saturating_sub_secs(DAY));
        assert_eq!(day.end, ChartRange::Day.interval().last_closed(NOW));
        assert!(day.end <= NOW);

        assert_eq!(ChartRange::Max.window(NOW).start, Timestamp::EPOCH);
    }
}
