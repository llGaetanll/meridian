//! Display helpers shared by every front-end (web, desktop, mobile).

use crate::Price;
use crate::Quantity;
use crate::Timestamp;

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `1234.5` -> `"1,234.50"`
pub fn fmt_decimal(value: f64, decimals: usize) -> String {
    let negative = value < 0.0;
    let text = format!("{:.*}", decimals, value.abs());

    let (int_part, frac_part) = match text.split_once('.') {
        Some((i, f)) => (i.to_string(), Some(f.to_string())),
        None => (text, None),
    };

    let mut grouped = String::new();
    for (i, c) in int_part.chars().enumerate() {
        if i > 0 && (int_part.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }

    let mut out = String::new();
    if negative {
        out.push('-');
    }
    out.push_str(&grouped);
    if let Some(frac) = frac_part {
        out.push('.');
        out.push_str(&frac);
    }

    out
}

/// `1234.5` -> `"$1,234.50"`
pub fn fmt_money(px: Price) -> String {
    let text = fmt_decimal(px.abs(), money_decimals(px));

    if px < 0.0 {
        format!("−${text}")
    } else {
        format!("${text}")
    }
}

/// Money with an explicit sign, e.g. `"+$600.85"`.
pub fn fmt_money_signed(px: Price) -> String {
    if px < 0.0 {
        fmt_money(px)
    } else {
        format!("+{}", fmt_money(px))
    }
}

/// Axis-sized money: `13000.0` -> `"$13k"`, `649.74` -> `"$650"`.
pub fn fmt_money_short(px: Price) -> String {
    let abs = px.abs();
    let text = if abs >= 1_000_000_000.0 {
        format!("{:.1}B", abs / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.1}M", abs / 1_000_000.0)
    } else if abs >= 10_000.0 {
        format!("{:.0}k", abs / 1_000.0)
    } else if abs >= 100.0 {
        format!("{:.0}", abs)
    } else if abs >= 1.0 {
        format!("{:.2}", abs)
    } else {
        format!("{:.4}", abs)
    };

    if px < 0.0 {
        format!("−${text}")
    } else {
        format!("${text}")
    }
}

/// `71_400_000.0` -> `"71.4M"`
pub fn fmt_compact(value: f64) -> String {
    let abs = value.abs();
    let text = if abs >= 1_000_000_000.0 {
        format!("{:.1}B", abs / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.1}M", abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}K", abs / 1_000.0)
    } else {
        fmt_decimal(abs, 0)
    };

    if value < 0.0 {
        format!("−{text}")
    } else {
        text
    }
}

/// A held quantity, e.g. `"38.000"` or `"0.2410"`.
pub fn fmt_qty(qty: Quantity) -> String {
    let decimals = if qty.abs() >= 1.0 { 3 } else { 4 };
    fmt_decimal(qty, decimals)
}

/// `0.184` -> `"+18.40%"`
pub fn fmt_pct(ratio: f64) -> String {
    let text = format!("{:.2}%", (ratio * 100.0).abs());

    if ratio < 0.0 {
        format!("−{text}")
    } else {
        format!("+{text}")
    }
}

/// `"Aug 02 2026"`
pub fn fmt_date(ts: Timestamp) -> String {
    let (y, m, d) = civil_from_ts(ts);
    format!("{} {:02} {}", MONTHS[m as usize - 1], d, y)
}

/// `"Aug 02"`
pub fn fmt_month_day(ts: Timestamp) -> String {
    let (_, m, d) = civil_from_ts(ts);
    format!("{} {:02}", MONTHS[m as usize - 1], d)
}

/// `"Aug 2026"`
pub fn fmt_month_year(ts: Timestamp) -> String {
    let (y, m, _) = civil_from_ts(ts);
    format!("{} {}", MONTHS[m as usize - 1], y)
}

/// `"2026"`
pub fn fmt_year(ts: Timestamp) -> String {
    civil_from_ts(ts).0.to_string()
}

/// `"9:30"`
pub fn fmt_time_of_day(ts: Timestamp) -> String {
    let secs = ts % 86_400;
    format!("{}:{:02}", secs / 3600, (secs % 3600) / 60)
}

/// Sub-unit prices (FX rates, small caps) need more places to say anything.
fn money_decimals(px: Price) -> usize {
    if px == 0.0 || px.abs() >= 1.0 { 2 } else { 4 }
}

/// Days-to-civil-date, after Howard Hinnant's `chrono` algorithms. Keeps
/// `api-common` dependency free.
fn civil_from_ts(ts: Timestamp) -> (i64, u32, u32) {
    let days = (ts / 86_400) as i64;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;

    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(fmt_decimal(1234.5, 2), "1,234.50");
        assert_eq!(fmt_decimal(-1_234_567.0, 0), "-1,234,567");
        assert_eq!(fmt_decimal(12.0, 3), "12.000");
    }

    #[test]
    fn formats_money() {
        assert_eq!(fmt_money(649.74), "$649.74");
        assert_eq!(fmt_money_signed(600.85), "+$600.85");
        assert_eq!(fmt_money_short(13_000.0), "$13k");
    }

    #[test]
    fn formats_dates() {
        // 2026-08-02 00:00:00 UTC
        assert_eq!(fmt_date(1_785_628_800), "Aug 02 2026");
        assert_eq!(fmt_month_year(1_785_628_800), "Aug 2026");
    }
}
