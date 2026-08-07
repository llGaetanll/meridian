//! Scaling and tick helpers shared by every chart.
//!
//! The x axis is *ordinal*: points are spread evenly across the width, one
//! slot each, rather than placed by timestamp. Markets are shut most of the
//! time, and a time-proportional axis spends that width on nothing — an
//! overnight gap is 17 hours against a 6.5 hour session, so the trading gets
//! squeezed into narrow bands joined by long empty diagonals. Plotting by slot
//! drops the closed hours, which is how price charts are read everywhere, and
//! it keeps the price line aligned with the volume bars under it.

use std::ops::Range;

use api_common::ChartData;
use api_common::Price;
use api_common::Timestamp;

/// Width of the SVG user space every chart draws in. Charts stretch with
/// `preserveAspectRatio="none"`, so this is an arbitrary but shared unit.
pub const VIEW_W: f64 = 800.0;

/// Height of the SVG user space for price charts.
pub const VIEW_H: f64 = 280.0;

/// Aimed at, not guaranteed: the step is rounded to a readable number after
/// dividing by this, which usually costs a tick or two.
const TARGET_TICKS: usize = 6;

/// Maps a price onto `0..height`, flipped so that higher prices sit higher.
pub fn px_to_y(px: Price, rng: &Range<Price>, height: f64) -> f64 {
    let span = rng.end - rng.start;

    if span <= 0.0 {
        return height / 2.0;
    }

    (1.0 - (px - rng.start) / span) * height
}

/// Maps the `i`th of `count` points onto `0..width`.
pub fn slot_to_x(i: usize, count: usize, width: f64) -> f64 {
    if count <= 1 {
        return 0.0;
    }

    i as f64 / (count - 1) as f64 * width
}

/// The price range padded a little, so the line never touches the frame.
pub fn padded_price_range(data: &ChartData, pad: f64) -> Range<Price> {
    let rng = data.range_price();
    let span = rng.end - rng.start;

    if span <= 0.0 {
        return (rng.start - 1.0)..(rng.end + 1.0);
    }

    (rng.start - span * pad)..(rng.end + span * pad)
}

/// `x,y x,y …` for an SVG `points` attribute.
pub fn polyline_points(data: &ChartData, px_rng: &Range<Price>, width: f64, height: f64) -> String {
    points(data, px_rng, width, height, 1)
}

/// The same curve closed down to the baseline, for area fills.
pub fn area_points(data: &ChartData, px_rng: &Range<Price>, width: f64, height: f64) -> String {
    let line = polyline_points(data, px_rng, width, height);

    if line.is_empty() {
        return String::new();
    }

    format!("0,{height:.1} {line} {width:.1},{height:.1}")
}

/// The curve in `objectBoundingBox` units (`0..1`), for a `clipPath` that has
/// to survive being stretched by CSS. Unit space needs the extra precision.
pub fn clip_polygon_points(data: &ChartData, px_rng: &Range<Price>) -> String {
    let line = points(data, px_rng, 1.0, 1.0, 4);

    if line.is_empty() {
        return String::new();
    }

    format!("0,1 {line} 1,1")
}

fn points(
    data: &ChartData,
    px_rng: &Range<Price>,
    width: f64,
    height: f64,
    decimals: usize,
) -> String {
    let count = data.len();

    data.iter()
        .enumerate()
        .map(|(i, (_, px))| {
            format!(
                "{:.*},{:.*}",
                decimals,
                slot_to_x(i, count, width),
                decimals,
                px_to_y(px, px_rng, height)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Rounded tick values covering `rng`, at most one per ~`TARGET_TICKS`.
pub fn nice_ticks(rng: &Range<Price>) -> Vec<Price> {
    let span = rng.end - rng.start;

    if span <= 0.0 || !span.is_finite() {
        return Vec::new();
    }

    let raw_step = span / (TARGET_TICKS - 1) as f64;
    let magnitude = 10_f64.powf(raw_step.log10().floor());
    let normalized = raw_step / magnitude;

    let nice = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };

    let step = nice * magnitude;
    let mut ticks = Vec::new();
    let mut current = (rng.start / step).ceil() * step;

    while current <= rng.end + f64::EPSILON {
        ticks.push(current);
        current += step;
    }

    ticks
}

/// The timestamps sitting under `count` evenly spaced points on the x axis.
///
/// Sampled by slot, not by clock, so a label lands exactly where its point is
/// drawn — the two only agree while the market is open.
pub fn axis_stops(data: &ChartData, count: usize) -> Vec<Timestamp> {
    let timestamps = data.timestamps();

    if timestamps.is_empty() || count == 0 {
        return Vec::new();
    }

    let last = timestamps.len() - 1;

    (0..count)
        .map(|i| {
            let at = if count == 1 {
                0
            } else {
                (last * i).div_ceil(count - 1)
            };

            timestamps[at.min(last)]
        })
        .collect()
}

/// Where `ts` falls along the x axis, as a fraction of the width.
pub fn slot_fraction(data: &ChartData, ts: Timestamp) -> f64 {
    let timestamps = data.timestamps();

    if timestamps.len() <= 1 {
        return 0.0;
    }

    let last = timestamps.len() - 1;
    let at = timestamps.partition_point(|&t| t < ts).min(last);

    at as f64 / last as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Three sessions of two points each, with a long night between them —
    /// the shape that made a time-proportional axis unreadable.
    fn gapped() -> ChartData {
        const DAY: Timestamp = 86_400;

        [
            (0, 10.0),
            (3_600, 11.0),
            (DAY, 12.0),
            (DAY + 3_600, 13.0),
            (2 * DAY, 14.0),
            (2 * DAY + 3_600, 15.0),
        ]
        .into_iter()
        .collect()
    }

    #[test]
    fn spreads_points_evenly_whatever_the_gaps() {
        let data = gapped();
        let rng = 10.0..15.0;

        let xs = polyline_points(&data, &rng, 100.0, 10.0)
            .split(' ')
            .map(|point| point.split_once(',').unwrap().0.parse::<f64>().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(xs, vec![0.0, 20.0, 40.0, 60.0, 80.0, 100.0]);
    }

    #[test]
    fn labels_land_on_points() {
        let data = gapped();
        let stops = axis_stops(&data, 3);

        // First, middle and last point — not first, midnight and last.
        assert_eq!(stops, vec![0, 86_400 + 3_600, 2 * 86_400 + 3_600]);
    }

    #[test]
    fn locates_a_timestamp_by_slot() {
        let data = gapped();

        assert_eq!(slot_fraction(&data, 0), 0.0);
        assert_eq!(slot_fraction(&data, 86_400), 0.4);
        assert_eq!(slot_fraction(&data, 2 * 86_400 + 3_600), 1.0);
        // Anything past the end pins to the end.
        assert_eq!(slot_fraction(&data, 10 * 86_400), 1.0);
    }

    #[test]
    fn ticks_cover_the_range() {
        let ticks = nice_ticks(&(101.0..149.0));

        assert_eq!(ticks, vec![110.0, 120.0, 130.0, 140.0]);
    }
}
