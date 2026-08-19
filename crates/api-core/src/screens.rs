//! State in, finished screens out.
//!
//! Every string a platform prints is produced here. That is more code in Rust
//! than a view-model of raw values would be, and it is the trade `docs/FFI.md`
//! argues for: the type surface grows a little, and the number of places that
//! can format a date differently stays at one.

use api_common::format::fmt_money;
use api_ledger::Account;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Stream;

use crate::event::RangeSelection;
use crate::fmt;
use crate::state::State;
use crate::view::*;

/// How many ledger rows Home previews.
const HOME_LEDGER_ROWS: usize = 4;

/// How many upcoming streams Home previews.
const HOME_UPCOMING_ROWS: usize = 2;

/// A stream landing within this many days reads as a heads-up.
const SOON_DAYS: u64 = 7;

/// How many selected categories a shut filter names before it says "+2".
const SUMMARY_CHIPS: usize = 2;

// ── the screen ──────────────────────────────────────────────────────────────

pub fn screen(state: &State) -> ScreenView {
    match state.screen {
        Screen::Home => ScreenView::Home { view: home(state) },
        Screen::Ledger => ScreenView::Ledger {
            view: ledger(state),
        },
        Screen::Recurring => ScreenView::Recurring {
            view: recurring(state),
        },
        Screen::Balances => ScreenView::Balances {
            view: balances(state),
        },
    }
}

pub fn dock(current: Screen) -> DockView {
    let item = |screen: Screen, label: &str, icon: &str| DockItem {
        screen,
        label: label.to_string(),
        icon: icon.to_string(),
        selected: screen == current,
    };

    DockView {
        items: vec![
            item(Screen::Home, "Home", "home"),
            item(Screen::Ledger, "Ledger", "ledger"),
            item(Screen::Recurring, "Recurring", "recurring"),
            item(Screen::Balances, "Balances", "balances"),
        ],
    }
}

// ── ledger ──────────────────────────────────────────────────────────────────

fn ledger(state: &State) -> LedgerView {
    let entries = state.filtered();

    LedgerView {
        subtitle: sync_subtitle(state),
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
fn empty_message(state: &State, shown: usize) -> Option<String> {
    if shown > 0 {
        return None;
    }

    if let Some(error) = &state.load_failed
        && !state.loaded
    {
        return Some(error.clone());
    }

    if !state.loaded {
        return Some("Loading…".to_string());
    }

    if state.filter.is_active() && !state.entries.is_empty() {
        return Some("Nothing matches this filter.".to_string());
    }

    Some("No transactions yet.".to_string())
}

/// The rows, cut into months and then days.
///
/// One pass: the entries arrive newest-first and stay in that order, so a
/// change of day or month is simply a change of key from the row before.
fn group_by_month(entries: &[&Entry]) -> Vec<MonthSection> {
    let mut months: Vec<MonthSection> = Vec::new();

    for entry in entries {
        let ts = entry.ts();

        let new_month = months
            .last()
            .is_none_or(|m| m.name != fmt::month_name(ts) || m.year != fmt::year(ts));

        if new_month {
            months.push(MonthSection {
                name: fmt::month_name(ts),
                year: fmt::year(ts),
                days: Vec::new(),
            });
        }

        let month = months.last_mut().expect("just pushed");

        let heading = fmt::day_heading(ts);
        let new_day = month.days.last().is_none_or(|d| d.heading != heading);

        if new_day {
            month.days.push(DaySection {
                heading,
                rows: Vec::new(),
            });
        }

        month
            .days
            .last_mut()
            .expect("just pushed")
            .rows
            .push(tx_row(entry, false));
    }

    months
}

/// Money is never masked on a ledger row.
///
/// The sketch covers the two headline figures and leaves the rows alone, which
/// is the right call: a list of covered bars says nothing at all, and the
/// figure worth hiding from someone glancing over is the total, not a $4.33
/// coffee.
fn tx_row(entry: &Entry, masked: bool) -> TxRow {
    TxRow {
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

// ── the filter panel ────────────────────────────────────────────────────────

fn filter_view(state: &State) -> FilterView {
    let counts = state.category_counts();

    let categories: Vec<CategoryChip> = counts
        .iter()
        .map(|(category, count)| CategoryChip {
            id: category_id(*category).to_string(),
            label: category.label().to_string(),
            token: category.token().to_string(),
            count: count.to_string(),
            selected: state.filter.categories.contains(category),
            empty: *count == 0,
        })
        .collect();

    let selected: Vec<CategoryChip> = categories.iter().filter(|c| c.selected).cloned().collect();

    let overflow = selected.len().saturating_sub(SUMMARY_CHIPS);

    FilterView {
        open: state.filter.open,
        categories_open: state.filter.categories_open,
        range_open: state.filter.range_open,
        categories,
        categories_summary: selected.iter().take(SUMMARY_CHIPS).cloned().collect(),
        categories_overflow: (overflow > 0).then(|| format!("+{overflow}")),
        range: range_view(state),
        range_summary: range_summary(&state.filter.range),
        active: state.filter.is_active(),
    }
}

fn range_view(state: &State) -> RangeView {
    let (this_unit, last_count, last_unit) = state.range_units();

    let (between_start, between_end) = match state.filter.range {
        RangeSelection::Between { start, end } => (fmt::short_date(start), fmt::short_date(end)),
        // Defaults that read as a sensible starting sentence rather than as
        // two empty slots.
        _ => (
            fmt::short_date(state.now.saturating_sub(80 * 86_400)),
            fmt::short_date(state.now),
        ),
    };

    RangeView {
        kind: match state.filter.range {
            RangeSelection::This { .. } => RangeKind::This,
            RangeSelection::Last { .. } => RangeKind::Last,
            RangeSelection::Between { .. } => RangeKind::Between,
            RangeSelection::All => RangeKind::All,
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
        RangeSelection::Between { start, end } => {
            format!("{} – {}", fmt::short_date(*start), fmt::short_date(*end))
        }
    }
}

// ── balances ────────────────────────────────────────────────────────────────

fn balances(state: &State) -> BalancesView {
    let accounts: usize = state.connections.iter().map(|c| c.accounts.len()).sum();

    let held: f64 = state.connections.iter().map(|c| c.held()).sum();

    BalancesView {
        subtitle: format!(
            "{} · {}",
            fmt::plural(state.connections.len(), "connection", "connections"),
            fmt::plural(accounts, "account", "accounts"),
        ),
        headline_label: "Held".to_string(),
        headline: fmt::money(held, state.masked),
        masked: state.masked,
        connections: connection_views(state),
    }
}

/// One block per connection, with debts split into their own block.
///
/// Splitting rather than mixing is the sketch's call and a sound one: a
/// mortgage listed between a checking account and an HSA reads as something
/// you have, and the minus sign is doing too much work to say otherwise.
fn connection_views(state: &State) -> Vec<ConnectionView> {
    let mut views = Vec::new();

    for connection in &state.connections {
        let held: Vec<&Account> = connection
            .accounts
            .iter()
            .filter(|a| !a.is_debt())
            .collect();

        if !held.is_empty() {
            views.push(connection_view(
                connection,
                &held,
                state.masked,
                false,
                state.now,
            ));
        }
    }

    // Every debt block comes after every held block, so the screen opens on
    // what you have.
    for connection in &state.connections {
        let owed: Vec<&Account> = connection.accounts.iter().filter(|a| a.is_debt()).collect();

        if !owed.is_empty() {
            views.push(connection_view(
                connection,
                &owed,
                state.masked,
                true,
                state.now,
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
    now: u64,
) -> ConnectionView {
    let total: f64 = accounts
        .iter()
        .map(|a| if is_debts { -a.balance } else { a.balance })
        .sum();

    let needs_reconnect = connection.status == ConnectionStatus::LoginRequired;

    ConnectionView {
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

fn account_row(account: &Account, masked: bool) -> AccountRow {
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

    AccountRow {
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

// ── recurring ───────────────────────────────────────────────────────────────

fn recurring(state: &State) -> RecurringView {
    let active: Vec<&Stream> = state.streams.iter().filter(|s| s.is_active).collect();

    let (out, inflow): (Vec<&Stream>, Vec<&Stream>) = active.iter().partition(|s| s.is_outflow());

    let monthly_out: f64 = out.iter().map(|s| s.monthly_amount()).sum();
    let monthly_in: f64 = inflow.iter().map(|s| s.monthly_amount()).sum();

    let due_soon = out.iter().filter(|s| is_soon(s, state.now)).count();

    RecurringView {
        subtitle: format!(
            "{} · {} out, {} in",
            fmt::plural(active.len(), "stream", "streams"),
            out.len(),
            inflow.len(),
        ),
        headline: fmt::money(monthly_out, state.masked),
        detail: if state.masked {
            fmt::plural(due_soon, "due within the week", "due within the week")
        } else {
            format!(
                "{} in · net {}",
                fmt::money_signed(monthly_in, false),
                fmt_money(monthly_out + monthly_in),
            )
        },
        masked: state.masked,
        breakdown: breakdown(&out, state.masked),
        groups: stream_groups(&out, &inflow, state.now, state.masked),
    }
}

/// Whether a stream lands within the week.
fn is_soon(stream: &Stream, now: u64) -> bool {
    match stream.next_date {
        Some(next) => {
            let days = fmt::day_index(next) as i64 - fmt::day_index(now) as i64;
            (0..=SOON_DAYS as i64).contains(&days)
        }
        None => false,
    }
}

/// The bar and its legend, biggest share first.
fn breakdown(out: &[&Stream], masked: bool) -> Vec<BreakdownSlice> {
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
        .map(|(category, total)| BreakdownSlice {
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
fn stream_groups(out: &[&Stream], inflow: &[&Stream], now: u64, masked: bool) -> Vec<StreamGroup> {
    let mut groups = group_streams(out, now, masked);

    // Money coming in is a different kind of thing from a commitment, so it
    // sits after every outflow group rather than sorted among them.
    groups.extend(group_streams(inflow, now, masked));

    groups
}

fn group_streams(streams: &[&Stream], now: u64, masked: bool) -> Vec<StreamGroup> {
    let mut groups: Vec<(Category, Vec<&Stream>)> = Vec::new();

    for stream in streams {
        match groups.iter_mut().find(|(c, _)| *c == stream.category) {
            Some((_, list)) => list.push(stream),
            None => groups.push((stream.category, vec![stream])),
        }
    }

    let mut groups: Vec<StreamGroup> = groups
        .into_iter()
        .map(|(category, mut list)| {
            // Biggest first inside a group too, so the eye lands on the charge
            // that matters.
            list.sort_by(|a, b| a.monthly_amount().total_cmp(&b.monthly_amount()));

            let total: f64 = list.iter().map(|s| s.monthly_amount()).sum();

            StreamGroup {
                name: category.label().to_string(),
                token: category.token().to_string(),
                total: fmt::money(total.abs(), masked),
                streams: list.iter().map(|s| stream_row(s, now, masked)).collect(),
            }
        })
        .collect();

    groups.sort_by(|a, b| {
        let value = |g: &StreamGroup| g.streams.len();
        value(b).cmp(&value(a))
    });

    groups
}

fn stream_row(stream: &Stream, now: u64, masked: bool) -> StreamRow {
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

    StreamRow {
        id: stream.id.clone(),
        title: stream.label.clone(),
        logo_url: stream.logo_url.clone(),
        when,
        soon: is_soon(stream, now),
        amount: fmt::money_signed(stream.amount, masked),
        frequency: stream.frequency.clone(),
    }
}

// ── home ────────────────────────────────────────────────────────────────────

fn home(state: &State) -> HomeView {
    let accounts: usize = state.connections.iter().map(|c| c.accounts.len()).sum();
    let held: f64 = state.connections.iter().map(|c| c.held()).sum();

    let active: Vec<&Stream> = state.streams.iter().filter(|s| s.is_active).collect();
    let out: Vec<&Stream> = active.iter().copied().filter(|s| s.is_outflow()).collect();

    let monthly_out: f64 = out.iter().map(|s| s.monthly_amount()).sum();

    // What lands next, soonest first — the only ordering that makes a
    // two-row preview useful.
    let mut upcoming: Vec<&Stream> = out
        .iter()
        .copied()
        .filter(|s| s.next_date.is_some())
        .collect();

    upcoming.sort_by_key(|s| s.next_date.unwrap_or(u64::MAX));

    let due_soon = out.iter().filter(|s| is_soon(s, state.now)).count();

    HomeView {
        balances: HomeBalances {
            headline: fmt::money(held, state.masked),
            masked: state.masked,
            detail: format!(
                "held across {}",
                fmt::plural(accounts, "account", "accounts")
            ),
        },
        recurring: HomeRecurring {
            headline: fmt::money(monthly_out, state.masked),
            masked: state.masked,
            detail: match due_soon {
                0 => "nothing due this week".to_string(),
                n => format!("{n} due within the week"),
            },
            breakdown: breakdown(&out, state.masked),
            upcoming: upcoming
                .iter()
                .take(HOME_UPCOMING_ROWS)
                .map(|s| stream_row(s, state.now, state.masked))
                .collect(),
        },
        ledger: state
            .entries
            .iter()
            .take(HOME_LEDGER_ROWS)
            .map(|e| tx_row(e, false))
            .collect(),
        attention: state
            .connections
            .iter()
            .filter(|c| c.status == ConnectionStatus::LoginRequired)
            .map(|c| {
                let accounts: Vec<&Account> = c.accounts.iter().collect();
                connection_view(c, &accounts, state.masked, false, state.now)
            })
            .collect(),
    }
}

// ── shared ──────────────────────────────────────────────────────────────────

/// `"3 connections · synced 2m ago"`.
fn sync_subtitle(state: &State) -> String {
    let synced = state
        .connections
        .iter()
        .find_map(|c| match &c.status {
            ConnectionStatus::Ok { synced_at } => Some(fmt::relative_time(*synced_at, state.now)),
            ConnectionStatus::LoginRequired => None,
        })
        .unwrap_or_else(|| "not syncing".to_string());

    format!(
        "{} · {synced}",
        fmt::plural(state.connections.len(), "connection", "connections"),
    )
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
