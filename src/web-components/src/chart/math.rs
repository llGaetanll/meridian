//! Scaling and tick helpers shared by every chart.

use std::ops::Range;

use api_common::ChartData;
use api_common::Price;
use api_common::Timestamp;

/// Width of the SVG user space every chart draws in. Charts stretch with
/// `preserveAspectRatio="none"`, so this is an arbitrary but shared unit.
pub const VIEW_W: f64 = 800.0;

/// Height of the SVG user space for price charts.
pub const VIEW_H: f64 = 280.0;

const TARGET_TICKS: usize = 5;

/// Maps a price onto `0..height`, flipped so that higher prices sit higher.
pub fn px_to_y(px: Price, rng: &Range<Price>, height: f64) -> f64 {
    let span = rng.end - rng.start;

    if span <= 0.0 {
        return height / 2.0;
    }

    (1.0 - (px - rng.start) / span) * height
}

/// Maps a timestamp onto `0..width`.
pub fn ts_to_x(ts: Timestamp, rng: &Range<Timestamp>, width: f64) -> f64 {
    let span = (rng.end - rng.start) as f64;

    if span <= 0.0 {
        return 0.0;
    }

    ((ts - rng.start) as f64 / span) * width
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
    let ts_rng = data.range_time();

    data.iter()
        .map(|(ts, px)| {
            format!(
                "{:.1},{:.1}",
                ts_to_x(ts, &ts_rng, width),
                px_to_y(px, px_rng, height)
            )
        })
        .collect::<Vec<_>>()
        .join(" ")
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
/// to survive being stretched by CSS.
pub fn clip_polygon_points(data: &ChartData, px_rng: &Range<Price>) -> String {
    let line = polyline_points(data, px_rng, 1.0, 1.0);

    if line.is_empty() {
        return String::new();
    }

    // The polyline helper rounds to one decimal, which is far too coarse in
    // unit space, so re-render at full precision here.
    let ts_rng = data.range_time();
    let line = data
        .iter()
        .map(|(ts, px)| {
            format!(
                "{:.4},{:.4}",
                ts_to_x(ts, &ts_rng, 1.0),
                px_to_y(px, px_rng, 1.0)
            )
        })
        .collect::<Vec<_>>()
        .join(" ");

    format!("0,1 {line} 1,1")
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

/// `count` evenly spaced timestamps across the data, for the x axis.
pub fn time_axis(data: &ChartData, count: usize) -> Vec<Timestamp> {
    let rng = data.range_time();

    if data.is_empty() || count == 0 || rng.end < rng.start {
        return Vec::new();
    }

    let span = rng.end - rng.start;

    (0..count)
        .map(|i| {
            if count == 1 {
                rng.start
            } else {
                rng.start + span * i as u64 / (count - 1) as u64
            }
        })
        .collect()
}
