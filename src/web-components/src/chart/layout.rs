//! Where each point of a series sits on the x axis, and what lies between the
//! points when the market was shut.
//!
//! The axis is ordinal (see [`super::math`]), so a closure takes no width at
//! all unless it is given some. Giving it the width it really covers is what
//! the ordinal axis was adopted to avoid — a night is 17 hours against a 6.5
//! hour session. So a closure is given a slot or two more than the points
//! around it: enough to read as a break in trading, not enough to crowd out
//! the trading itself.

use std::ops::Range;

use api_common::ChartData;
use api_common::Timestamp;

const DAY: u64 = 86_400;

/// How much longer than the usual spacing a gap has to be before it counts as
/// the market shutting. Loose, because a minute nobody traded in leaves the
/// same kind of hole as a night does, only smaller.
const GAP_FACTOR: f64 = 4.0;

/// Extra slots a closure is given, per square root of the slots it skipped.
/// Square rooted because the skipped time spans three orders of magnitude
/// across the ranges the page offers, and the band only has to be legible.
const GAP_SCALE: f64 = 1.7;

/// The most of the chart that closures may take between them.
const GAP_BUDGET: f64 = 0.18;

/// Under this share of the width a band is a smudge rather than a band. A
/// chart with one that thin is drawn without bands at all — a year of hourly
/// points is closed as often as it is open, and saying so 250 times says
/// nothing.
const MIN_BAND: f64 = 0.003;

/// The x axis of one series: a fraction of the width for every point, the
/// runs of points the market stayed open for, and the closures between them.
#[derive(Debug, Clone, Default)]
pub struct Layout {
    xs: Vec<f64>,
    sessions: Vec<Range<usize>>,
    closures: Vec<Closure>,
    slot: f64,
}

/// A stretch of closed market, as fractions of the chart's width.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Closure {
    pub start: f64,
    pub end: f64,
    pub kind: Closed,
}

/// What kind of closure a gap is, read off the exchange's own calendar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Closed {
    Overnight,
    Weekend,
    Holiday,
}

impl Layout {
    /// The axis for `data`, with closures worked out from the gaps in it.
    /// `utc_offset` is the exchange's offset from UTC, in seconds, which is
    /// what tells a weekend apart from a long night.
    pub fn new(data: &ChartData, utc_offset: i64) -> Self {
        let ts = data.timestamps();

        let Some(typical) = typical_spacing(ts) else {
            return Self::even(ts.len());
        };

        // Width of the interval that follows each point, in slots. Open
        // intervals are one slot, which is what makes a gapless series come
        // out evenly spaced.
        let mut widths = vec![1.0; ts.len() - 1];
        let mut kinds: Vec<Option<Closed>> = vec![None; widths.len()];
        let mut closed = 0.0;

        for i in 0..widths.len() {
            let gap = (ts[i + 1] - ts[i]) as f64;

            if gap <= GAP_FACTOR * typical as f64 {
                continue;
            }

            widths[i] = 1.0 + GAP_SCALE * (gap / typical as f64).sqrt();
            kinds[i] = Some(Closed::of(ts[i], ts[i + 1], utc_offset));
            closed += widths[i];
        }

        if closed == 0.0 {
            return Self::even(ts.len());
        }

        let open = (widths.len() - kinds.iter().filter(|k| k.is_some()).count()) as f64;
        let allowance = open * GAP_BUDGET / (1.0 - GAP_BUDGET);

        if closed > allowance {
            let scale = allowance / closed;

            for (width, _) in widths.iter_mut().zip(&kinds).filter(|(_, k)| k.is_some()) {
                *width *= scale;
            }

            closed = allowance;
        }

        let total = open + closed;
        let thinnest = widths
            .iter()
            .zip(&kinds)
            .filter(|(_, kind)| kind.is_some())
            .fold(f64::MAX, |thinnest, (&width, _)| thinnest.min(width));

        if total <= 0.0 || thinnest / total < MIN_BAND {
            return Self::even(ts.len());
        }

        let mut layout = Self {
            xs: Vec::with_capacity(ts.len()),
            sessions: Vec::new(),
            closures: Vec::new(),
            slot: 1.0 / total,
        };

        let mut at = 0.0;
        let mut session_start = 0;
        layout.xs.push(0.0);

        for (i, (width, kind)) in widths.iter().zip(&kinds).enumerate() {
            let from = at / total;
            at += width;
            layout.xs.push(at / total);

            if let Some(&kind) = kind.as_ref() {
                layout.sessions.push(session_start..i + 1);
                layout.closures.push(Closure {
                    start: from,
                    end: at / total,
                    kind,
                });
                session_start = i + 1;
            }
        }

        layout.sessions.push(session_start..ts.len());
        layout
    }

    /// `count` points spread evenly, with nothing between them.
    pub fn even(count: usize) -> Self {
        let last = count.saturating_sub(1);
        let slot = if last == 0 { 1.0 } else { 1.0 / last as f64 };

        let mut sessions = Vec::new();

        if count > 0 {
            sessions.push(0..count);
        }

        Self {
            xs: (0..count).map(|i| i as f64 * slot).collect(),
            sessions,
            closures: Vec::new(),
            slot,
        }
    }

    /// Where the `i`th point sits, as a fraction of the width.
    pub fn fraction(&self, i: usize) -> f64 {
        self.xs.get(i).copied().unwrap_or(0.0)
    }

    /// The same, scaled into a chart's user space.
    pub fn x(&self, i: usize, width: f64) -> f64 {
        self.fraction(i) * width
    }

    /// The gap between two points the market was open across, as a fraction
    /// of the width. What a bar drawn for one point has to fit inside.
    pub fn slot(&self) -> f64 {
        self.slot
    }

    /// Runs of points with no closure inside them, drawn as separate lines so
    /// that nothing is joined across hours it did not trade in.
    pub fn sessions(&self) -> &[Range<usize>] {
        &self.sessions
    }

    pub fn closures(&self) -> &[Closure] {
        &self.closures
    }
}

impl Closed {
    /// A gap is a night unless it skipped a whole day at the exchange, and a
    /// weekend if one of the days it skipped fell on one.
    fn of(from: Timestamp, to: Timestamp, utc_offset: i64) -> Self {
        let local = |ts: Timestamp| ts.saturating_add_signed(utc_offset) / DAY;
        let (first, last) = (local(from) + 1, local(to));

        if first >= last {
            return Self::Overnight;
        }

        // 1970-01-01 was a Thursday, so day 0 is weekday 3 counting from Monday.
        if (first..last).any(|day| (day + 3) % 7 >= 5) {
            Self::Weekend
        } else {
            Self::Holiday
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Overnight => "Market closed · overnight",
            Self::Weekend => "Market closed · weekend",
            Self::Holiday => "Market closed · holiday",
        }
    }
}

/// The spacing points usually sit at, which everything else is judged against.
/// The median rather than the mean: the gaps are precisely what we are trying
/// to measure, and they would drag a mean up past them.
fn typical_spacing(ts: &[Timestamp]) -> Option<u64> {
    if ts.len() < 2 {
        return None;
    }

    let mut steps = ts.windows(2).map(|w| w[1] - w[0]).collect::<Vec<_>>();
    steps.sort_unstable();

    Some(steps[steps.len() / 2]).filter(|&step| step > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: u64 = 3_600;
    /// 1970-01-05 was a Monday. Sessions open at 09:30.
    const FIRST_MONDAY: u64 = 4;

    /// `days` sessions of `len` half-hourly points, one per weekday.
    fn sessions(days: u64, len: u64) -> ChartData {
        (0..days)
            .map(|day| FIRST_MONDAY + day / 5 * 7 + day % 5)
            .flat_map(|day| (0..len).map(move |i| day * DAY + 9 * HOUR + 1_800 * (i + 1)))
            .map(|ts| (ts, 10.0))
            .collect()
    }

    #[test]
    fn spreads_a_gapless_series_evenly() {
        let layout = Layout::new(&sessions(1, 13), 0);

        assert_eq!(layout.sessions().len(), 1);
        assert_eq!(layout.sessions()[0], 0..13);
        assert!(layout.closures().is_empty());
        assert_eq!(layout.fraction(6), 0.5);
    }

    #[test]
    fn breaks_a_series_at_its_closures() {
        let layout = Layout::new(&sessions(3, 13), 0);

        assert_eq!(layout.sessions(), [0..13, 13..26, 26..39]);
        assert_eq!(layout.closures().len(), 2);
    }

    /// The whole point: the nights are visible, and they are still a sliver
    /// of the width next to the hours that traded.
    #[test]
    fn gives_a_closure_room_without_giving_it_the_chart() {
        let layout = Layout::new(&sessions(3, 13), 0);

        for closure in layout.closures() {
            let width = closure.end - closure.start;

            assert!(width > MIN_BAND, "{width} is too thin to see");
            assert!(width < 0.12, "{width} is most of a session");
        }
    }

    #[test]
    fn drops_the_bands_when_there_are_too_many_to_read() {
        let layout = Layout::new(&sessions(250, 7), 0);

        assert!(layout.closures().is_empty());
        assert_eq!(layout.sessions().len(), 1);
    }

    #[test]
    fn tells_a_weekend_from_a_night() {
        // Monday to Friday, then over the weekend into the next Monday.
        let kinds = Layout::new(&sessions(6, 13), 0)
            .closures()
            .iter()
            .map(|c| c.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            [
                Closed::Overnight,
                Closed::Overnight,
                Closed::Overnight,
                Closed::Overnight,
                Closed::Weekend,
            ]
        );
    }

    /// A gap of a day in a series that only holds one point a day is just the
    /// next day, not a closure.
    #[test]
    fn judges_a_gap_against_the_usual_spacing() {
        let daily = (0..30).map(|i| (i * DAY, 10.0)).collect::<ChartData>();

        assert!(Layout::new(&daily, 0).closures().is_empty());
    }

    #[test]
    fn survives_an_empty_series() {
        let layout = Layout::new(&ChartData::default(), 0);

        assert!(layout.sessions().is_empty());
        assert_eq!(layout.fraction(0), 0.0);
    }
}
