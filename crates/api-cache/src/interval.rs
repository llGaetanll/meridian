use api_common::Timestamp;

/// How much time one bar of a series stands for.
///
/// Only fixed-length intervals are here. Months and quarters are not a number
/// of seconds, so bars of that size cannot be laid on a grid, merged, or
/// folded into one another the way the rest of this crate assumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Interval {
    Min1,
    Min2,
    Min5,
    Min15,
    Min30,
    Hour1,
    Day1,
    Week1,
}

impl Interval {
    pub const ALL: [Self; 8] = [
        Self::Min1,
        Self::Min2,
        Self::Min5,
        Self::Min15,
        Self::Min30,
        Self::Hour1,
        Self::Day1,
        Self::Week1,
    ];

    pub fn seconds(self) -> u64 {
        match self {
            Self::Min1 => 60,
            Self::Min2 => 120,
            Self::Min5 => 300,
            Self::Min15 => 900,
            Self::Min30 => 1_800,
            Self::Hour1 => 3_600,
            Self::Day1 => 86_400,
            Self::Week1 => 604_800,
        }
    }

    /// The start of the bar `ts` falls in.
    ///
    /// Bars of a given size sit on one grid whoever sent them, which is what
    /// lets two fetches of the same interval be merged point for point. The
    /// grid is anchored at the epoch, so daily bars turn over at UTC midnight
    /// rather than at any exchange's.
    pub fn floor(self, ts: Timestamp) -> Timestamp {
        // Floor to the interval boundary this timestamp falls in.
        Timestamp::from_secs(ts.as_secs() - ts.as_secs() % self.seconds())
    }

    /// The last bar that has finished forming as of `now`.
    ///
    /// A bar still being traded into is not a fact yet: it is left outside
    /// coverage so that it is fetched again next time, while everything
    /// behind it is settled and never fetched twice.
    pub fn last_closed(self, now: Timestamp) -> Timestamp {
        self.floor(now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_onto_the_grid() {
        assert_eq!(
            Interval::Min5.floor(Timestamp::from_secs(1_000_000_301)),
            Timestamp::from_secs(1_000_000_200)
        );
        assert_eq!(
            Interval::Min5.floor(Timestamp::from_secs(1_000_000_200)),
            Timestamp::from_secs(1_000_000_200)
        );
        assert_eq!(
            Interval::Day1.floor(Timestamp::from_secs(1_786_127_325)),
            Timestamp::from_secs(1_786_060_800)
        );
    }

    /// Two windows landing on the same bar must floor to the same timestamp,
    /// or merging them would double up the point.
    #[test]
    fn agrees_across_windows() {
        let (a, b) = (
            Timestamp::from_secs(1_786_127_325),
            Timestamp::from_secs(1_786_127_399),
        );

        assert_eq!(Interval::Min5.floor(a), Interval::Min5.floor(b));
    }
}
