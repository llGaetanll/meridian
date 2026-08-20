//! A point in time.
//!
//! A newtype rather than a bare `u64`, because a plain integer is assignable
//! from anything: a duration, a count, an index, a millisecond reading. All of
//! those compile, and all of them are wrong in a way no test notices until a
//! date renders as 1970.
//!
//! There is deliberately no `Timestamp::now()`. Reading a clock is IO, and the
//! state machines are built so that time arrives as an argument — see
//! `docs/ARCHITECTURE.md`. Whatever reads the real clock lives at the edge, in
//! a runtime, and hands the value in.

use std::fmt::Display;
use std::iter::Sum;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Sub;
use std::ops::SubAssign;
use std::time::Duration;

/// Seconds since the unix epoch.
///
/// The field is `pub` only because `uniffi::custom_newtype!` needs to reach it
/// to lower the value across FFI. Nothing should construct one that way —
/// [`Timestamp::from_secs`] says what the number means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Timestamp {
    pub t: u64,
}

impl From<u64> for Timestamp {
    fn from(t: u64) -> Self {
        Timestamp { t }
    }
}

impl From<Timestamp> for u64 {
    fn from(ts: Timestamp) -> u64 {
        ts.t
    }
}

impl Timestamp {
    /// The epoch itself, 1970-01-01T00:00:00Z.
    pub const EPOCH: Timestamp = Timestamp { t: 0 };

    /// The furthest representable point. Useful as the identity when folding a
    /// minimum over a series.
    pub const MAX: Timestamp = Timestamp { t: u64::MAX };

    pub const MIN: Timestamp = Timestamp::EPOCH;

    pub const fn from_secs(t: u64) -> Self {
        Self { t }
    }

    /// The raw count of seconds.
    ///
    /// Needed at the edges — serialising, formatting, indexing a cache — and
    /// nowhere in between. If you are reaching for this to do arithmetic, use
    /// the operators instead.
    pub const fn as_secs(&self) -> u64 {
        self.t
    }

    /// How long after `earlier` this is. Saturates at zero rather than
    /// wrapping, so an out-of-order pair gives `0` and not 584 billion years.
    pub const fn since(&self, earlier: Timestamp) -> Duration {
        Duration::from_secs(self.t.saturating_sub(earlier.t))
    }

    /// This many seconds earlier, floored at the epoch.
    pub const fn saturating_sub_secs(&self, secs: u64) -> Self {
        Self {
            t: self.t.saturating_sub(secs),
        }
    }

    /// Shift by a signed number of seconds, floored at the epoch.
    ///
    /// For timezone offsets, which are the one place a shift is genuinely
    /// signed rather than known to go one way.
    pub const fn saturating_add_signed(&self, secs: i64) -> Self {
        Self {
            t: self.t.saturating_add_signed(secs),
        }
    }

    pub const fn saturating_add_secs(&self, secs: u64) -> Self {
        Self {
            t: self.t.saturating_add(secs),
        }
    }

    /// Which day this falls on, as a count of days since the epoch.
    ///
    /// The unit both date grouping and "how many days apart" are computed in,
    /// so it lives here rather than being re-derived per caller.
    pub const fn day_index(&self) -> u64 {
        self.t / 86_400
    }

    /// Midnight UTC on the day this falls on.
    pub const fn start_of_day(&self) -> Self {
        Self {
            t: self.day_index() * 86_400,
        }
    }

    /// Whole days from `self` to `other`, negative if `other` is earlier.
    ///
    /// Counted on day boundaries rather than by dividing the difference: a
    /// transaction at 23:00 yesterday is one day ago, not zero, and a 25-hour
    /// gap that crosses two midnights is two.
    pub const fn days_until(&self, other: Timestamp) -> i64 {
        other.day_index() as i64 - self.day_index() as i64
    }
}

/// Parses the same text `Display` writes: a bare count of seconds. The ledger's
/// text format relies on the pair round-tripping.
impl std::str::FromStr for Timestamp {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse().map(Timestamp::from_secs)
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.t)
    }
}

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: Duration) -> Timestamp {
        Timestamp {
            t: self.t.saturating_add(rhs.as_secs()),
        }
    }
}

impl AddAssign<Duration> for Timestamp {
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, rhs: Duration) -> Timestamp {
        Timestamp {
            t: self.t.saturating_sub(rhs.as_secs()),
        }
    }
}

impl SubAssign<Duration> for Timestamp {
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

/// The gap between two points is a `Duration`, not another `Timestamp` — which
/// is most of the reason this type exists.
impl Sub<Timestamp> for Timestamp {
    type Output = Duration;

    fn sub(self, rhs: Timestamp) -> Duration {
        self.since(rhs)
    }
}

impl Sum for Timestamp {
    fn sum<I: Iterator<Item = Timestamp>>(iter: I) -> Self {
        Timestamp {
            t: iter.map(|ts| ts.t).sum(),
        }
    }
}

impl serde::Serialize for Timestamp {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.t)
    }
}

impl<'de> serde::Deserialize<'de> for Timestamp {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        u64::deserialize(deserializer).map(Timestamp::from_secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-08-19 00:00:00 UTC.
    const TODAY: Timestamp = Timestamp::from_secs(1_787_097_600);

    const DAY: Duration = Duration::from_secs(86_400);

    #[test]
    fn adding_a_duration_moves_forward() {
        assert_eq!(TODAY + DAY, Timestamp::from_secs(1_787_184_000));
        assert_eq!(TODAY - DAY, Timestamp::from_secs(1_787_011_200));
    }

    /// The gap between two points is a duration, not a point.
    #[test]
    fn subtracting_two_points_gives_a_duration() {
        assert_eq!((TODAY + DAY) - TODAY, DAY);
    }

    /// An out-of-order pair gives zero rather than wrapping to an absurd
    /// number, which is the failure mode a bare `u64` has.
    #[test]
    fn a_backwards_gap_saturates() {
        assert_eq!(TODAY.since(TODAY + DAY), Duration::ZERO);
        assert_eq!(Timestamp::EPOCH - DAY, Timestamp::EPOCH);
    }

    #[test]
    fn a_signed_shift_goes_both_ways_and_floors() {
        assert_eq!(
            TODAY.saturating_add_signed(3_600),
            TODAY + Duration::from_secs(3_600)
        );
        assert_eq!(
            TODAY.saturating_add_signed(-3_600),
            TODAY - Duration::from_secs(3_600)
        );
        assert_eq!(Timestamp::EPOCH.saturating_add_signed(-1), Timestamp::EPOCH);
    }

    #[test]
    fn days_are_counted_on_boundaries() {
        // 23:00 the day before is one day ago, not zero.
        let late_yesterday = TODAY - Duration::from_secs(3_600);

        assert_eq!(late_yesterday.days_until(TODAY), 1);
        assert_eq!(TODAY.days_until(late_yesterday), -1);
        assert_eq!(TODAY.days_until(TODAY), 0);
    }

    #[test]
    fn start_of_day_floors_to_midnight() {
        let midday = TODAY + Duration::from_secs(12 * 3_600);

        assert_eq!(midday.start_of_day(), TODAY);
        assert_eq!(TODAY.start_of_day(), TODAY);
    }

    #[test]
    fn it_round_trips_through_text() {
        let text = TODAY.to_string();

        assert_eq!(text.parse::<Timestamp>().expect("parses"), TODAY);
    }

    #[test]
    fn it_round_trips_through_json() {
        let json = serde_json::to_string(&TODAY).expect("serialises");

        assert_eq!(json, "1787097600");
        assert_eq!(
            serde_json::from_str::<Timestamp>(&json).expect("parses"),
            TODAY
        );
    }
}
