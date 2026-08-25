//! State in, finished screens out.
//!
//! Every string a platform prints is produced here. That is more code in Rust
//! than a view-model of raw values would be, and it is the trade `docs/FFI.md`
//! argues for: the type surface grows a little, and the number of places that
//! can format a date differently stays at one.

use std::collections::BTreeMap;

use api_common::format::fmt_money;
use api_ledger::Account;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Stream;

use api_common::Timestamp;

use crate::RangeSelection;
use crate::Screen;
use crate::StateApp;
use crate::ViewAccountRow;
use crate::ViewBalances;
use crate::ViewBreakdownSlice;
use crate::ViewCalendar;
use crate::ViewCalendarDay;
use crate::ViewCategoryChip;
use crate::ViewConnection;
use crate::ViewDaySection;
use crate::ViewDock;
use crate::ViewDockItem;
use crate::ViewFilter;
use crate::ViewLedger;
use crate::ViewMonthSection;
use crate::ViewRange;
use crate::ViewRangeKind;
use crate::ViewRecurring;
use crate::ViewScreen;
use crate::ViewStreamGroup;
use crate::ViewStreamRow;
use crate::ViewTxRow;
use crate::fmt;

/// Seconds in a day, for stepping the calendar a cell at a time.
const DAY: u64 = 86_400;

/// Rows in the calendar. Six covers every month whatever day it starts on,
/// which is what keeps the grid one height all year.
const WEEKS: u64 = 6;

/// The calendar's column headings, starting on Monday.
const WEEKDAY_INITIALS: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];

/// A stream landing within this many days reads as a heads-up.
const SOON_DAYS: u64 = 7;

/// How many selected categories a shut filter names before it says "+2".
const SUMMARY_CHIPS: usize = 2;

pub fn screen(state: &StateApp) -> ViewScreen {
    match state.screen() {
        Screen::Ledger => ViewScreen::Ledger {
            view: ledger(state),
        },
        Screen::Recurring => ViewScreen::Recurring {
            view: recurring(state),
        },
        Screen::Balances => ViewScreen::Balances {
            view: balances(state),
        },
    }
}

pub fn dock(current: Screen) -> ViewDock {
    let item = |screen: Screen, label: &str, icon: &str| ViewDockItem {
        screen,
        label: label.to_string(),
        icon: icon.to_string(),
        selected: screen == current,
    };

    ViewDock {
        items: vec![
            item(Screen::Ledger, "Ledger", "ledger"),
            item(Screen::Recurring, "Recurring", "recurring"),
            item(Screen::Balances, "Balances", "balances"),
        ],
    }
}

fn ledger(state: &StateApp) -> ViewLedger {
    let entries = state.filtered();

    ViewLedger {
        months: group_by_month(&entries),
        filter: filter_view(state),
        empty: empty_message(state, entries.len()),
    }
}

/// What the list says when it has nothing to show.
///
/// Three different nothings, and they need different words: still loading,
/// loaded but the filter excludes everything, and loaded with an empty ledger.
/// A single "No transactions" would be wrong in two of the three.
fn empty_message(state: &StateApp, shown: usize) -> Option<String> {
    if shown > 0 {
        return None;
    }

    if let Some(error) = state.load_failed()
        && !state.loaded()
    {
        return Some(error.to_string());
    }

    if !state.loaded() {
        return Some("Loading…".to_string());
    }

    if state.ledger().is_active() && !state.entries().is_empty() {
        return Some("Nothing matches this filter.".to_string());
    }

    Some("No transactions yet.".to_string())
}

/// The rows, cut into months and then days.
///
/// One pass: the entries arrive newest-first and stay in that order, so a
/// change of day or month is simply a change of key from the row before.
fn group_by_month(entries: &[&Entry]) -> Vec<ViewMonthSection> {
    let mut months: Vec<ViewMonthSection> = Vec::new();

    for entry in entries {
        let ts = entry.ts();
        let (name, year, heading) = (fmt::month_name(ts), fmt::year(ts), fmt::day_heading(ts));

        let month = match months.last_mut() {
            Some(month) if month.name == name && month.year == year => month,
            _ => {
                months.push(ViewMonthSection {
                    name,
                    year,
                    days: Vec::new(),
                });

                let Some(month) = months.last_mut() else {
                    unreachable!("a month was just pushed")
                };

                month
            }
        };

        let day = match month.days.last_mut() {
            Some(day) if day.heading == heading => day,
            _ => {
                month.days.push(ViewDaySection {
                    heading,
                    rows: Vec::new(),
                });

                let Some(day) = month.days.last_mut() else {
                    unreachable!("a day was just pushed")
                };

                day
            }
        };

        day.rows.push(tx_row(entry, false));
    }

    debug_assert!(
        months.iter().all(|m| !m.days.is_empty()),
        "a month section with no days should not exist",
    );

    months
}

/// Money is never masked on a ledger row.
///
/// The sketch covers the two headline figures and leaves the rows alone, which
/// is the right call: a list of covered bars says nothing at all, and the
/// figure worth hiding from someone glancing over is the total, not a $4.33
/// coffee.
fn tx_row(entry: &Entry, masked: bool) -> ViewTxRow {
    ViewTxRow {
        id: entry.id.clone(),
        title: entry.label.clone(),
        category: entry.category.label().to_string(),
        category_token: entry.category.token().to_string(),
        amount: fmt::money_signed(entry.amount(), masked),
        is_inflow: entry.is_inflow(),
        logo_url: entry.logo_url.clone(),
        pending: entry.pending,
    }
}

fn filter_view(state: &StateApp) -> ViewFilter {
    let counts = state.category_counts();

    let categories: Vec<ViewCategoryChip> = counts
        .iter()
        .map(|(category, count)| ViewCategoryChip {
            id: category_id(*category).to_string(),
            label: category.label().to_string(),
            token: category.token().to_string(),
            count: count.to_string(),
            selected: state.ledger().categories().contains(category),
            empty: *count == 0,
        })
        .collect();

    let selected: Vec<ViewCategoryChip> =
        categories.iter().filter(|c| c.selected).cloned().collect();

    let overflow = selected.len().saturating_sub(SUMMARY_CHIPS);

    ViewFilter {
        categories_open: state.ledger().categories_open(),
        range_open: state.ledger().range_open(),
        categories,
        categories_summary: selected.iter().take(SUMMARY_CHIPS).cloned().collect(),
        categories_overflow: (overflow > 0).then(|| format!("+{overflow}")),
        range: range_view(state),
        range_summary: range_summary(&state.ledger().range()),
    }
}

fn range_view(state: &StateApp) -> ViewRange {
    let (this_unit, last_count, last_unit) = state.ledger().range_units();

    let (between_start, between_end) = match state.ledger().range() {
        RangeSelection::Between { .. } => match state.ledger().range().window_bounds() {
            Some((start, end)) => (fmt::short_date(start), fmt::short_date(end)),
            None => (String::new(), String::new()),
        },
        // Defaults that read as a sensible starting sentence rather than as
        // two empty slots.
        _ => (
            fmt::short_date(state.now().saturating_sub_secs(80 * 86_400)),
            fmt::short_date(state.now()),
        ),
    };

    ViewRange {
        kind: match state.ledger().range() {
            RangeSelection::This { .. } => ViewRangeKind::This,
            RangeSelection::Last { .. } => ViewRangeKind::Last,
            RangeSelection::Between { .. } => ViewRangeKind::Between,
            RangeSelection::All => ViewRangeKind::All,
        },
        this_unit: this_unit.label(1).to_string(),
        last_count: last_count.to_string(),
        last_unit: last_unit.label(last_count).to_string(),
        between_start,
        between_end,
    }
}

fn range_summary(range: &RangeSelection) -> String {
    match range {
        RangeSelection::All => "All time".to_string(),
        RangeSelection::This { unit } => format!("This {}", unit.label(1)),
        RangeSelection::Last { count, unit } => {
            format!("Last {count} {}", unit.label(*count))
        }
        RangeSelection::Between { .. } => match range.window_bounds() {
            Some((start, end)) => format!("{} – {}", fmt::short_date(start), fmt::short_date(end)),
            None => "All time".to_string(),
        },
    }
}

fn balances(state: &StateApp) -> ViewBalances {
    let held: f64 = state.connections().iter().map(|c| c.held()).sum();

    ViewBalances {
        headline_label: "Held".to_string(),
        headline: fmt::money(held, state.masked()),
        masked: state.masked(),
        connections: connection_views(state),
    }
}

/// One block per connection, with debts split into their own block.
///
/// Splitting rather than mixing is the sketch's call and a sound one: a
/// mortgage listed between a checking account and an HSA reads as something
/// you have, and the minus sign is doing too much work to say otherwise.
fn connection_views(state: &StateApp) -> Vec<ViewConnection> {
    let mut views = Vec::new();

    for connection in state.connections() {
        let held: Vec<&Account> = connection
            .accounts
            .iter()
            .filter(|a| !a.is_debt())
            .collect();

        if !held.is_empty() {
            views.push(connection_view(
                connection,
                &held,
                state.masked(),
                false,
                state.now(),
            ));
        }
    }

    // Every debt block comes after every held block, so the screen opens on
    // what you have.
    for connection in state.connections() {
        let owed: Vec<&Account> = connection.accounts.iter().filter(|a| a.is_debt()).collect();

        if !owed.is_empty() {
            views.push(connection_view(
                connection,
                &owed,
                state.masked(),
                true,
                state.now(),
            ));
        }
    }

    views
}

fn connection_view(
    connection: &Connection,
    accounts: &[&Account],
    masked: bool,
    is_debts: bool,
    now: Timestamp,
) -> ViewConnection {
    let total: f64 = accounts
        .iter()
        .map(|a| if is_debts { -a.balance } else { a.balance })
        .sum();

    let needs_reconnect = connection.status == ConnectionStatus::LoginRequired;

    ViewConnection {
        id: format!("{}{}", connection.id, if is_debts { "-owed" } else { "" }),
        name: connection.name.clone(),
        initials: connection.initials.clone(),
        synced: match &connection.status {
            ConnectionStatus::Ok { synced_at } => fmt::short_sync(*synced_at, now),
            ConnectionStatus::LoginRequired => String::new(),
        },
        needs_reconnect,
        is_debts,
        accounts: accounts.iter().map(|a| account_row(a, masked)).collect(),
        total_label: format!(
            "{} {}",
            fmt::plural(accounts.len(), "account", "accounts"),
            if is_debts { "owed" } else { "held" },
        ),
        total: fmt::money(total, masked),
    }
}

fn account_row(account: &Account, masked: bool) -> ViewAccountRow {
    let mut parts = vec![account.kind.clone()];

    if let Some(mask) = &account.mask {
        parts.push(format!("·{mask}"));
    }

    // Only one of these is ever worth the room, and available beats APY: a
    // number that moves says more than one that does not.
    if let Some(available) = account.available {
        parts.push(format!("{} available", fmt::money(available, masked)));
    } else if let Some(apy) = account.apy {
        parts.push(format!("{apy:.2}% APY"));
    }

    let amount = if account.is_debt() {
        -account.balance
    } else {
        account.balance
    };

    ViewAccountRow {
        id: account.id.clone(),
        name: account.name.clone(),
        // The kind and mask read as one phrase, so they join with a space and
        // the rest with a separator.
        subtitle: match parts.len() {
            0 | 1 => parts.join(""),
            _ => format!("{} {}", parts[0], parts[1..].join(" · ")),
        },
        amount: fmt::money(amount, masked),
        owed: account.is_debt(),
        utilisation: utilisation(account, masked),
    }
}

/// `"$410 of $2,000 limit used"`, or what is left on a line of credit.
fn utilisation(account: &Account, masked: bool) -> Option<String> {
    let limit = account.limit?;

    if masked {
        return None;
    }

    if !account.is_debt() || limit <= 0.0 {
        return None;
    }

    // A HELOC reports what is still available, and that is the more useful
    // half of it — how much room is left, rather than how much is gone.
    if let Some(available) = account.available {
        return Some(format!("{} still available", fmt_money(available)));
    }

    Some(format!(
        "{} of {} limit used",
        fmt_money(account.balance),
        fmt_money(limit),
    ))
}

fn recurring(state: &StateApp) -> ViewRecurring {
    let active: Vec<&Stream> = state.streams().iter().filter(|s| s.is_active).collect();

    let (out, inflow): (Vec<&Stream>, Vec<&Stream>) = active.iter().partition(|s| s.is_outflow());

    let monthly_out: f64 = out.iter().map(|s| s.monthly_amount()).sum();
    let monthly_in: f64 = inflow.iter().map(|s| s.monthly_amount()).sum();

    let due_soon = out.iter().filter(|s| is_soon(s, state.now())).count();

    ViewRecurring {
        headline: fmt::money(monthly_out, state.masked()),
        detail: if state.masked() {
            fmt::plural(due_soon, "due within the week", "due within the week")
        } else {
            format!(
                "{} in · net {}",
                fmt::money_signed(monthly_in, false),
                fmt_money(monthly_out + monthly_in),
            )
        },
        masked: state.masked(),
        breakdown: breakdown(&out, state.masked()),
        calendar: calendar(state, &out),
        groups: stream_groups(&out, &inflow, state.now(), state.masked()),
    }
}

/// Whether a stream lands within the week.
fn is_soon(stream: &Stream, now: Timestamp) -> bool {
    match stream.next_date {
        Some(next) => {
            let days = now.days_until(next);
            (0..=SOON_DAYS as i64).contains(&days)
        }
        None => false,
    }
}

/// The bar and its legend, biggest share first.
fn breakdown(out: &[&Stream], masked: bool) -> Vec<ViewBreakdownSlice> {
    let mut totals: Vec<(Category, f64)> = Vec::new();

    for stream in out {
        let amount = stream.monthly_amount().abs();

        match totals.iter_mut().find(|(c, _)| *c == stream.category) {
            Some((_, total)) => *total += amount,
            None => totals.push((stream.category, amount)),
        }
    }

    let sum: f64 = totals.iter().map(|(_, t)| t).sum();

    totals.sort_by(|a, b| b.1.total_cmp(&a.1));

    totals
        .into_iter()
        .map(|(category, total)| ViewBreakdownSlice {
            label: category.label().to_string(),
            token: category.token().to_string(),
            // Guarded: an empty list would otherwise divide by zero and hand
            // the view a NaN width.
            share: if sum > 0.0 { total / sum } else { 0.0 },
            amount: fmt::money(total, masked),
        })
        .collect()
}

/// Streams grouped by the same split as the bar, biggest group first, with the
/// inflows last.
fn stream_groups(
    out: &[&Stream],
    inflow: &[&Stream],
    now: Timestamp,
    masked: bool,
) -> Vec<ViewStreamGroup> {
    let mut groups = group_streams(out, now, masked);

    // Money coming in is a different kind of thing from a commitment, so it
    // sits after every outflow group rather than sorted among them.
    groups.extend(group_streams(inflow, now, masked));

    groups
}

fn group_streams(streams: &[&Stream], now: Timestamp, masked: bool) -> Vec<ViewStreamGroup> {
    let mut groups: Vec<(Category, Vec<&Stream>)> = Vec::new();

    for stream in streams {
        match groups.iter_mut().find(|(c, _)| *c == stream.category) {
            Some((_, list)) => list.push(stream),
            None => groups.push((stream.category, vec![stream])),
        }
    }

    // Biggest commitment first, matching the breakdown bar above — the two are
    // meant to read as one split, so they cannot disagree about the order.
    // Sorted here, while the totals are still numbers rather than text.
    groups.sort_by(|(_, a), (_, b)| {
        let total =
            |list: &Vec<&Stream>| list.iter().map(|s| s.monthly_amount().abs()).sum::<f64>();
        total(b).total_cmp(&total(a))
    });

    groups
        .into_iter()
        .map(|(category, mut list)| {
            // Biggest first inside a group too, so the eye lands on the charge
            // that matters.
            list.sort_by(|a, b| a.monthly_amount().total_cmp(&b.monthly_amount()));

            let total: f64 = list.iter().map(|s| s.monthly_amount()).sum();

            ViewStreamGroup {
                name: category.label().to_string(),
                token: category.token().to_string(),
                total: fmt::money(total.abs(), masked),
                streams: list.iter().map(|s| stream_row(s, now, masked)).collect(),
            }
        })
        .collect()
}

fn stream_row(stream: &Stream, now: Timestamp, masked: bool) -> ViewStreamRow {
    let when = match stream.next_date {
        Some(next) => format!(
            "next {} · {}",
            fmt::short_date(next),
            fmt::relative_day(next, now),
        ),
        // Plaid stops predicting a stream it thinks has drifted. Saying so is
        // better than showing a date that is not coming.
        None => "no next date".to_string(),
    };

    ViewStreamRow {
        id: stream.id.clone(),
        title: stream.label.clone(),
        logo_url: stream.logo_url.clone(),
        when,
        soon: is_soon(stream, now),
        amount: fmt::money_signed(stream.amount, masked),
        frequency: stream.frequency.clone(),
    }
}

/// The stable id a category travels under.
///
/// Written out rather than derived from the `Debug` name, so that renaming a
/// variant cannot silently change what a saved filter means.
pub fn category_id(category: Category) -> &'static str {
    match category {
        Category::Income => "Income",
        Category::TransferIn => "TransferIn",
        Category::TransferOut => "TransferOut",
        Category::LoanPayments => "LoanPayments",
        Category::BankFees => "BankFees",
        Category::Entertainment => "Entertainment",
        Category::FoodAndDrink => "FoodAndDrink",
        Category::GeneralMerchandise => "GeneralMerchandise",
        Category::HomeImprovement => "HomeImprovement",
        Category::Medical => "Medical",
        Category::PersonalCare => "PersonalCare",
        Category::GeneralServices => "GeneralServices",
        Category::GovernmentAndNonProfit => "GovernmentAndNonProfit",
        Category::Transportation => "Transportation",
        Category::Travel => "Travel",
        Category::RentAndUtilities => "RentAndUtilities",
        Category::Unknown => "Unknown",
    }
}

pub fn category_from_id(id: &str) -> Category {
    Category::ALL
        .iter()
        .copied()
        .find(|c| category_id(*c) == id)
        .unwrap_or(Category::Unknown)
}

/// The month, with a mark on every day a stream lands.
///
/// Six rows, always, and the days either side of the month drawn rather than
/// left blank: the grid then has one height, and the streams below it do not
/// move when the month turns.
fn calendar(state: &StateApp, out: &[&Stream]) -> ViewCalendar {
    let now = state.now();
    let (year, month, _) = fmt::civil(now);

    let first = fmt::days_from_civil(year, month, 1);

    // 1970-01-01 was a Thursday, and the weeks here start on Monday, so the
    // column a day falls in is its index shifted by three.
    let leading = (first + 3) % 7;

    let start = first - leading;
    let today = now.day_index();

    // What lands when. Only the dates the source predicts — nothing is
    // projected forward from a stream's frequency, so a stream contributes at
    // most one mark.
    let mut marks: BTreeMap<u64, Vec<String>> = BTreeMap::new();

    for stream in out {
        let Some(next) = stream.next_date else { continue };

        marks
            .entry(next.day_index())
            .or_default()
            .push(stream.category.token().to_string());
    }

    let weeks = (0..WEEKS)
        .map(|week| {
            (0..7)
                .map(|column| {
                    let day = start + week * 7 + column;
                    let (_, its_month, its_day) = fmt::civil(Timestamp::from_secs(day * DAY));

                    ViewCalendarDay {
                        label: its_day.to_string(),
                        outside: its_month != month,
                        today: day == today,
                        marks: marks.get(&day).cloned().unwrap_or_default(),
                    }
                })
                .collect()
        })
        .collect();

    ViewCalendar {
        month: format!("{} {}", fmt::month_name(now), fmt::year(now)),
        weekdays: WEEKDAY_INITIALS.iter().map(|d| d.to_string()).collect(),
        weeks,
    }
}
