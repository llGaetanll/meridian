//! State in, finished screens out.
//!
//! Every string a platform prints is produced here, so the number of places
//! that can format a date differently stays at one. See `docs/FFI.md`.

use std::collections::BTreeMap;

use api_common::format::fmt_money;
use api_ledger::Account;
use api_ledger::Category;
use api_ledger::Connection;
use api_ledger::ConnectionStatus;
use api_ledger::Entry;
use api_ledger::Stream;
use api_ledger::initials;

use api_common::Timestamp;

use crate::LoadFailure;
use crate::RangeSelection;
use crate::Screen;
use crate::StateApp;
use crate::StateDelete;
use crate::StateRemove;
use crate::ViewAccount;
use crate::ViewAccountBank;
use crate::ViewAccountDelete;
use crate::ViewAccountName;
use crate::ViewAccountRow;
use crate::ViewBreakdownSlice;
use crate::ViewCalendar;
use crate::ViewCalendarDay;
use crate::ViewCalendarMonth;
use crate::ViewCategoryChip;
use crate::ViewConnection;
use crate::ViewConnectionRemove;
use crate::ViewConnections;
use crate::ViewDayCard;
use crate::ViewDayRow;
use crate::ViewDaySection;
use crate::ViewDock;
use crate::ViewDockItem;
use crate::ViewField;
use crate::ViewFilter;
use crate::ViewLedger;
use crate::ViewMonthSection;
use crate::ViewRange;
use crate::ViewRangeKind;
use crate::ViewRecurring;
use crate::ViewScreen;
use crate::ViewSettings;
use crate::ViewStreamGroup;
use crate::ViewStreamRow;
use crate::ViewTxRow;
use crate::ViewWho;
use crate::fmt;
use crate::screens_onboarding;
use crate::screens_paywall;
use crate::state_app::User;

/// Seconds in a day, for stepping the calendar a cell at a time.
const DAY: u64 = 86_400;

/// The calendar's column headings, starting on Monday.
const WEEKDAY_INITIALS: [&str; 7] = ["M", "T", "W", "T", "F", "S", "S"];

/// A stream landing within this many days reads as a heads-up.
const SOON_DAYS: u64 = 7;

/// How many selected categories a shut filter names before it says "+2".
const SUMMARY_CHIPS: usize = 2;

/// How many selected categories the Ledger's headline names before it counts
/// them instead.
const HEADLINE_CATEGORIES: usize = 2;

/// What the Connections page says with no bank on it. Onboarding does not
/// leave an account here, but a link that failed does.
const NO_CONNECTIONS: &str =
    "No bank is connected, so there is nothing to show anywhere else either.";

/// The same case on the Account tab, where the list is one section of a page
/// rather than the whole of it.
const NO_BANKS: &str = "No banks connected.";

pub fn screen(state: &StateApp) -> ViewScreen {
    // In front of everything, first run included: every route that feeds a
    // screen is refused for this session, so there is nothing behind it to
    // draw. The bank step of first run is one of those routes.
    if let Some(unpaid) = state.unpaid() {
        return ViewScreen::Paywall {
            view: screens_paywall::paywall(unpaid, state.payment()),
        };
    }

    // First run is a road with an end, so it owns the screen — tabs included.
    if let Some(flow) = state.onboarding() {
        return ViewScreen::Onboarding {
            view: screens_onboarding::onboarding(flow, state.connections(), state.now()),
        };
    }

    match state.screen() {
        Screen::Ledger => ViewScreen::Ledger {
            view: ledger(state),
        },
        Screen::Recurring => ViewScreen::Recurring {
            view: recurring(state),
        },
        Screen::Connections => ViewScreen::Connections {
            view: connections(state),
        },
        Screen::Account => ViewScreen::Account {
            view: account(state),
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
            item(Screen::Connections, "Connections", "bank"),
            item(Screen::Account, "Account", "user"),
        ],
    }
}

/// Who the device is signed in as, for the account chip beside every screen.
///
/// `None` until a snapshot has said. The chip is the only thing that draws it
/// and an account chip with a guess on it is worse than one that waits.
pub fn who(user: Option<&User>) -> Option<ViewWho> {
    let user = user?;

    Some(ViewWho {
        title: user.name.clone(),
        subtitle: user.email.clone(),
        settings_label: "Settings".to_string(),
        sign_out_label: "Sign out".to_string(),
    })
}

/// The settings window, drawn over whichever screen is up.
///
/// `None` for the same reason [`who`] is: until a snapshot has said who this
/// is there is no name to change, and a window with an empty field in it is a
/// window that has guessed.
pub fn settings(state: &StateApp) -> Option<ViewSettings> {
    state.user()?;

    Some(ViewSettings {
        title: "Settings".to_string(),
        account_label: "Account Settings".to_string(),
        close_label: "Close".to_string(),
        name: account_name(state),
        delete_label: "Delete my account".to_string(),
        delete: state.deleting().map(delete_confirmation),
    })
}

/// What a failure reads as.
///
/// Every platform gets this sentence rather than writing its own, which is the
/// whole reason the runtimes hand over a reason instead of a string. Neither
/// of the two failures the machine acts on is worded here: they are screens,
/// not sentences.
pub(crate) fn failure(failure: &LoadFailure) -> String {
    match failure {
        LoadFailure::Offline => "Meridian can't reach the server.".to_string(),

        LoadFailure::TooMany => "That was a lot of asking at once. Give it a moment.".to_string(),

        LoadFailure::Server { status } => {
            format!("The server is having trouble ({status}).")
        }

        LoadFailure::Unreadable => "The server sent something Meridian can't read.".to_string(),

        // None of these reaches a banner: one is first run, one is the
        // paywall, and one asks for a snapshot instead — all decided in
        // `StateApp`.
        LoadFailure::SignedOut => "You've been signed out.".to_string(),
        LoadFailure::Unpaid { .. } => "Meridian is a subscription.".to_string(),
        LoadFailure::Subscribed => "You already have a subscription.".to_string(),
    }
}

fn ledger(state: &StateApp) -> ViewLedger {
    let entries = state.filtered();
    let masked = state.masked();

    let flow_in: f64 = entries
        .iter()
        .map(|e| e.amount())
        .filter(|a| *a > 0.0)
        .sum();
    let flow_out: f64 = entries
        .iter()
        .map(|e| e.amount())
        .filter(|a| *a < 0.0)
        .sum();

    ViewLedger {
        headline_label: headline_label(state, entries.len()),
        headline: fmt::money_signed(flow_in + flow_out, masked),
        // Unsigned: the word beside each figure says which way money goes.
        flow_in: fmt::money(flow_in, masked),
        flow_out: fmt::money(flow_out.abs(), masked),
        months: group_by_month(&entries, &account_labels(state)),
        filter: filter_view(state),
        empty: empty_message(state, entries.len()),
    }
}

/// What the headline is a figure for: the range, the categories, and how many
/// rows the two of them let through.
fn headline_label(state: &StateApp, shown: usize) -> String {
    let selected: Vec<&str> = Category::ALL
        .iter()
        .filter(|category| state.ledger().categories().contains(category))
        .map(|category| category.label())
        .collect();

    let categories = match selected.len() {
        0 => "every category".to_string(),
        count if count <= HEADLINE_CATEGORIES => selected.join(", "),
        count => format!("{count} categories"),
    };

    format!(
        "{} · {categories} · {}",
        range_summary(&state.ledger().range()),
        fmt::plural(shown, "transaction", "transactions"),
    )
}

/// Each account's name as a row prints it, `"Plaid Checking ·0000"`, by id.
fn account_labels(state: &StateApp) -> BTreeMap<&str, String> {
    state
        .connections()
        .iter()
        .flat_map(|connection| &connection.accounts)
        .map(|account| {
            let label = match &account.mask {
                Some(mask) => format!("{} ·{mask}", account.name),
                None => account.name.clone(),
            };

            (account.id.as_str(), label)
        })
        .collect()
}

/// What the list says when it has nothing to show.
///
/// Three nothings, three different words: loading, filtered out, empty.
fn empty_message(state: &StateApp, shown: usize) -> Option<String> {
    if shown > 0 {
        return None;
    }

    if let Some(reason) = state.load_failed()
        && !state.loaded()
    {
        return Some(failure(reason));
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
/// One pass, relying on entries arriving newest-first: a change of day or
/// month is a change of key from the row before.
fn group_by_month(entries: &[&Entry], accounts: &BTreeMap<&str, String>) -> Vec<ViewMonthSection> {
    let mut months: Vec<Month<'_>> = Vec::new();

    for entry in entries {
        let ts = entry.ts();
        let (name, year, heading) = (fmt::month_name(ts), fmt::year(ts), fmt::day_heading(ts));

        let days = match months.last_mut() {
            Some((month, this_year, days)) if *month == name && *this_year == year => days,
            _ => {
                months.push((name, year, Vec::new()));

                let Some((_, _, days)) = months.last_mut() else {
                    unreachable!("a month was just pushed")
                };

                days
            }
        };

        let rows = match days.last_mut() {
            Some((day, rows)) if *day == heading => rows,
            _ => {
                days.push((heading, Vec::new()));

                let Some((_, rows)) = days.last_mut() else {
                    unreachable!("a day was just pushed")
                };

                rows
            }
        };

        rows.push(entry);
    }

    debug_assert!(
        months.iter().all(|(_, _, days)| !days.is_empty()),
        "a month section with no days should not exist",
    );

    months
        .into_iter()
        .map(|(name, year, days)| ViewMonthSection {
            label: format!("{name} {year}"),
            name,
            year,
            days: days
                .into_iter()
                .map(|day| day_section(day, accounts))
                .collect(),
        })
        .collect()
}

/// A month being filled: its name, its year, and its days.
type Month<'a> = (String, String, Vec<Day<'a>>);

/// A day being filled: its heading, and the entries that fall on it.
type Day<'a> = (String, Vec<&'a Entry>);

/// A day's rows, under a heading that carries the day's net.
fn day_section((heading, entries): Day<'_>, accounts: &BTreeMap<&str, String>) -> ViewDaySection {
    let net: f64 = entries.iter().map(|e| e.amount()).sum();

    ViewDaySection {
        heading,
        total: fmt::money_signed(net, false),
        rows: entries.iter().map(|e| tx_row(e, accounts, false)).collect(),
    }
}

/// A ledger row. Masking is for headline figures, not a $4.33 coffee.
fn tx_row(entry: &Entry, accounts: &BTreeMap<&str, String>, masked: bool) -> ViewTxRow {
    ViewTxRow {
        id: entry.id.clone(),
        title: entry.label.clone(),
        category: entry.category.label().to_string(),
        category_token: entry.category.token().to_string(),
        amount: fmt::money_signed(entry.amount(), masked),
        is_inflow: entry.is_inflow(),
        account: entry
            .account_id
            .as_deref()
            .and_then(|id| accounts.get(id))
            .cloned()
            .unwrap_or_default(),
        logo_url: entry.logo_url.clone(),
        initials: initials(&entry.label),
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
        // Defaults, so the picker opens on a sentence rather than two blanks.
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

/// Which logins the app has, and what each one brings in.
fn connections(state: &StateApp) -> ViewConnections {
    let banks = state.connections().len();

    let accounts: usize = state
        .connections()
        .iter()
        .map(|connection| connection.accounts.len())
        .sum();

    ViewConnections {
        title: "Connections".to_string(),
        count: match banks {
            0 => "No banks".to_string(),
            _ => format!(
                "{} · {}",
                fmt::plural(banks, "bank", "banks"),
                fmt::plural(accounts, "account", "accounts"),
            ),
        },
        add_label: "Add connection".to_string(),
        adding: state.connecting(),
        connections: connection_views(state),
        empty: state
            .connections()
            .is_empty()
            .then(|| no_banks_message(state, NO_CONNECTIONS)),
    }
}

/// What the "are you sure" under one bank's trash says, at whichever step it
/// is on.
///
/// Named, even though the card stands on the block it is about: a question
/// that says which bank it is about can be answered without looking up at
/// what opened it.
fn remove_confirmation(step: &StateRemove, name: &str) -> ViewConnectionRemove {
    match step {
        StateRemove::Asking { .. } | StateRemove::Refused { .. } => ViewConnectionRemove {
            title: format!("Remove {name}?"),
            body: "Its accounts, transactions and recurring bills are deleted \
                   permanently from Meridian."
                .to_string(),
            error: matches!(step, StateRemove::Refused { .. })
                .then(|| "That did not work. Nothing was removed.".to_string()),
            confirm_label: Some(
                match step {
                    StateRemove::Refused { .. } => "Try again",
                    _ => "Remove",
                }
                .to_string(),
            ),
            cancel_label: Some("Cancel".to_string()),
            busy: false,
        },

        StateRemove::Sending { .. } => ViewConnectionRemove {
            title: format!("Removing {name}…"),
            body: "The bank is being disconnected and its transactions deleted.".to_string(),
            error: None,
            confirm_label: None,
            cancel_label: None,
            busy: true,
        },
    }
}

/// One block per login, held and owed together: the page answers which logins
/// we have and whether they are working, and a bank split in two answers that
/// twice.
fn connection_views(state: &StateApp) -> Vec<ViewConnection> {
    state
        .connections()
        .iter()
        .map(|connection| {
            connection_view(
                connection,
                state.masked(),
                state.reconnecting() == Some(connection.id.as_str()),
                state
                    .removing()
                    .filter(|step| step.item_id() == connection.id),
                state.now(),
            )
        })
        .collect()
}

fn connection_view(
    connection: &Connection,
    masked: bool,
    reconnecting: bool,
    removing: Option<&StateRemove>,
    now: Timestamp,
) -> ViewConnection {
    let needs_reconnect = connection.status == ConnectionStatus::LoginRequired;

    ViewConnection {
        id: connection.id.clone(),
        item_id: connection.id.clone(),
        name: connection.name.clone(),
        synced: match &connection.status {
            ConnectionStatus::Ok { synced_at } => format!("Synced {}", fmt::ago(*synced_at, now)),
            ConnectionStatus::LoginRequired => String::new(),
        },
        needs_reconnect,
        reconnect_label: "Sign in".to_string(),
        remove_label: "Remove bank".to_string(),
        reconnecting,
        accounts: connection
            .accounts
            .iter()
            .map(|account| account_row(account, masked, needs_reconnect))
            .collect(),
        remove: removing.map(|step| remove_confirmation(step, &connection.name)),
    }
}

fn account_row(account: &Account, masked: bool, stale: bool) -> ViewAccountRow {
    let kind = fmt::account_kind(account.subtype.as_deref(), account.group);

    // The kind and the last four digits read as one phrase, so only what
    // comes after them gets a separator.
    let phrase = match &account.mask {
        Some(mask) => format!("{kind} ·{mask}"),
        None => kind.clone(),
    };

    // Only one fits, and available beats APY: it moves.
    let detail = if let Some(available) = account.available {
        format!("{} available", fmt::money(available, masked))
    } else if let Some(apy) = account.apy {
        format!("{apy:.2}% APY")
    } else {
        String::new()
    };

    let amount = if account.is_debt() {
        -account.balance
    } else {
        account.balance
    };

    ViewAccountRow {
        id: account.id.clone(),
        name: account.name.clone(),
        subtitle: if detail.is_empty() {
            phrase
        } else {
            format!("{phrase} · {detail}")
        },
        mask: account.mask.clone().unwrap_or_default(),
        kind,
        detail,
        amount: fmt::money(amount, masked),
        owed: account.is_debt(),
        stale,
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

    // A HELOC reports what is still available, the more useful half.
    if let Some(available) = account.available {
        return Some(format!("{} still available", fmt_money(available)));
    }

    Some(format!(
        "{} of {} limit used",
        fmt_money(account.balance),
        fmt_money(limit),
    ))
}

/// Which banks are attached, and the way out.
fn account(state: &StateApp) -> ViewAccount {
    let banks: Vec<ViewAccountBank> = state
        .connections()
        .iter()
        .map(|connection| ViewAccountBank {
            id: connection.id.clone(),
            name: connection.name.clone(),
            initials: connection.initials.clone(),
            detail: match &connection.status {
                ConnectionStatus::Ok { synced_at } => format!(
                    "{} · {}",
                    fmt::plural(connection.accounts.len(), "account", "accounts"),
                    fmt::relative_time(*synced_at, state.now()),
                ),
                ConnectionStatus::LoginRequired => "Sign-in expired · not syncing".to_string(),
            },
            needs_reconnect: connection.status == ConnectionStatus::LoginRequired,
        })
        .collect();

    ViewAccount {
        name: account_name(state),
        banks_label: "Banks".to_string(),
        banks_empty: banks.is_empty().then(|| no_banks_message(state, NO_BANKS)),
        banks,
        session_label: "Session".to_string(),
        sign_out_label: "Sign out".to_string(),
        delete_label: "Delete my account".to_string(),
        delete: state.deleting().map(delete_confirmation),
        build: format!("Meridian {}", env!("CARGO_PKG_VERSION")),
    }
}

/// The name, and what it takes to change it.
///
/// The field draws the draft if there is one and the kept name if there is
/// not, so arriving on this screen shows what the app calls you rather than an
/// empty box you have to guess the contents of.
fn account_name(state: &StateApp) -> Option<ViewAccountName> {
    let kept = state.user()?.name.as_str();
    let value = state.renaming().unwrap_or(kept);

    Some(ViewAccountName {
        label: "User name".to_string(),

        field: ViewField {
            icon: "user".to_string(),
            value: value.to_string(),
            placeholder: "Your name".to_string(),

            // A name is whatever somebody says it is. The only two values
            // this refuses are none at all and the one already saved, and
            // neither is a thing gone wrong — they are just nothing to do.
            fault: None,
        },

        save_label: "Save".to_string(),

        can_save: {
            let typed = value.trim();

            !typed.is_empty() && typed != kept
        },
    })
}

/// The delete confirmation, worded for the step it is on.
///
/// The loud button is the safe one on every step, so the destructive label is
/// plain and the cancel is what a reflexive tap lands on.
fn delete_confirmation(step: &StateDelete) -> ViewAccountDelete {
    match step {
        StateDelete::Confirming | StateDelete::SendingCode => ViewAccountDelete {
            title: "Delete your account?".to_string(),
            body: "Your banks, transactions and sign-in are deleted permanently. \
                   To confirm, we will email a 6-digit code to your address."
                .to_string(),
            code: None,
            error: None,
            confirm_label: matches!(step, StateDelete::Confirming)
                .then(|| "Email me the code".to_string()),
            cancel_label: Some("Keep my account".to_string()),
            busy: matches!(step, StateDelete::SendingCode),
        },

        StateDelete::Code { digits, wrong } => ViewAccountDelete {
            title: "Check your email".to_string(),
            body: "Enter the 6-digit code we sent you. \
                   Entering it deletes your account permanently."
                .to_string(),
            code: Some(digits.clone()),
            error: wrong
                .then(|| "That code was not right. Check the email and try again.".to_string()),
            confirm_label: (digits.len() == crate::state_onboarding::CODE_LENGTH)
                .then(|| "Delete everything".to_string()),
            cancel_label: Some("Keep my account".to_string()),
            busy: false,
        },

        StateDelete::Deleting => ViewAccountDelete {
            title: "Deleting your account…".to_string(),
            body: "Your banks are being disconnected and your data deleted.".to_string(),
            code: None,
            error: None,
            confirm_label: None,
            cancel_label: None,
            busy: true,
        },
    }
}

/// What a list of banks says when it is empty, and what it says instead while
/// the snapshot behind it is missing.
///
/// The failure comes first, with no `loaded` guard: an empty list is what
/// somebody is looking at *because* the app is not working.
fn no_banks_message(state: &StateApp, settled: &str) -> String {
    if let Some(reason) = state.load_failed() {
        return failure(reason);
    }

    if !state.loaded() {
        return "Loading…".to_string();
    }

    settled.to_string()
}

fn recurring(state: &StateApp) -> ViewRecurring {
    let active: Vec<&Stream> = state.streams().iter().filter(|s| s.is_active).collect();

    let (out, inflow): (Vec<&Stream>, Vec<&Stream>) = active.iter().partition(|s| s.is_outflow());

    let monthly_out: f64 = out.iter().map(|s| s.monthly_amount()).sum();
    let monthly_in: f64 = inflow.iter().map(|s| s.monthly_amount()).sum();

    let due_soon = out.iter().filter(|s| is_soon(s, state.now())).count();

    let masked = state.masked();
    let groups = stream_groups(&out, &inflow, state.now(), masked);
    let streams: usize = groups.iter().map(|group| group.streams.len()).sum();

    ViewRecurring {
        // Not "so far this month": every figure here is Plaid's prediction of
        // what comes back, not a claim about what has landed.
        headline_label: "Every month".to_string(),
        headline: fmt::money(monthly_out + monthly_in, masked),
        detail: if masked {
            fmt::plural(due_soon, "due within the week", "due within the week")
        } else {
            format!(
                "{} in · net {}",
                fmt::money_signed(monthly_in, false),
                fmt_money(monthly_out + monthly_in),
            )
        },
        // Unsigned: the word beside each figure says which way money goes.
        flow_in: fmt::money(monthly_in.abs(), masked),
        flow_out: fmt::money(monthly_out.abs(), masked),
        flow_in_signed: fmt::money_signed(monthly_in, masked),
        flow_out_signed: fmt::money_signed(monthly_out, masked),
        masked,
        breakdown_label: "Where it goes".to_string(),
        breakdown_total: fmt::money(monthly_out, masked),
        breakdown: breakdown(&out, masked),
        calendar: calendar(state, &out),
        day_card: day_card(state, &out),
        streams_label: "Every stream".to_string(),
        streams_count: streams.to_string(),
        groups,
    }
}

/// What lands on the day the calendar has selected.
///
/// `None` covers both "nothing tapped" and "nothing lands there".
fn day_card(state: &StateApp, out: &[&Stream]) -> Option<ViewDayCard> {
    let picked = state.picked_day()?;
    let masked = state.masked();

    let landing: Vec<&&Stream> = out
        .iter()
        .filter(|stream| {
            stream
                .next_date
                .is_some_and(|next| next.day_index() == picked)
        })
        .collect();

    if landing.is_empty() {
        return None;
    }

    let total: f64 = landing.iter().map(|stream| stream.amount).sum();

    Some(ViewDayCard {
        heading: fmt::day_heading(stamp(picked)),
        total: fmt::money_signed(total, masked),
        rows: landing
            .iter()
            .map(|stream| ViewDayRow {
                id: stream.id.clone(),
                name: stream.label.clone(),
                amount: fmt::money_signed(stream.amount, masked),
            })
            .collect(),
    })
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
            // Guarded: an empty list would divide by zero into a NaN width.
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

    // Money in is not a commitment, so it sits after every outflow group.
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

    // Biggest first, matching the breakdown bar: the two read as one split.
    groups.sort_by(|(_, a), (_, b)| {
        let total =
            |list: &Vec<&Stream>| list.iter().map(|s| s.monthly_amount().abs()).sum::<f64>();
        total(b).total_cmp(&total(a))
    });

    groups
        .into_iter()
        .map(|(category, mut list)| {
            // Biggest first inside a group too.
            list.sort_by(|a, b| a.monthly_amount().total_cmp(&b.monthly_amount()));

            let total: f64 = list.iter().map(|s| s.monthly_amount()).sum();

            ViewStreamGroup {
                name: category.label().to_string(),
                token: category.token().to_string(),
                total: fmt::money(total.abs(), masked),
                count_label: fmt::plural(list.len(), "stream", "streams"),
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
        // Plaid stops predicting a stream it thinks has drifted.
        None => "no next date".to_string(),
    };

    let sub = match stream.next_date {
        Some(next) => format!("{} · next {}", stream.frequency, fmt::short_date(next)),
        None => format!("{} · no next date", stream.frequency),
    };

    ViewStreamRow {
        id: stream.id.clone(),
        title: stream.label.clone(),
        logo_url: stream.logo_url.clone(),
        when,
        sub,
        soon: is_soon(stream, now),
        amount: fmt::money_signed(stream.amount, masked),
        frequency: stream.frequency.clone(),
    }
}

/// The stable id a category travels under.
///
/// Written out rather than derived from `Debug`, so renaming a variant cannot
/// silently change what a saved filter means.
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

/// Three months, with a mark on every day a stream lands.
///
/// Either side is drawn in full because turning the calendar is a scroll.
fn calendar(state: &StateApp, out: &[&Stream]) -> ViewCalendar {
    let now = state.now();

    let (year, month) = {
        let (year, month, _) = fmt::civil(now);
        step_month(year, month, state.month_step() as i64)
    };

    // Only the dates the source predicts — nothing is projected forward from a
    // frequency, so a stream contributes at most one mark.
    let mut marks: BTreeMap<u64, Vec<String>> = BTreeMap::new();

    for stream in out {
        let Some(next) = stream.next_date else {
            continue;
        };

        marks
            .entry(next.day_index())
            .or_default()
            .push(stream.category.token().to_string());
    }

    let today = now.day_index();
    let picked = state.picked_day();

    let month_view = |step: i64| {
        let (year, month) = step_month(year, month, step);
        let (start, rows) = month_grid(year, month);

        ViewCalendarMonth {
            label: format!(
                "{} {}",
                fmt::month_name(stamp(fmt::days_from_civil(year, month, 1))),
                year,
            ),
            weeks: (0..rows)
                .map(|week| {
                    (0..7)
                        .map(|column| {
                            let day = start + week * 7 + column;
                            let (_, its_month, its_day) = fmt::civil(stamp(day));

                            ViewCalendarDay {
                                id: day.to_string(),
                                label: its_day.to_string(),
                                outside: its_month != month,
                                today: day == today,
                                picked: picked == Some(day),
                                marks: marks.get(&day).cloned().unwrap_or_default(),
                            }
                        })
                        .collect()
                })
                .collect(),
        }
    };

    ViewCalendar {
        weekdays: WEEKDAY_INITIALS.iter().map(|d| d.to_string()).collect(),
        prev: month_view(-1),
        current: month_view(0),
        next: month_view(1),
    }
}

/// Where a month's grid starts, and how many rows it takes.
///
/// 1970-01-01 was a Thursday and weeks start on Monday, so the column a day
/// falls in is its index shifted by three.
fn month_grid(year: i64, month: u32) -> (u64, u64) {
    let first = fmt::days_from_civil(year, month, 1);

    let (next_year, next_month) = step_month(year, month, 1);
    let length = fmt::days_from_civil(next_year, next_month, 1).saturating_sub(first);

    let leading = (first + 3) % 7;

    (
        first.saturating_sub(leading),
        (leading + length).div_ceil(7),
    )
}

/// The month one step either side, carrying the year with it.
fn step_month(year: i64, month: u32, step: i64) -> (i64, u32) {
    let index = year * 12 + month as i64 - 1 + step;

    (index.div_euclid(12), index.rem_euclid(12) as u32 + 1)
}

/// The day at a day index, which is what `fmt` takes.
fn stamp(day: u64) -> Timestamp {
    Timestamp::from_secs(day * DAY)
}
