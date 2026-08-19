//! The state machine, tested the way `docs/ARCHITECTURE.md` says it should be:
//! a slice of events in, a `Vec<Action>` to assert on out. No runtime, no
//! await, no mocks.
//!
//! The snapshot these run against is the real captured Plaid Sandbox feed, so
//! what they pin is the app's behaviour on data it will actually see — the
//! same 48 transactions, 14 accounts and 8 streams the sketch was drawn from.

use api_core::*;

/// 2026-08-19 00:00 UTC — the day the sketch is drawn against, so "in 3 days"
/// and the rest line up with what it shows.
const NOW: u64 = 1_787_097_600;

/// The snapshot the server would send, built from the same fixtures it serves.
fn snapshot_json() -> String {
    let raw = include_str!("snapshot.json");

    // The fixture is captured with a fixed clock so the tests are not a
    // function of when they run.
    raw.replace("\"now\":0", &format!("\"now\":{NOW}"))
}

fn started() -> Vec<Event> {
    vec![
        Event::FromUi {
            event: UiEvent::Started { now: NOW },
        },
        Event::FromSource {
            event: SourceEvent::SnapshotLoaded {
                json: snapshot_json(),
                now: NOW,
            },
        },
    ]
}

fn run(events: &[Event]) -> (State, Vec<Action>) {
    let mut state = State::default();
    let mut actions = Vec::new();

    state.handle_events(events, &mut actions);

    (state, actions)
}

/// The last screen the view was told to draw.
fn last_screen(actions: &[Action]) -> ScreenView {
    actions
        .iter()
        .rev()
        .find_map(|a| match a {
            Action::ToUi {
                action: UiAction::Render { screen, .. },
            } => Some(screen.clone()),
            _ => None,
        })
        .expect("something should have been rendered")
}

fn last_dock(actions: &[Action]) -> DockView {
    actions
        .iter()
        .rev()
        .find_map(|a| match a {
            Action::ToUi {
                action: UiAction::Render { dock, .. },
            } => Some(dock.clone()),
            _ => None,
        })
        .expect("something should have been rendered")
}

fn ledger_of(actions: &[Action]) -> LedgerView {
    match last_screen(actions) {
        ScreenView::Ledger { view } => view,
        other => panic!("expected the ledger, got {other:?}"),
    }
}

fn open(screen: Screen) -> Event {
    Event::FromUi {
        event: UiEvent::ScreenOpened { screen },
    }
}

fn with(extra: &[Event]) -> (State, Vec<Action>) {
    let mut events = started();
    events.extend_from_slice(extra);
    run(&events)
}

// ── starting up ─────────────────────────────────────────────────────────────

#[test]
fn starting_asks_for_a_snapshot() {
    let (_, actions) = run(&[Event::FromUi {
        event: UiEvent::Started { now: NOW },
    }]);

    assert!(actions.contains(&Action::ToSource {
        action: SourceAction::LoadSnapshot { force: false },
    }));
}

#[test]
fn the_first_screen_is_home() {
    let (_, actions) = with(&[]);

    assert!(matches!(last_screen(&actions), ScreenView::Home { .. }));
}

/// Nothing is on screen until data arrives, and the wording has to say
/// "loading" rather than "nothing here".
#[test]
fn an_empty_app_says_it_is_loading() {
    let (_, actions) = run(&[
        Event::FromUi {
            event: UiEvent::Started { now: NOW },
        },
        open(Screen::Ledger),
    ]);

    assert_eq!(ledger_of(&actions).empty.as_deref(), Some("Loading…"));
}

#[test]
fn a_loaded_snapshot_fills_every_screen() {
    let (state, _) = with(&[]);

    // The real Sandbox feed.
    assert_eq!(state.entry_count(), 48);
    assert_eq!(state.connection_count(), 1);
    assert_eq!(state.stream_count(), 8);
}

// ── navigation ──────────────────────────────────────────────────────────────

#[test]
fn the_dock_lights_the_screen_it_is_on() {
    let (_, actions) = with(&[open(Screen::Recurring)]);

    let dock = last_dock(&actions);
    let lit: Vec<&DockItem> = dock.items.iter().filter(|i| i.selected).collect();

    assert_eq!(lit.len(), 1, "exactly one tab is lit");
    assert_eq!(lit[0].screen, Screen::Recurring);
}

/// One snapshot feeds every screen, so switching tabs must not go back to the
/// network. This is what makes the dock feel instant.
#[test]
fn switching_tabs_asks_for_nothing() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        open(Screen::Recurring),
        open(Screen::Balances),
    ]);

    let fetches = actions
        .iter()
        .filter(|a| matches!(a, Action::ToSource { .. }))
        .count();

    assert_eq!(fetches, 1, "only the initial load should fetch");
}

#[test]
fn every_screen_renders() {
    for screen in [
        Screen::Home,
        Screen::Ledger,
        Screen::Recurring,
        Screen::Balances,
    ] {
        let (_, actions) = with(&[open(screen)]);

        let view = last_screen(&actions);

        let matches = matches!(
            (screen, &view),
            (Screen::Home, ScreenView::Home { .. })
                | (Screen::Ledger, ScreenView::Ledger { .. })
                | (Screen::Recurring, ScreenView::Recurring { .. })
                | (Screen::Balances, ScreenView::Balances { .. })
        );

        assert!(matches, "{screen:?} rendered as {view:?}");
    }
}

#[test]
fn pull_to_refresh_forces_a_fetch() {
    let (_, actions) = with(&[Event::FromUi {
        event: UiEvent::RefreshRequested,
    }]);

    assert!(actions.contains(&Action::ToSource {
        action: SourceAction::LoadSnapshot { force: true },
    }));
}

// ── the ledger ──────────────────────────────────────────────────────────────

#[test]
fn the_ledger_groups_by_month_and_day() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    let ledger = ledger_of(&actions);

    assert!(!ledger.months.is_empty());

    // The feed runs May to August 2026.
    assert_eq!(ledger.months[0].name, "August");
    assert_eq!(ledger.months[0].year, "2026");

    // Newest day first, and the sketch's own heading.
    assert_eq!(ledger.months[0].days[0].heading, "Wed, Aug 12");

    // Every row lands in exactly one day.
    let rows: usize = ledger
        .months
        .iter()
        .flat_map(|m| &m.days)
        .map(|d| d.rows.len())
        .sum();

    assert_eq!(rows, 48);
}

#[test]
fn a_day_heading_never_repeats_within_a_month() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    for month in ledger_of(&actions).months {
        let mut headings: Vec<String> = month.days.iter().map(|d| d.heading.clone()).collect();
        let before = headings.len();

        headings.sort();
        headings.dedup();

        assert_eq!(headings.len(), before, "{} has a repeated day", month.name);
    }
}

/// The row the sketch calls out: Plaid reports −500.00 for the United Airlines
/// refund, and the ledger shows it as money arriving.
#[test]
fn a_refund_reads_as_money_arriving() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    let ledger = ledger_of(&actions);
    let row = ledger
        .months
        .iter()
        .flat_map(|m| &m.days)
        .flat_map(|d| &d.rows)
        .find(|r| r.title == "United Airlines" && r.is_inflow)
        .expect("the feed has a United Airlines refund");

    assert_eq!(row.amount, "+$500.00");
}

#[test]
fn a_purchase_reads_as_money_leaving() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    let ledger = ledger_of(&actions);
    let row = ledger
        .months
        .iter()
        .flat_map(|m| &m.days)
        .flat_map(|d| &d.rows)
        .find(|r| r.title == "McDonald's")
        .expect("the feed has a McDonald's");

    assert_eq!(row.amount, "−$12.00");
    assert!(!row.is_inflow);
}

// ── filtering ───────────────────────────────────────────────────────────────

fn toggle(id: &str) -> Event {
    Event::FromUi {
        event: UiEvent::CategoryToggled { id: id.to_string() },
    }
}

fn rows(ledger: &LedgerView) -> Vec<&TxRow> {
    ledger
        .months
        .iter()
        .flat_map(|m| &m.days)
        .flat_map(|d| &d.rows)
        .collect()
}

#[test]
fn selecting_a_category_narrows_the_list() {
    let (_, actions) = with(&[open(Screen::Ledger), toggle("FoodAndDrink")]);

    let ledger = ledger_of(&actions);
    let shown = rows(&ledger);

    // Nine food transactions in the captured feed.
    assert_eq!(shown.len(), 9);
    assert!(shown.iter().all(|r| r.category == "Food & drink"));
}

/// Multi-select: the chips are a union, not a replacement.
#[test]
fn two_categories_show_both() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        toggle("FoodAndDrink"),
        toggle("Travel"),
    ]);

    let ledger = ledger_of(&actions);

    // 9 food + 6 travel.
    assert_eq!(rows(&ledger).len(), 15);
}

#[test]
fn tapping_a_selected_category_deselects_it() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        toggle("FoodAndDrink"),
        toggle("FoodAndDrink"),
    ]);

    let ledger = ledger_of(&actions);

    assert_eq!(rows(&ledger).len(), 48, "back to everything");
    assert!(!ledger.filter.active);
}

/// The counts on the chips have to say what selecting *that* chip would give.
/// Computing them through the current selection would read zero for every
/// unselected chip, which is the bug this pins.
#[test]
fn the_chip_counts_ignore_the_category_selection() {
    let (_, actions) = with(&[open(Screen::Ledger), toggle("FoodAndDrink")]);

    let ledger = ledger_of(&actions);

    let travel = ledger
        .filter
        .categories
        .iter()
        .find(|c| c.id == "Travel")
        .expect("travel is always a chip");

    assert_eq!(travel.count, "6", "still says what travel would give");
    assert!(!travel.selected);
}

/// Every one of Plaid's sixteen primaries is offered, even the ones with
/// nothing in them — they read as unavailable rather than vanishing, so the
/// grid does not reflow as data changes.
#[test]
fn all_sixteen_categories_are_offered() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    let chips = ledger_of(&actions).filter.categories;

    assert_eq!(chips.len(), 16);
    assert!(chips.iter().any(|c| c.empty), "some have no transactions");
    assert!(chips.iter().any(|c| !c.empty));
}

#[test]
fn the_counts_match_the_feed() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    let chips = ledger_of(&actions).filter.categories;
    let count = |id: &str| {
        chips
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.count.clone())
            .unwrap_or_default()
    };

    // The numbers the sketch's filter grid shows.
    assert_eq!(count("Transportation"), "9");
    assert_eq!(count("FoodAndDrink"), "9");
    assert_eq!(count("Travel"), "6");
    assert_eq!(count("LoanPayments"), "6");
    assert_eq!(count("TransferOut"), "6");
    assert_eq!(count("Entertainment"), "0");
}

#[test]
fn a_shut_filter_summarises_what_it_is_doing() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        toggle("FoodAndDrink"),
        toggle("Travel"),
        toggle("Transportation"),
        toggle("LoanPayments"),
    ]);

    let filter = ledger_of(&actions).filter;

    assert_eq!(filter.categories_summary.len(), 2, "two named");
    assert_eq!(filter.categories_overflow.as_deref(), Some("+2"));
}

#[test]
fn clearing_categories_shows_everything_again() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        toggle("FoodAndDrink"),
        Event::FromUi {
            event: UiEvent::CategoriesCleared,
        },
    ]);

    assert_eq!(rows(&ledger_of(&actions)).len(), 48);
}

/// A category id the view invented should narrow nothing, rather than
/// filtering to a category no transaction is in.
#[test]
fn an_unknown_category_id_is_ignored() {
    let (_, actions) = with(&[open(Screen::Ledger), toggle("Crypto")]);

    let ledger = ledger_of(&actions);

    assert_eq!(rows(&ledger).len(), 48);
    assert!(!ledger.filter.active);
}

#[test]
fn a_filter_matching_nothing_says_so() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        // Nothing in the feed is medical.
        toggle("Medical"),
    ]);

    let ledger = ledger_of(&actions);

    assert!(rows(&ledger).is_empty());
    assert_eq!(
        ledger.empty.as_deref(),
        Some("Nothing matches this filter."),
        "and it must not say the ledger is empty",
    );
}

// ── the time range ──────────────────────────────────────────────────────────

fn range(kind: RangeSelection) -> Event {
    Event::FromUi {
        event: UiEvent::RangeSelected { kind },
    }
}

#[test]
fn this_month_keeps_only_august() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        range(RangeSelection::This {
            unit: RangeUnit::Month,
        }),
    ]);

    let ledger = ledger_of(&actions);

    assert_eq!(ledger.months.len(), 1);
    assert_eq!(ledger.months[0].name, "August");
    assert_eq!(ledger.filter.range_summary, "This month");
}

#[test]
fn the_last_three_months_reach_back_further() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        range(RangeSelection::Last {
            count: 3,
            unit: RangeUnit::Month,
        }),
    ]);

    let ledger = ledger_of(&actions);

    assert!(ledger.months.len() > 1);
    assert_eq!(ledger.filter.range_summary, "Last 3 months");
}

/// The end of a `Between` range is a day, and a day includes its own
/// transactions. Off by one here silently drops the last day.
#[test]
fn a_between_range_includes_its_last_day() {
    // Aug 12 is the newest day in the feed, with two transactions on it.
    let aug_12 = 1_786_492_800;

    let (_, actions) = with(&[
        open(Screen::Ledger),
        range(RangeSelection::Between {
            start: aug_12,
            end: aug_12,
        }),
    ]);

    let ledger = ledger_of(&actions);

    assert_eq!(rows(&ledger).len(), 2, "both of Aug 12's rows");
    assert_eq!(ledger.months[0].days[0].heading, "Wed, Aug 12");
}

#[test]
fn all_time_is_the_default_and_filters_nothing() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    let ledger = ledger_of(&actions);

    assert_eq!(ledger.filter.range.kind, RangeKind::All);
    assert!(!ledger.filter.active);
    assert_eq!(rows(&ledger).len(), 48);
}

/// The two filters are independent, so they compose rather than override.
#[test]
fn a_category_and_a_range_apply_together() {
    let (_, actions) = with(&[
        open(Screen::Ledger),
        toggle("FoodAndDrink"),
        range(RangeSelection::This {
            unit: RangeUnit::Month,
        }),
    ]);

    let ledger = ledger_of(&actions);
    let shown = rows(&ledger);

    assert!(shown.iter().all(|r| r.category == "Food & drink"));
    assert_eq!(ledger.months.len(), 1, "August only");
    assert!(shown.len() < 9, "fewer than food across all time");
}

/// The chip counts respect the range even while ignoring the category
/// selection — the two filters are independent in both directions.
#[test]
fn the_chip_counts_follow_the_time_range() {
    let (_, all_time) = with(&[open(Screen::Ledger)]);
    let (_, this_month) = with(&[
        open(Screen::Ledger),
        range(RangeSelection::This {
            unit: RangeUnit::Month,
        }),
    ]);

    let count = |actions: &[Action]| {
        ledger_of(actions)
            .filter
            .categories
            .iter()
            .find(|c| c.id == "FoodAndDrink")
            .expect("food is a chip")
            .count
            .clone()
    };

    assert_eq!(count(&all_time), "9");
    assert_ne!(count(&this_month), "9", "August alone holds fewer");
}

// ── masking ─────────────────────────────────────────────────────────────────

fn balances_of(actions: &[Action]) -> BalancesView {
    match last_screen(actions) {
        ScreenView::Balances { view } => view,
        other => panic!("expected balances, got {other:?}"),
    }
}

/// Covered by default, so the app can be opened in public without a decision.
#[test]
fn figures_start_covered() {
    let (_, actions) = with(&[open(Screen::Balances)]);

    let balances = balances_of(&actions);

    assert!(balances.masked);
    assert!(
        !balances.headline.contains('8'),
        "the figure must not travel in the clear: {}",
        balances.headline,
    );
}

#[test]
fn revealing_shows_the_real_figure() {
    let (_, actions) = with(&[
        open(Screen::Balances),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let balances = balances_of(&actions);

    assert!(!balances.masked);
    assert_eq!(balances.headline, "$86,541.74");
}

/// One switch for the whole app: revealing on Balances and staying covered on
/// Home would be a strange thing to have to explain.
#[test]
fn the_mask_is_one_switch_across_screens() {
    let (_, actions) = with(&[
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
        open(Screen::Home),
    ]);

    match last_screen(&actions) {
        ScreenView::Home { view } => {
            assert!(!view.balances.masked);
            assert_eq!(view.balances.headline, "$86,541.74");
        }
        other => panic!("expected home, got {other:?}"),
    }
}

/// Ledger rows are never covered — a list of grey bars says nothing, and the
/// figure worth hiding is the total rather than a $4.33 coffee.
#[test]
fn the_mask_leaves_ledger_rows_alone() {
    let (_, actions) = with(&[open(Screen::Ledger)]);

    let ledger = ledger_of(&actions);

    assert!(rows(&ledger).iter().any(|r| r.amount.contains('$')));
}

// ── balances ────────────────────────────────────────────────────────────────

#[test]
fn balances_lead_with_what_is_held() {
    let (_, actions) = with(&[
        open(Screen::Balances),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let balances = balances_of(&actions);

    assert_eq!(balances.headline_label, "Held");
    assert_eq!(balances.headline, "$86,541.74");
    assert_eq!(balances.subtitle, "1 connection · 14 accounts");
}

/// A mortgage between a checking account and an HSA reads as something you
/// have. Debts get their own block, after everything held.
#[test]
fn debts_are_split_out_and_come_last() {
    let (_, actions) = with(&[
        open(Screen::Balances),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let connections = balances_of(&actions).connections;

    let first_debt = connections.iter().position(|c| c.is_debts);
    let last_held = connections.iter().rposition(|c| !c.is_debts);

    assert!(first_debt.is_some(), "the feed has debts");
    assert!(
        first_debt > last_held,
        "every debt block comes after every held one",
    );

    let debts = connections.iter().find(|c| c.is_debts).unwrap();

    assert_eq!(debts.accounts.len(), 6);
    assert_eq!(debts.total, "−$163,705.89");
    assert_eq!(debts.total_label, "6 accounts owed");
    assert!(debts.accounts.iter().all(|a| a.owed));
}

#[test]
fn a_credit_card_shows_what_is_used_of_its_limit() {
    let (_, actions) = with(&[
        open(Screen::Balances),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let balances = balances_of(&actions);
    let row = balances
        .connections
        .iter()
        .flat_map(|c| &c.accounts)
        .find(|a| a.name == "Plaid Credit Card")
        .expect("the feed has a credit card");

    assert_eq!(row.amount, "−$410.00");
    assert_eq!(
        row.utilisation.as_deref(),
        Some("$410.00 of $2,000.00 limit used"),
    );
}

/// A HELOC's useful half is how much room is left, not how much is gone.
#[test]
fn a_line_of_credit_shows_what_is_left() {
    let (_, actions) = with(&[
        open(Screen::Balances),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let balances = balances_of(&actions);
    let row = balances
        .connections
        .iter()
        .flat_map(|c| &c.accounts)
        .find(|a| a.name.contains("Home Equity"))
        .expect("the feed has a HELOC");

    assert_eq!(
        row.utilisation.as_deref(),
        Some("$47,647.50 still available")
    );
}

#[test]
fn an_account_subtitle_reads_as_a_sentence() {
    let (_, actions) = with(&[
        open(Screen::Balances),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let balances = balances_of(&actions);
    let accounts: Vec<&AccountRow> = balances
        .connections
        .iter()
        .flat_map(|c| &c.accounts)
        .collect();

    let checking = accounts
        .iter()
        .find(|a| a.name == "Plaid Checking")
        .unwrap();
    assert_eq!(checking.subtitle, "Checking ·0000 · $100.00 available");

    let savings = accounts.iter().find(|a| a.name == "Plaid Saving").unwrap();
    assert_eq!(savings.subtitle, "Savings ·1111 · $200.00 available");

    // Plaid's `401k` is not what a person calls it.
    let retirement = accounts.iter().find(|a| a.name == "Plaid 401k").unwrap();
    assert_eq!(retirement.subtitle, "401(k) ·6666");
}

// ── recurring ───────────────────────────────────────────────────────────────

fn recurring_of(actions: &[Action]) -> RecurringView {
    match last_screen(actions) {
        ScreenView::Recurring { view } => view,
        other => panic!("expected recurring, got {other:?}"),
    }
}

#[test]
fn recurring_leads_with_the_monthly_commitment() {
    let (_, actions) = with(&[
        open(Screen::Recurring),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let recurring = recurring_of(&actions);

    assert_eq!(recurring.headline, "−$3,771.40");
    assert_eq!(recurring.subtitle, "8 streams · 7 out, 1 in");
    assert_eq!(recurring.detail, "+$4.22 in · net −$3,767.18");
}

/// The bar and the legend are two views of one split, so the shares have to
/// add up to the whole bar.
#[test]
fn the_breakdown_covers_the_whole_bar() {
    let (_, actions) = with(&[open(Screen::Recurring)]);

    let breakdown = recurring_of(&actions).breakdown;

    assert!(!breakdown.is_empty());

    let total: f64 = breakdown.iter().map(|s| s.share).sum();

    assert!((total - 1.0).abs() < 1e-9, "shares sum to {total}");

    // Biggest first, so the eye lands on what matters.
    for pair in breakdown.windows(2) {
        assert!(pair[0].share >= pair[1].share);
    }
}

#[test]
fn a_stream_says_when_it_next_lands() {
    let (_, actions) = with(&[
        open(Screen::Recurring),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
    ]);

    let recurring = recurring_of(&actions);
    let row = recurring
        .groups
        .iter()
        .flat_map(|g| &g.streams)
        .find(|s| s.title == "AUTOMATIC PAYMENT - THANK")
        .expect("the feed has this stream");

    assert_eq!(row.when, "next Aug 22 · in 3 days");
    assert!(row.soon, "three days out is a heads-up");
    assert_eq!(row.amount, "−$2,078.50");
    assert_eq!(row.frequency, "Monthly");
}

#[test]
fn a_distant_stream_is_not_marked_soon() {
    let (_, actions) = with(&[open(Screen::Recurring)]);

    let recurring = recurring_of(&actions);
    let row = recurring
        .groups
        .iter()
        .flat_map(|g| &g.streams)
        .find(|s| s.title == "Touchstone Climbing")
        .expect("the feed has this stream");

    assert!(!row.soon, "three weeks out is not urgent");
}

#[test]
fn money_coming_in_is_grouped_after_everything_going_out() {
    let (_, actions) = with(&[open(Screen::Recurring)]);

    let groups = recurring_of(&actions).groups;

    let inflow = groups
        .iter()
        .position(|g| g.streams.iter().any(|s| s.title == "INTRST PYMNT"))
        .expect("the feed has one inflow");

    assert_eq!(inflow, groups.len() - 1, "the inflow group comes last");
}

// ── home ────────────────────────────────────────────────────────────────────

fn home_of(actions: &[Action]) -> HomeView {
    match last_screen(actions) {
        ScreenView::Home { view } => view,
        other => panic!("expected home, got {other:?}"),
    }
}

#[test]
fn home_previews_each_tab() {
    let (_, actions) = with(&[
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
        open(Screen::Home),
    ]);

    let home = home_of(&actions);

    assert_eq!(home.balances.headline, "$86,541.74");
    assert_eq!(home.balances.detail, "held across 14 accounts");
    assert_eq!(home.recurring.headline, "−$3,771.40");
    assert_eq!(home.recurring.detail, "3 due within the week");

    assert_eq!(home.ledger.len(), 4, "just the latest few");
    assert!(!home.recurring.breakdown.is_empty());
}

/// The preview is only useful if it shows what lands next.
#[test]
fn home_previews_the_soonest_streams_first() {
    let (_, actions) = with(&[open(Screen::Home)]);

    let upcoming = home_of(&actions).recurring.upcoming;

    assert_eq!(upcoming.len(), 2);
    assert!(upcoming.iter().all(|s| s.soon), "both land within the week");
    assert_eq!(upcoming[0].when, "next Aug 22 · in 3 days");
}

/// A healthy connection is not something to put on the home screen.
#[test]
fn home_says_nothing_about_connections_that_are_fine() {
    let (_, actions) = with(&[open(Screen::Home)]);

    assert!(home_of(&actions).attention.is_empty());
}

// ── failure ─────────────────────────────────────────────────────────────────

#[test]
fn a_failed_load_tells_the_user_and_still_draws() {
    let (_, actions) = run(&[
        Event::FromUi {
            event: UiEvent::Started { now: NOW },
        },
        Event::FromSource {
            event: SourceEvent::LoadFailed {
                message: "the server is not running".to_string(),
            },
        },
    ]);

    assert!(actions.iter().any(|a| matches!(
        a,
        Action::ToUi {
            action: UiAction::ShowError { .. }
        }
    )));

    // And something is still on screen.
    assert!(matches!(last_screen(&actions), ScreenView::Home { .. }));
}

/// Whatever was already loaded stays on screen: a dropped refresh should not
/// empty the app.
#[test]
fn a_failure_after_a_good_load_keeps_the_data() {
    let (state, actions) = with(&[
        Event::FromSource {
            event: SourceEvent::LoadFailed {
                message: "network unreachable".to_string(),
            },
        },
        open(Screen::Ledger),
    ]);

    assert_eq!(state.entry_count(), 48);
    assert!(
        ledger_of(&actions).empty.is_none(),
        "the list still has rows"
    );
}

/// A snapshot we cannot parse is a failed load, not a panic.
#[test]
fn an_unreadable_snapshot_does_not_crash() {
    let (state, actions) = run(&[
        Event::FromUi {
            event: UiEvent::Started { now: NOW },
        },
        Event::FromSource {
            event: SourceEvent::SnapshotLoaded {
                json: "{ not json at all".to_string(),
                now: NOW,
            },
        },
    ]);

    assert_eq!(state.entry_count(), 0);
    assert!(matches!(last_screen(&actions), ScreenView::Home { .. }));
}

// ── the property the whole design rests on ──────────────────────────────────

/// The reason for all of this: if every decision is a pure function of state
/// and an event, a log of the events is a recording of the session. Replay it
/// and the same thing happens.
#[test]
fn the_same_events_give_the_same_actions() {
    let mut events = started();

    events.extend([
        open(Screen::Ledger),
        toggle("FoodAndDrink"),
        range(RangeSelection::This {
            unit: RangeUnit::Month,
        }),
        Event::FromUi {
            event: UiEvent::MaskToggled,
        },
        open(Screen::Balances),
        open(Screen::Recurring),
        Event::FromUi {
            event: UiEvent::FilterToggled,
        },
    ]);

    let (_, first) = run(&events);
    let (_, second) = run(&events);

    assert_eq!(first, second);
}

/// Nothing in the machine reads a clock, so the same events an hour apart still
/// land on the same screen.
#[test]
fn the_machine_does_not_depend_on_when_it_runs() {
    let (_, first) = with(&[open(Screen::Recurring)]);

    std::thread::sleep(std::time::Duration::from_millis(20));

    let (_, second) = with(&[open(Screen::Recurring)]);

    assert_eq!(first, second);
}

/// Every event renders, so the view never has to guess whether to redraw.
#[test]
fn every_ui_event_produces_a_render() {
    for event in [
        UiEvent::ScreenOpened {
            screen: Screen::Ledger,
        },
        UiEvent::MaskToggled,
        UiEvent::FilterToggled,
        UiEvent::FilterGroupToggled {
            group: FilterGroup::Categories,
        },
        UiEvent::CategoryToggled {
            id: "Travel".to_string(),
        },
        UiEvent::CategoriesCleared,
        UiEvent::RangeSelected {
            kind: RangeSelection::All,
        },
        UiEvent::TxSelected {
            id: "whatever".to_string(),
        },
        UiEvent::RefreshRequested,
    ] {
        let (_, actions) = with(&[Event::FromUi {
            event: event.clone(),
        }]);

        let renders = actions
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    Action::ToUi {
                        action: UiAction::Render { .. }
                    }
                )
            })
            .count();

        assert!(renders >= 2, "{event:?} should have rendered");
    }
}
