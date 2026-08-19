//! The wording the screens use.
//!
//! Everything here takes `now` as an argument rather than reading a clock.
//! That is the whole reason a replay of an event log lands on the same screen
//! it did the first time: `"in 3 days"` is a function of two timestamps, and
//! one of them arrived in an event.

use api_common::Timestamp;
use api_common::format::fmt_money;

const DAY: u64 = 86_400;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const MONTHS_SHORT: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];

/// `"Wed, Aug 12"` — a day heading in the ledger.
pub fn day_heading(ts: Timestamp) -> String {
    let (_, m, d) = civil(ts);

    format!("{}, {} {}", weekday(ts), MONTHS_SHORT[m as usize - 1], d)
}

/// `"August"`.
pub fn month_name(ts: Timestamp) -> String {
    MONTHS[civil(ts).1 as usize - 1].to_string()
}

/// `"2026"`.
pub fn year(ts: Timestamp) -> String {
    civil(ts).0.to_string()
}

/// `"Aug 22"`.
pub fn short_date(ts: Timestamp) -> String {
    let (_, m, d) = civil(ts);
    format!("{} {}", MONTHS_SHORT[m as usize - 1], d)
}

/// `"Thu"`.
pub fn weekday(ts: Timestamp) -> &'static str {
    // 1970-01-01 was a Thursday, which is why the table starts there.
    WEEKDAYS[((ts / DAY) % 7) as usize]
}

/// The civil date, in days since the epoch, for grouping.
pub fn day_index(ts: Timestamp) -> u64 {
    ts / DAY
}

/// `"in 3 days"`, `"today"`, `"2 days ago"`.
///
/// Whole days apart, computed on the day boundaries rather than on the raw
/// difference: a transaction at 23:00 yesterday is "yesterday", not "9 hours
/// ago", and a 25-hour gap is not "in 1 day" when it crosses two midnights.
pub fn relative_day(ts: Timestamp, now: Timestamp) -> String {
    let days = day_index(ts) as i64 - day_index(now) as i64;

    match days {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        -1 => "yesterday".to_string(),
        d if d > 1 => format!("in {d} days"),
        d => format!("{} days ago", -d),
    }
}

/// `"synced 2m ago"`, `"synced just now"`.
pub fn relative_time(ts: Timestamp, now: Timestamp) -> String {
    let secs = now.saturating_sub(ts);

    if secs < 60 {
        return "synced just now".to_string();
    }

    let minutes = secs / 60;
    if minutes < 60 {
        return format!("synced {minutes}m ago");
    }

    let hours = minutes / 60;
    if hours < 24 {
        return format!("synced {hours}h ago");
    }

    format!("synced {}d ago", hours / 24)
}

/// `"synced 2m"` — the shorter form the balances screen uses.
pub fn short_sync(ts: Timestamp, now: Timestamp) -> String {
    relative_time(ts, now).replace(" ago", "")
}

/// `"3 connections"`, `"1 connection"`.
pub fn plural(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{count} {singular}")
    } else {
        format!("{count} {plural}")
    }
}

/// Money, covered when the app is masked.
///
/// The mask is applied here rather than in the view so that no platform has to
/// remember to apply it — and so a screenshot of the FFI payload does not
/// carry the figure either.
pub fn money(amount: f64, masked: bool) -> String {
    if masked {
        // The bar the view draws is sized from the text it covers, so the
        // placeholder keeps roughly the shape of a real figure.
        return "••••••".to_string();
    }

    fmt_money(amount)
}

/// Money with an explicit `+` when it is money arriving.
pub fn money_signed(amount: f64, masked: bool) -> String {
    if masked {
        return "••••••".to_string();
    }

    if amount > 0.0 {
        format!("+{}", fmt_money(amount))
    } else {
        fmt_money(amount)
    }
}

/// The start of the current week, month or year, as a timestamp.
///
/// Exact, unlike [`crate::event::RangeUnit::seconds`]: "this month" means from
/// the first of the month, not from thirty days ago.
pub fn start_of(unit: crate::event::RangeUnit, now: Timestamp) -> Timestamp {
    let (y, m, _) = civil(now);

    match unit {
        // Weeks start on Monday. 1970-01-01 was a Thursday, so the epoch is
        // day 3 of its week.
        crate::event::RangeUnit::Week => {
            let dow = (day_index(now) + 3) % 7;
            (day_index(now) - dow) * DAY
        }
        crate::event::RangeUnit::Month => days_from_civil(y, m, 1) * DAY,
        crate::event::RangeUnit::Year => days_from_civil(y, 1, 1) * DAY,
    }
}

/// Days-to-civil-date, after Howard Hinnant's `chrono` algorithms.
fn civil(ts: Timestamp) -> (i64, u32, u32) {
    let days = (ts / DAY) as i64;
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

/// The inverse: a civil date to days since the epoch.
fn days_from_civil(y: i64, m: u32, d: u32) -> u64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;

    (era * 146_097 + doe - 719_468).max(0) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-08-19, the day the sketch is drawn against.
    const TODAY: Timestamp = 1_787_097_600;

    #[test]
    fn day_headings_match_the_sketch() {
        // 2026-08-12
        assert_eq!(day_heading(1_786_492_800), "Wed, Aug 12");
        // 2026-08-09
        assert_eq!(day_heading(1_786_233_600), "Sun, Aug 9");
    }

    #[test]
    fn months_and_years_read_plainly() {
        assert_eq!(month_name(TODAY), "August");
        assert_eq!(year(TODAY), "2026");
        assert_eq!(short_date(TODAY), "Aug 19");
    }

    /// The wording the Recurring screen leads with: Aug 22 is three days out
    /// from Aug 19.
    #[test]
    fn relative_days_count_midnights_not_hours() {
        let aug_22 = 1_787_356_800;

        assert_eq!(relative_day(aug_22, TODAY), "in 3 days");
        assert_eq!(relative_day(TODAY, TODAY), "today");
        assert_eq!(relative_day(TODAY - DAY, TODAY), "yesterday");
        assert_eq!(relative_day(TODAY + DAY, TODAY), "tomorrow");
        assert_eq!(relative_day(TODAY - 3 * DAY, TODAY), "3 days ago");
    }

    /// A late-evening transaction is "yesterday", not "9 hours ago".
    #[test]
    fn a_late_night_yesterday_is_still_yesterday() {
        let yesterday_2300 = TODAY - DAY + 23 * 3_600;

        assert_eq!(relative_day(yesterday_2300, TODAY), "yesterday");
    }

    #[test]
    fn sync_times_read_as_the_sketch_writes_them() {
        assert_eq!(relative_time(TODAY, TODAY), "synced just now");
        assert_eq!(relative_time(TODAY - 120, TODAY), "synced 2m ago");
        assert_eq!(relative_time(TODAY - 7_200, TODAY), "synced 2h ago");
        assert_eq!(short_sync(TODAY - 120, TODAY), "synced 2m");
    }

    #[test]
    fn masked_money_says_nothing() {
        assert_eq!(money(86_541.74, false), "$86,541.74");
        assert_eq!(money(86_541.74, true), "••••••");
        assert_eq!(money_signed(500.0, false), "+$500.00");
        assert_eq!(money_signed(-5.40, false), "−$5.40");
    }

    #[test]
    fn plurals_agree() {
        assert_eq!(plural(1, "connection", "connections"), "1 connection");
        assert_eq!(plural(3, "connection", "connections"), "3 connections");
        assert_eq!(plural(0, "account", "accounts"), "0 accounts");
    }

    #[test]
    fn the_civil_date_round_trips() {
        for ts in [0, TODAY, 1_786_492_800, 1_609_459_200] {
            let (y, m, d) = civil(ts);
            assert_eq!(days_from_civil(y, m, d) * DAY, ts / DAY * DAY, "{ts}");
        }
    }

    #[test]
    fn this_month_starts_on_the_first() {
        let start = start_of(crate::event::RangeUnit::Month, TODAY);

        assert_eq!(civil(start), (2026, 8, 1));
    }

    #[test]
    fn this_year_starts_in_january() {
        let start = start_of(crate::event::RangeUnit::Year, TODAY);

        assert_eq!(civil(start), (2026, 1, 1));
    }

    /// 2026-08-19 is a Wednesday, so the week began on Monday the 17th.
    #[test]
    fn a_week_starts_on_monday() {
        let start = start_of(crate::event::RangeUnit::Week, TODAY);

        assert_eq!(civil(start), (2026, 8, 17));
        assert_eq!(weekday(start), "Mon");
    }
}
