//! The app's state machine, tested the way `docs/STYLE.md` says: events in
//! with `<`, actions out with `>`, and an `insta` snapshot per batch.
//!
//! The snapshot these run against is the real captured Plaid Sandbox feed, so
//! what they pin is the app's behaviour on data it will actually see — the same
//! 48 transactions, 14 accounts and 8 streams the sketch was drawn from.
//!
//! `eval!` covers the *actions*; the assertions after it cover the screens the
//! actions carry, which are too large to read as a snapshot.

use api_common::Timestamp;
use api_eval::eval;
use phone_state::*;

/// 2026-08-19 00:00 UTC — the day the sketch is drawn against, so "in 3 days"
/// and the rest line up with what it shows.
const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

/// A machine with its clock set, ready for `eval!` — which reads the time from
/// the machine rather than carrying one of its own.
fn at_now() -> StateApp {
    let mut state = StateApp::new();
    state.set_now(NOW);
    state
}

/// The snapshot the server would send, built from the same fixtures it serves.
fn snapshot_json() -> String {
    // The fixture is captured with `now: 0`; the tests substitute a fixed clock
    // so nothing about them depends on when they run.
    include_str!("snapshot.json").replace("\"now\": 0", &format!("\"now\": {}", NOW.as_secs()))
}

fn loaded() -> Event {
    Event::FromSource {
        event: EventSource::SnapshotLoaded {
            json: snapshot_json(),
        },
    }
}

fn open(screen: Screen) -> Event {
    Event::FromUi {
        event: EventUi::ScreenOpened { screen },
    }
}

fn ui(event: EventUi) -> Event {
    Event::FromUi { event }
}

fn toggle(id: &str) -> Event {
    ui(EventUi::CategoryToggled { id: id.to_string() })
}

fn range(kind: RangeSelection) -> Event {
    ui(EventUi::RangeSelected { kind })
}

/// A machine with the fixture already loaded, and the screen it rendered.
///
/// Most tests care about what a screen *says* rather than which actions fired,
/// so this hands back the view and leaves `eval!` to the tests that are about
/// the actions themselves.
fn loaded_on(screen: Screen) -> (StateApp, ViewScreen) {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    state.handle_events(
        NOW,
        &[ui(EventUi::Started), loaded(), open(screen)],
        &mut actions,
    );

    let view = last_render(&actions);

    (state, view)
}

/// The same, with the figures revealed.
fn revealed_on(screen: Screen) -> ViewScreen {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    state.handle_events(
        NOW,
        &[
            ui(EventUi::Started),
            loaded(),
            ui(EventUi::MaskToggled),
            open(screen),
        ],
        &mut actions,
    );

    last_render(&actions)
}

fn last_render(actions: &[Action]) -> ViewScreen {
    actions
        .iter()
        .rev()
        .find_map(|a| match a {
            Action::ToUi {
                action: ActionUi::Render { screen, .. },
            } => Some(screen.clone()),
            _ => None,
        })
        .expect("something should have been rendered")
}

fn ledger_of(view: &ViewScreen) -> &ViewLedger {
    match view {
        ViewScreen::Ledger { view } => view,
        other => panic!("expected the ledger, got {other:?}"),
    }
}

fn rows(ledger: &ViewLedger) -> Vec<&ViewTxRow> {
    ledger
        .months
        .iter()
        .flat_map(|m| &m.days)
        .flat_map(|d| &d.rows)
        .collect()
}

#[test]
fn starting_asks_for_a_snapshot() {
    let mut state = at_now();

    eval!(&mut state,
        < ui(EventUi::Started),
        @r#"
    [1] ToSource {
        action: LoadSnapshot {
            force: false,
        },
    }
    [1] ToUi {
        action: Render {
            screen: Home {
                view: ViewHome {
                    balances: ViewHomeBalances {
                        headline: "••••••",
                        masked: true,
                        detail: "held across 0 accounts",
                    },
                    recurring: ViewHomeRecurring {
                        headline: "••••••",
                        masked: true,
                        detail: "nothing due this week",
                        breakdown: [],
                        upcoming: [],
                    },
                    ledger: [],
                    attention: [],
                },
            },
            dock: ViewDock {
                items: [
                    ViewDockItem {
                        screen: Home,
                        label: "Home",
                        icon: "home",
                        selected: true,
                    },
                    ViewDockItem {
                        screen: Ledger,
                        label: "Ledger",
                        icon: "ledger",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Recurring,
                        label: "Recurring",
                        icon: "recurring",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Balances,
                        label: "Balances",
                        icon: "balances",
                        selected: false,
                    },
                ],
            },
        },
    }
    "#,
    );
}

#[test]
fn pull_to_refresh_forces_a_fetch() {
    let mut state = at_now();

    eval!(&mut state,
        < ui(EventUi::RefreshRequested),
        @r#"
    [1] ToSource {
        action: LoadSnapshot {
            force: true,
        },
    }
    [1] ToUi {
        action: Render {
            screen: Home {
                view: ViewHome {
                    balances: ViewHomeBalances {
                        headline: "••••••",
                        masked: true,
                        detail: "held across 0 accounts",
                    },
                    recurring: ViewHomeRecurring {
                        headline: "••••••",
                        masked: true,
                        detail: "nothing due this week",
                        breakdown: [],
                        upcoming: [],
                    },
                    ledger: [],
                    attention: [],
                },
            },
            dock: ViewDock {
                items: [
                    ViewDockItem {
                        screen: Home,
                        label: "Home",
                        icon: "home",
                        selected: true,
                    },
                    ViewDockItem {
                        screen: Ledger,
                        label: "Ledger",
                        icon: "ledger",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Recurring,
                        label: "Recurring",
                        icon: "recurring",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Balances,
                        label: "Balances",
                        icon: "balances",
                        selected: false,
                    },
                ],
            },
        },
    }
    "#,
    );
}

/// One snapshot feeds every screen, so switching tabs must not go back to the
/// network. This is what makes the dock feel instant.
#[test]
fn switching_tabs_asks_for_nothing() {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    state.handle_events(
        NOW,
        &[
            ui(EventUi::Started),
            loaded(),
            open(Screen::Ledger),
            open(Screen::Recurring),
            open(Screen::Balances),
        ],
        &mut actions,
    );

    let fetches = actions
        .iter()
        .filter(|a| matches!(a, Action::ToSource { .. }))
        .count();

    assert_eq!(fetches, 1, "only the initial load should fetch");
}

#[test]
fn a_loaded_snapshot_fills_every_screen() {
    let (state, _) = loaded_on(Screen::Home);

    assert_eq!(state.entry_count(), 48);
    assert_eq!(state.connection_count(), 1);
    assert_eq!(state.stream_count(), 8);
}

#[test]
fn every_screen_renders() {
    for screen in [
        Screen::Home,
        Screen::Ledger,
        Screen::Recurring,
        Screen::Balances,
    ] {
        let (_, view) = loaded_on(screen);

        let matches = matches!(
            (screen, &view),
            (Screen::Home, ViewScreen::Home { .. })
                | (Screen::Ledger, ViewScreen::Ledger { .. })
                | (Screen::Recurring, ViewScreen::Recurring { .. })
                | (Screen::Balances, ViewScreen::Balances { .. })
        );

        assert!(matches, "{screen:?} rendered as {view:?}");
    }
}

#[test]
fn the_ledger_groups_by_month_and_day() {
    let (_, view) = loaded_on(Screen::Ledger);
    let ledger = ledger_of(&view);

    // The feed runs May to August 2026.
    assert_eq!(ledger.months[0].name, "August");
    assert_eq!(ledger.months[0].year, "2026");

    // Newest day first, and the sketch's own heading.
    assert_eq!(ledger.months[0].days[0].heading, "Wed, Aug 12");

    assert_eq!(rows(ledger).len(), 48, "every row lands in exactly one day");
}

#[test]
fn a_day_heading_never_repeats_within_a_month() {
    let (_, view) = loaded_on(Screen::Ledger);

    for month in &ledger_of(&view).months {
        let mut headings: Vec<&str> = month.days.iter().map(|d| d.heading.as_str()).collect();
        let before = headings.len();

        headings.sort_unstable();
        headings.dedup();

        assert_eq!(headings.len(), before, "{} has a repeated day", month.name);
    }
}

/// The row the sketch calls out: Plaid reports −500.00 for the United Airlines
/// refund, and the ledger shows it as money arriving.
#[test]
fn a_refund_reads_as_money_arriving() {
    let (_, view) = loaded_on(Screen::Ledger);
    let ledger = ledger_of(&view);

    let row = rows(ledger)
        .into_iter()
        .find(|r| r.title == "United Airlines" && r.is_inflow)
        .expect("the feed has a United Airlines refund");

    assert_eq!(row.amount, "+$500.00");
}

#[test]
fn a_purchase_reads_as_money_leaving() {
    let (_, view) = loaded_on(Screen::Ledger);
    let ledger = ledger_of(&view);

    let row = rows(ledger)
        .into_iter()
        .find(|r| r.title == "McDonald's")
        .expect("the feed has a McDonald's");

    assert_eq!(row.amount, "−$12.00");
    assert!(!row.is_inflow);
}

/// Filtering is where most of the machine's decisions live, so these drive it
/// through `eval!` and then read the screen the last action carried.
fn filtered(events: Vec<Event>) -> ViewLedger {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    let mut all = vec![ui(EventUi::Started), loaded(), open(Screen::Ledger)];
    all.extend(events);

    state.handle_events(NOW, &all, &mut actions);

    match last_render(&actions) {
        ViewScreen::Ledger { view } => view,
        other => panic!("expected the ledger, got {other:?}"),
    }
}

#[test]
fn selecting_a_category_narrows_the_list() {
    let ledger = filtered(vec![toggle("FoodAndDrink")]);
    let shown = rows(&ledger);

    // Nine food transactions in the captured feed.
    assert_eq!(shown.len(), 9);
    assert!(shown.iter().all(|r| r.category == "Food & drink"));
}

/// Multi-select: the chips are a union, not a replacement.
#[test]
fn two_categories_show_both() {
    let ledger = filtered(vec![toggle("FoodAndDrink"), toggle("Travel")]);

    // 9 food + 6 travel.
    assert_eq!(rows(&ledger).len(), 15);
}

#[test]
fn tapping_a_selected_category_deselects_it() {
    let ledger = filtered(vec![toggle("FoodAndDrink"), toggle("FoodAndDrink")]);

    assert_eq!(rows(&ledger).len(), 48, "back to everything");
    assert!(!ledger.filter.active);
}

/// The counts on the chips have to say what selecting *that* chip would give.
/// Computing them through the current selection would read zero for every
/// unselected chip, which is the bug this pins.
#[test]
fn the_chip_counts_ignore_the_category_selection() {
    let ledger = filtered(vec![toggle("FoodAndDrink")]);

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
    let ledger = filtered(vec![]);
    let chips = &ledger.filter.categories;

    assert_eq!(chips.len(), 16);
    assert!(chips.iter().any(|c| c.empty), "some have no transactions");
    assert!(chips.iter().any(|c| !c.empty));
}

#[test]
fn the_counts_match_the_feed() {
    let ledger = filtered(vec![]);

    let count = |id: &str| {
        ledger
            .filter
            .categories
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.count.as_str())
            .unwrap_or_default()
            .to_string()
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
    let ledger = filtered(vec![
        toggle("FoodAndDrink"),
        toggle("Travel"),
        toggle("Transportation"),
        toggle("LoanPayments"),
    ]);

    assert_eq!(ledger.filter.categories_summary.len(), 2, "two named");
    assert_eq!(ledger.filter.categories_overflow.as_deref(), Some("+2"));
}

#[test]
fn clearing_categories_shows_everything_again() {
    let ledger = filtered(vec![toggle("FoodAndDrink"), ui(EventUi::CategoriesCleared)]);

    assert_eq!(rows(&ledger).len(), 48);
}

/// A category id the view invented should narrow nothing, rather than
/// filtering to a category no transaction is in.
#[test]
fn an_unknown_category_id_is_ignored() {
    let ledger = filtered(vec![toggle("Crypto")]);

    assert_eq!(rows(&ledger).len(), 48);
    assert!(!ledger.filter.active);
}

#[test]
fn a_filter_matching_nothing_says_so() {
    // Nothing in the feed is medical.
    let ledger = filtered(vec![toggle("Medical")]);

    assert!(rows(&ledger).is_empty());
    assert_eq!(
        ledger.empty.as_deref(),
        Some("Nothing matches this filter."),
        "and it must not say the ledger is empty",
    );
}

#[test]
fn this_month_keeps_only_august() {
    let ledger = filtered(vec![range(RangeSelection::This {
        unit: RangeUnit::Month,
    })]);

    assert_eq!(ledger.months.len(), 1);
    assert_eq!(ledger.months[0].name, "August");
    assert_eq!(ledger.filter.range_summary, "This month");
}

#[test]
fn the_last_three_months_reach_back_further() {
    let ledger = filtered(vec![range(RangeSelection::Last {
        count: 3,
        unit: RangeUnit::Month,
    })]);

    assert!(ledger.months.len() > 1);
    assert_eq!(ledger.filter.range_summary, "Last 3 months");
}

/// The end of a `Between` range is a day, and a day includes its own
/// transactions. Off by one here silently drops the last day.
#[test]
fn a_between_range_includes_its_last_day() {
    // Aug 12 is the newest day in the feed, with two transactions on it.
    const AUG_12: u64 = 1_786_492_800;

    let ledger = filtered(vec![range(RangeSelection::Between {
        start: AUG_12,
        end: AUG_12,
    })]);

    assert_eq!(rows(&ledger).len(), 2, "both of Aug 12's rows");
    assert_eq!(ledger.months[0].days[0].heading, "Wed, Aug 12");
}

#[test]
fn all_time_is_the_default_and_filters_nothing() {
    let ledger = filtered(vec![]);

    assert_eq!(ledger.filter.range.kind, ViewRangeKind::All);
    assert!(!ledger.filter.active);
    assert_eq!(rows(&ledger).len(), 48);
}

/// The two filters are independent, so they compose rather than override.
#[test]
fn a_category_and_a_range_apply_together() {
    let ledger = filtered(vec![
        toggle("FoodAndDrink"),
        range(RangeSelection::This {
            unit: RangeUnit::Month,
        }),
    ]);

    let shown = rows(&ledger);

    assert!(shown.iter().all(|r| r.category == "Food & drink"));
    assert_eq!(ledger.months.len(), 1, "August only");
    assert!(shown.len() < 9, "fewer than food across all time");
}

/// The chip counts respect the range even while ignoring the category
/// selection — the two filters are independent in both directions.
#[test]
fn the_chip_counts_follow_the_time_range() {
    let count = |ledger: &ViewLedger| {
        ledger
            .filter
            .categories
            .iter()
            .find(|c| c.id == "FoodAndDrink")
            .expect("food is a chip")
            .count
            .clone()
    };

    let all_time = filtered(vec![]);
    let this_month = filtered(vec![range(RangeSelection::This {
        unit: RangeUnit::Month,
    })]);

    assert_eq!(count(&all_time), "9");
    assert_ne!(count(&this_month), "9", "August alone holds fewer");
}

fn balances_of(view: &ViewScreen) -> &ViewBalances {
    match view {
        ViewScreen::Balances { view } => view,
        other => panic!("expected balances, got {other:?}"),
    }
}

/// Covered by default, so the app can be opened in public without a decision.
#[test]
fn figures_start_covered() {
    let (_, view) = loaded_on(Screen::Balances);
    let balances = balances_of(&view);

    assert!(balances.masked);
    assert!(
        !balances.headline.contains('8'),
        "the figure must not travel in the clear: {}",
        balances.headline,
    );
}

#[test]
fn revealing_shows_the_real_figure() {
    let view = revealed_on(Screen::Balances);
    let balances = balances_of(&view);

    assert!(!balances.masked);
    assert_eq!(balances.headline, "$86,541.74");
}

/// One switch for the whole app: revealing on Balances and staying covered on
/// Home would be a strange thing to have to explain.
#[test]
fn the_mask_is_one_switch_across_screens() {
    match revealed_on(Screen::Home) {
        ViewScreen::Home { view } => {
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
    let (_, view) = loaded_on(Screen::Ledger);

    assert!(
        rows(ledger_of(&view))
            .iter()
            .any(|r| r.amount.contains('$'))
    );
}

#[test]
fn balances_lead_with_what_is_held() {
    let view = revealed_on(Screen::Balances);
    let balances = balances_of(&view);

    assert_eq!(balances.headline_label, "Held");
    assert_eq!(balances.headline, "$86,541.74");
    assert_eq!(balances.subtitle, "1 connection · 14 accounts");
}

/// A mortgage between a checking account and an HSA reads as something you
/// have. Debts get their own block, after everything held.
#[test]
fn debts_are_split_out_and_come_last() {
    let view = revealed_on(Screen::Balances);
    let connections = &balances_of(&view).connections;

    let first_debt = connections.iter().position(|c| c.is_debts);
    let last_held = connections.iter().rposition(|c| !c.is_debts);

    assert!(first_debt.is_some(), "the feed has debts");
    assert!(
        first_debt > last_held,
        "every debt block comes after every held one",
    );

    let debts = connections
        .iter()
        .find(|c| c.is_debts)
        .expect("just checked");

    assert_eq!(debts.accounts.len(), 6);
    assert_eq!(debts.total, "−$163,705.89");
    assert_eq!(debts.total_label, "6 accounts owed");
    assert!(debts.accounts.iter().all(|a| a.owed));
}

#[test]
fn a_credit_card_shows_what_is_used_of_its_limit() {
    let view = revealed_on(Screen::Balances);

    let row = balances_of(&view)
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
    let view = revealed_on(Screen::Balances);

    let row = balances_of(&view)
        .connections
        .iter()
        .flat_map(|c| &c.accounts)
        .find(|a| a.name.contains("Home Equity"))
        .expect("the feed has a HELOC");

    assert_eq!(
        row.utilisation.as_deref(),
        Some("$47,647.50 still available"),
    );
}

#[test]
fn an_account_subtitle_reads_as_a_sentence() {
    let view = revealed_on(Screen::Balances);

    let accounts: Vec<&ViewAccountRow> = balances_of(&view)
        .connections
        .iter()
        .flat_map(|c| &c.accounts)
        .collect();

    let subtitle = |name: &str| {
        accounts
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.subtitle.as_str())
            .unwrap_or_default()
            .to_string()
    };

    assert_eq!(
        subtitle("Plaid Checking"),
        "Checking ·0000 · $100.00 available"
    );
    assert_eq!(
        subtitle("Plaid Saving"),
        "Savings ·1111 · $200.00 available"
    );

    // Plaid's `401k` is not what a person calls it.
    assert_eq!(subtitle("Plaid 401k"), "401(k) ·6666");
}

fn recurring_of(view: &ViewScreen) -> &ViewRecurring {
    match view {
        ViewScreen::Recurring { view } => view,
        other => panic!("expected recurring, got {other:?}"),
    }
}

#[test]
fn recurring_leads_with_the_monthly_commitment() {
    let view = revealed_on(Screen::Recurring);
    let recurring = recurring_of(&view);

    assert_eq!(recurring.headline, "−$3,771.40");
    assert_eq!(recurring.subtitle, "8 streams · 7 out, 1 in");
    assert_eq!(recurring.detail, "+$4.22 in · net −$3,767.18");
}

/// The bar and the legend are two views of one split, so the shares have to add
/// up to the whole bar.
#[test]
fn the_breakdown_covers_the_whole_bar() {
    let (_, view) = loaded_on(Screen::Recurring);
    let breakdown = &recurring_of(&view).breakdown;

    assert!(!breakdown.is_empty());

    let total: f64 = breakdown.iter().map(|s| s.share).sum();

    assert!((total - 1.0).abs() < 1e-9, "shares sum to {total}");

    // Biggest first, so the eye lands on what matters.
    for pair in breakdown.windows(2) {
        assert!(pair[0].share >= pair[1].share);
    }
}

/// The bar and the list are two views of one split, so the groups have to run
/// in the same order the bar's segments do — biggest commitment first.
#[test]
fn stream_groups_run_biggest_first_like_the_bar() {
    let view = revealed_on(Screen::Recurring);
    let recurring = recurring_of(&view);

    // Outflow groups only: the inflow group is deliberately last regardless of
    // size, being a different kind of thing.
    let outflow: Vec<&ViewStreamGroup> = recurring
        .groups
        .iter()
        .filter(|g| g.streams.iter().any(|s| s.amount.starts_with('−')))
        .collect();

    assert_eq!(outflow[0].name, "Transfer out", "the $2,078.50 group leads");
    assert_eq!(outflow[0].total, "$2,078.50");

    let bar: Vec<&str> = recurring
        .breakdown
        .iter()
        .map(|s| s.label.as_str())
        .collect();
    let list: Vec<&str> = outflow.iter().map(|g| g.name.as_str()).collect();

    assert_eq!(bar, list, "the bar and the list must not disagree");
}

#[test]
fn a_stream_says_when_it_next_lands() {
    let view = revealed_on(Screen::Recurring);

    let row = recurring_of(&view)
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
    let (_, view) = loaded_on(Screen::Recurring);

    let row = recurring_of(&view)
        .groups
        .iter()
        .flat_map(|g| &g.streams)
        .find(|s| s.title == "Touchstone Climbing")
        .expect("the feed has this stream");

    assert!(!row.soon, "three weeks out is not urgent");
}

#[test]
fn money_coming_in_is_grouped_after_everything_going_out() {
    let (_, view) = loaded_on(Screen::Recurring);
    let groups = &recurring_of(&view).groups;

    let inflow = groups
        .iter()
        .position(|g| g.streams.iter().any(|s| s.title == "INTRST PYMNT"))
        .expect("the feed has one inflow");

    assert_eq!(inflow, groups.len() - 1, "the inflow group comes last");
}

#[test]
fn home_previews_each_tab() {
    match revealed_on(Screen::Home) {
        ViewScreen::Home { view } => {
            assert_eq!(view.balances.headline, "$86,541.74");
            assert_eq!(view.balances.detail, "held across 14 accounts");
            assert_eq!(view.recurring.headline, "−$3,771.40");
            assert_eq!(view.recurring.detail, "3 due within the week");

            assert_eq!(view.ledger.len(), 4, "just the latest few");
            assert!(!view.recurring.breakdown.is_empty());
        }
        other => panic!("expected home, got {other:?}"),
    }
}

/// The preview is only useful if it shows what lands next.
#[test]
fn home_previews_the_soonest_streams_first() {
    let (_, view) = loaded_on(Screen::Home);

    match view {
        ViewScreen::Home { view } => {
            let upcoming = &view.recurring.upcoming;

            assert_eq!(upcoming.len(), 2);
            assert!(upcoming.iter().all(|s| s.soon), "both land within the week");
            assert_eq!(upcoming[0].when, "next Aug 22 · in 3 days");
        }
        other => panic!("expected home, got {other:?}"),
    }
}

/// A healthy connection is not something to put on the home screen.
#[test]
fn home_says_nothing_about_connections_that_are_fine() {
    let (_, view) = loaded_on(Screen::Home);

    match view {
        ViewScreen::Home { view } => assert!(view.attention.is_empty()),
        other => panic!("expected home, got {other:?}"),
    }
}

/// Nothing is on screen until data arrives, and the wording has to say
/// "loading" rather than "nothing here".
#[test]
fn an_empty_app_says_it_is_loading() {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    state.handle_events(
        NOW,
        &[ui(EventUi::Started), open(Screen::Ledger)],
        &mut actions,
    );

    let view = last_render(&actions);

    assert_eq!(ledger_of(&view).empty.as_deref(), Some("Loading…"));
}

#[test]
fn a_failed_load_tells_the_user_and_still_draws() {
    let mut state = at_now();

    eval!(&mut state,
        < ui(EventUi::Started);

        < Event::FromSource {
            event: EventSource::LoadFailed {
                message: "the server is not running".to_string(),
            },
        },
        @r#"
    [1] ToSource {
        action: LoadSnapshot {
            force: false,
        },
    }
    [1] ToUi {
        action: Render {
            screen: Home {
                view: ViewHome {
                    balances: ViewHomeBalances {
                        headline: "••••••",
                        masked: true,
                        detail: "held across 0 accounts",
                    },
                    recurring: ViewHomeRecurring {
                        headline: "••••••",
                        masked: true,
                        detail: "nothing due this week",
                        breakdown: [],
                        upcoming: [],
                    },
                    ledger: [],
                    attention: [],
                },
            },
            dock: ViewDock {
                items: [
                    ViewDockItem {
                        screen: Home,
                        label: "Home",
                        icon: "home",
                        selected: true,
                    },
                    ViewDockItem {
                        screen: Ledger,
                        label: "Ledger",
                        icon: "ledger",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Recurring,
                        label: "Recurring",
                        icon: "recurring",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Balances,
                        label: "Balances",
                        icon: "balances",
                        selected: false,
                    },
                ],
            },
        },
    }
    [2] ToUi {
        action: Render {
            screen: Home {
                view: ViewHome {
                    balances: ViewHomeBalances {
                        headline: "••••••",
                        masked: true,
                        detail: "held across 0 accounts",
                    },
                    recurring: ViewHomeRecurring {
                        headline: "••••••",
                        masked: true,
                        detail: "nothing due this week",
                        breakdown: [],
                        upcoming: [],
                    },
                    ledger: [],
                    attention: [],
                },
            },
            dock: ViewDock {
                items: [
                    ViewDockItem {
                        screen: Home,
                        label: "Home",
                        icon: "home",
                        selected: true,
                    },
                    ViewDockItem {
                        screen: Ledger,
                        label: "Ledger",
                        icon: "ledger",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Recurring,
                        label: "Recurring",
                        icon: "recurring",
                        selected: false,
                    },
                    ViewDockItem {
                        screen: Balances,
                        label: "Balances",
                        icon: "balances",
                        selected: false,
                    },
                ],
            },
        },
    }
    [2] ToUi {
        action: ShowError {
            message: "the server is not running",
        },
    }
    "#,
    );
}

/// Whatever was already loaded stays on screen: a dropped refresh should not
/// empty the app.
#[test]
fn a_failure_after_a_good_load_keeps_the_data() {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    state.handle_events(
        NOW,
        &[
            ui(EventUi::Started),
            loaded(),
            Event::FromSource {
                event: EventSource::LoadFailed {
                    message: "network unreachable".to_string(),
                },
            },
            open(Screen::Ledger),
        ],
        &mut actions,
    );

    assert_eq!(state.entry_count(), 48);

    let view = last_render(&actions);

    assert!(ledger_of(&view).empty.is_none(), "the list still has rows",);
}

/// A snapshot we cannot read is a failed load, not a panic.
#[test]
fn an_unreadable_snapshot_does_not_crash() {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    state.handle_events(
        NOW,
        &[
            ui(EventUi::Started),
            Event::FromSource {
                event: EventSource::SnapshotLoaded {
                    json: "{ not json at all".to_string(),
                },
            },
        ],
        &mut actions,
    );

    assert_eq!(state.entry_count(), 0);
    assert!(matches!(last_render(&actions), ViewScreen::Home { .. }));
}

/// The reason for all of this: if every decision is a pure function of state
/// and an event, a log of the events is a recording of the session. Replay it
/// and the same thing happens.
#[test]
fn the_same_events_give_the_same_actions() {
    let events = || {
        vec![
            ui(EventUi::Started),
            loaded(),
            open(Screen::Ledger),
            toggle("FoodAndDrink"),
            range(RangeSelection::This {
                unit: RangeUnit::Month,
            }),
            ui(EventUi::MaskToggled),
            open(Screen::Balances),
            open(Screen::Recurring),
            ui(EventUi::FilterToggled),
        ]
    };

    let run = || {
        let mut state = StateApp::new();
        let mut actions = Vec::new();

        state.handle_events(NOW, &events(), &mut actions);

        format!("{actions:?}")
    };

    assert_eq!(run(), run());
}

/// The clock arrives as an argument, so the same events an hour of wall-clock
/// later still land on the same screen.
#[test]
fn the_machine_does_not_depend_on_when_it_runs() {
    let render = || {
        let mut state = StateApp::new();
        let mut actions = Vec::new();

        state.handle_events(
            NOW,
            &[ui(EventUi::Started), loaded(), open(Screen::Recurring)],
            &mut actions,
        );

        format!("{:?}", last_render(&actions))
    };

    let first = render();
    std::thread::sleep(std::time::Duration::from_millis(20));

    assert_eq!(first, render());
}

/// Every event renders, so the view never has to guess whether to redraw.
#[test]
fn every_ui_event_produces_a_render() {
    for event in [
        EventUi::ScreenOpened {
            screen: Screen::Ledger,
        },
        EventUi::MaskToggled,
        EventUi::FilterToggled,
        EventUi::FilterGroupToggled {
            group: FilterGroup::Categories,
        },
        EventUi::CategoryToggled {
            id: "Travel".to_string(),
        },
        EventUi::CategoriesCleared,
        EventUi::RangeSelected {
            kind: RangeSelection::All,
        },
        EventUi::TxSelected {
            id: "whatever".to_string(),
        },
        EventUi::RefreshRequested,
    ] {
        let mut state = StateApp::new();
        let mut actions = Vec::new();

        let label = format!("{event:?}");

        state.handle_events(
            NOW,
            &[ui(EventUi::Started), loaded(), Event::FromUi { event }],
            &mut actions,
        );

        let renders = actions
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    Action::ToUi {
                        action: ActionUi::Render { .. }
                    }
                )
            })
            .count();

        assert!(renders >= 2, "{label} should have rendered");
    }
}

/// The server's clock wins over the device's, and time still passes between
/// snapshots. A phone with a wrong date would otherwise render a confidently
/// wrong "in 3 days".
#[test]
fn the_servers_clock_wins_over_the_devices() {
    let mut state = StateApp::new();
    let mut actions = Vec::new();

    // A device that thinks it is a year earlier than the server.
    let device = NOW.saturating_sub_secs(365 * 86_400);

    state.handle_events(
        device,
        &[ui(EventUi::Started), loaded(), open(Screen::Recurring)],
        &mut actions,
    );

    let view = last_render(&actions);

    let row = recurring_of(&view)
        .groups
        .iter()
        .flat_map(|g| &g.streams)
        .find(|s| s.title == "AUTOMATIC PAYMENT - THANK")
        .expect("the feed has this stream");

    assert_eq!(
        row.when, "next Aug 22 · in 3 days",
        "the server's clock decides, not the device's",
    );
}
