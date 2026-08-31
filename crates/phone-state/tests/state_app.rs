//! The app's state machine, tested the way `docs/STYLE.md` says: events in
//! with `<`, actions out with `>`.
//!
//! Each test carries its own snapshot, written out in the test, holding the
//! fewest records its claim needs — usually one, never more than four. Two
//! transactions on one day is what "a day heading carries its net" is about;
//! forty-eight of them would say the same thing while making the expected
//! value something only a fixture file knows.
//!
//! The snapshot is the JSON the server sends (`crates/server-wire`). Every
//! field but `now` defaults, so `{"now": …}` is a whole snapshot, and a screen
//! whose claim needs no records asks for none.

use api_common::Timestamp;
use api_eval::eval;
use phone_state::*;

/// 2026-08-19 00:00 UTC — a Wednesday, and the day the sketch is drawn
/// against, so "in 3 days" and the rest line up with what it shows. The
/// snapshots below carry the same instant as their `now`.
const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);

/// A machine past first run, with its clock set — where every test here
/// starts.
///
/// The app assumes a device it has never run on, so saying who is signed in is
/// what puts it on the Ledger. What that answer draws is
/// `a_session_opens_the_app_on_its_data`'s claim rather than every test's, so
/// the actions go nowhere here. `tests/state_onboarding.rs` is the file that
/// does not sign in.
fn app() -> StateApp {
    let mut state = StateApp::new();
    state.set_now(NOW);

    state.handle_events(
        &[Event::FromSource {
            event: EventSource::SessionLoaded {
                email: Some("al@example.com".to_string()),
            },
        }],
        &mut Vec::new(),
    );

    state
}

/// The rows of a ledger, in the order they are drawn. Months and days are the
/// shape the screen wants; a test about a row wants the rows.
fn rows(ledger: &ViewLedger) -> Vec<&ViewTxRow> {
    ledger
        .months
        .iter()
        .flat_map(|m| &m.days)
        .flat_map(|d| &d.rows)
        .collect()
}

/// Who is signed in comes before anything else, because the answer is what
/// decides whether the app opens on the Ledger or on the first screen of first
/// run. Nothing is fetched until it comes back.
#[test]
fn starting_asks_who_is_signed_in() {
    let mut state = StateApp::new();
    state.set_now(NOW);

    eval!(&mut state,
        < Event::FromUi { event: EventUi::Started },
        > @r#"ToSource { action: LoadSession }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// And the answer is what asks for the data.
#[test]
fn a_session_opens_the_app_on_its_data() {
    let mut state = StateApp::new();
    state.set_now(NOW);

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SessionLoaded { email: Some("al@example.com".to_string()) } },
        > @r#"ToSource { action: LoadSnapshot { force: false } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

#[test]
fn pull_to_refresh_forces_a_fetch() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::RefreshRequested },
        > @r#"ToSource { action: LoadSnapshot { force: true } }"#,
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// One snapshot feeds every screen, so switching tabs must not go back to the
/// network. This is what makes the dock feel instant: there is no `ToSource`
/// anywhere in the conversation.
#[test]
fn switching_tabs_asks_for_nothing() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Ledger } },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// One document, three screens: a transaction, an account and a stream arrive
/// in the same snapshot and each lands where it belongs.
#[test]
fn a_snapshot_lands_in_the_machine() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "entries":[{"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}],
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[{"id":"acc","name":"Plaid Checking","balance":100.0,"group":"depository"}]}],
            "streams":[{"id":"s","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );

    assert_eq!(state.entry_count(), 1);
    assert_eq!(state.connection_count(), 1);
    assert_eq!(state.stream_count(), 1);
}

/// Every screen draws from the same snapshot, including one with nothing in
/// it — a tab that cannot draw an empty month is a tab that breaks on the day
/// a bank goes quiet.
#[test]
fn every_screen_renders() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { .. }, .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { .. }, .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { .. }, .. } },
    );
}

/// Two transactions on one day and one a month earlier: two months, and the
/// newest day of the newest month first.
#[test]
fn the_ledger_groups_by_month_and_day() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"b","ts":1786492800,"amount":-25.00,"asset":"USD","label":"Card payment","category":"LoanPayments"},
            {"id":"c","ts":1783814400,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let months: Vec<&str> = ledger.months.iter().map(|m| m.name.as_str()).collect();

            assert_eq!(months, ["August", "July"]);
            assert_eq!(ledger.months[0].days.len(), 1, "one day, both of its rows in it");
            assert_eq!(ledger.months[0].days[0].heading, "Wed, Aug 12");
            assert_eq!(rows(ledger).len(), 3, "every row lands in exactly one day");
        },
    );
}

/// The heading carries the day's net, so the two halves of the line say the
/// same thing about the same day. Signed, because a day that gave back more
/// than it took is worth seeing as such.
#[test]
fn a_day_heading_carries_its_net() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"b","ts":1786492800,"amount":-25.00,"asset":"USD","label":"Card payment","category":"LoanPayments"},
            {"id":"c","ts":1786320000,"amount":500.00,"asset":"USD","label":"United Airlines","category":"Travel"},
            {"id":"d","ts":1786320000,"amount":-78.50,"asset":"USD","label":"Climbing gym","category":"Entertainment"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let days = &ledger.months[0].days;

            assert_eq!(days[0].total, "−$30.40", "−5.40 and −25.00");
            assert_eq!(days[1].total, "+$421.50", "+500.00 against −78.50");
        },
    );
}

/// The marker over each month reads as one phrase.
#[test]
fn a_month_is_labelled_with_its_year() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(ledger.months[0].label, "August 2026");
        },
    );
}

/// Two rows on one day are one heading, not two.
#[test]
fn a_day_heading_never_repeats_within_a_month() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"b","ts":1786492800,"amount":-25.00,"asset":"USD","label":"Card payment","category":"LoanPayments"},
            {"id":"c","ts":1786320000,"amount":-78.50,"asset":"USD","label":"Climbing gym","category":"Entertainment"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let headings: Vec<&str> = ledger.months[0].days.iter().map(|d| d.heading.as_str()).collect();

            assert_eq!(headings, ["Wed, Aug 12", "Mon, Aug 10"], "one each, newest first");
        },
    );
}

/// Plaid reports a refund as a positive amount on a spending category; the
/// ledger shows it as money arriving.
#[test]
fn a_refund_reads_as_money_arriving() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786320000,"amount":500.00,"asset":"USD","label":"United Airlines","category":"Travel"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let row = rows(ledger)[0];

            assert_eq!(row.amount, "+$500.00");
            assert!(row.is_inflow);
        },
    );
}

#[test]
fn a_purchase_reads_as_money_leaving() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let row = rows(ledger)[0];

            assert_eq!(row.amount, "−$12.00");
            assert!(!row.is_inflow);
        },
    );
}

#[test]
fn selecting_a_category_narrows_the_list() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"b","ts":1786492800,"amount":-4.33,"asset":"USD","label":"Starbucks","category":"FoodAndDrink"},
            {"id":"c","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let shown = rows(ledger);

            assert_eq!(shown.len(), 2, "the Uber is filtered out");
            assert!(shown.iter().all(|r| r.category == "Food & drink"));
        },
    );
}

/// Multi-select: the chips are a union, not a replacement.
#[test]
fn two_categories_show_both() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"b","ts":1786492800,"amount":-410.00,"asset":"USD","label":"United Airlines","category":"Travel"},
            {"id":"c","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "Travel".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 2, "food and travel, not the Uber");
        },
    );
}

#[test]
fn tapping_a_selected_category_deselects_it() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"b","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 1);
        },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 2, "back to everything");
        },
    );
}

/// The count on a chip has to say what selecting *that* chip would give.
/// Computing it through the current selection would read zero for every
/// unselected chip, which is the bug this pins.
#[test]
fn the_chip_counts_ignore_the_category_selection() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"b","ts":1786492800,"amount":-410.00,"asset":"USD","label":"United Airlines","category":"Travel"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let travel = ledger.filter.categories.iter().find(|c| c.id == "Travel").expect("travel is always a chip");

            assert_eq!(travel.count, "1", "still says what travel would give");
            assert!(!travel.selected);
        },
    );
}

/// Every one of Plaid's sixteen primaries is offered, even the ones with
/// nothing in them — they read as unavailable rather than vanishing, so the
/// grid does not reflow as data changes.
#[test]
fn all_sixteen_categories_are_offered() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let chips = &ledger.filter.categories;

            assert_eq!(chips.len(), 16, "one transaction, still sixteen chips");
            assert_eq!(chips.iter().filter(|c| c.empty).count(), 15, "and fifteen of them read as empty");
        },
    );
}

/// A shut filter has to say what it is doing in the room a shut filter has:
/// the first two by name, the rest as a number.
#[test]
fn a_shut_filter_summarises_what_it_is_doing() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        < Event::FromUi { event: EventUi::CategoryToggled { id: "Travel".to_string() } },
        < Event::FromUi { event: EventUi::CategoryToggled { id: "Transportation".to_string() } },
        < Event::FromUi { event: EventUi::CategoryToggled { id: "LoanPayments".to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(ledger.filter.categories_summary.len(), 2, "two named");
            assert_eq!(ledger.filter.categories_overflow.as_deref(), Some("+2"));
        },
    );
}

#[test]
fn clearing_categories_shows_everything_again() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"b","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoriesCleared },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 2);
        },
    );
}

/// A chip id the app does not know is a phone running against a newer server.
/// It changes nothing rather than filtering everything away.
#[test]
fn an_unknown_category_id_is_ignored() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "NotACategory".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 1, "the list is untouched");
        },
    );
}

/// An empty list has to say which kind of empty it is: nothing yet, or nothing
/// that matches.
#[test]
fn a_filter_matching_nothing_says_so() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "Medical".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert!(rows(ledger).is_empty());
            assert_eq!(ledger.empty.as_deref(), Some("Nothing matches this filter."));
        },
    );
}

#[test]
fn this_month_keeps_only_august() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"b","ts":1783814400,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::RangeSelected { kind: RangeSelection::This { unit: RangeUnit::Month } } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let months: Vec<&str> = ledger.months.iter().map(|m| m.name.as_str()).collect();

            assert_eq!(months, ["August"], "July's row is out");
            assert_eq!(ledger.filter.range_summary, "This month");
        },
    );
}

/// Three months back reaches past this month and then stops.
#[test]
fn the_last_three_months_reach_back_further() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"b","ts":1781222400,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"c","ts":1778544000,"amount":-4.33,"asset":"USD","label":"Starbucks","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::RangeSelected { kind: RangeSelection::Last { count: 3, unit: RangeUnit::Month } } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let months: Vec<&str> = ledger.months.iter().map(|m| m.name.as_str()).collect();

            assert_eq!(months, ["August", "June"], "May is further back than three months");
            assert_eq!(ledger.filter.range_summary, "Last 3 months");
        },
    );
}

/// The end of a `Between` range is a day, and a day includes its own
/// transactions — an end that meant midnight would drop them all.
#[test]
fn a_between_range_includes_its_last_day() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"b","ts":1786492800,"amount":-25.00,"asset":"USD","label":"Card payment","category":"LoanPayments"},
            {"id":"c","ts":1786320000,"amount":-78.50,"asset":"USD","label":"Climbing gym","category":"Entertainment"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        // Aug 12 to Aug 12 — the day the first two rows are on.
        < Event::FromUi { event: EventUi::RangeSelected { kind: RangeSelection::Between { start: 1_786_492_800, end: 1_786_492_800 } } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 2, "both of Aug 12's rows, and Aug 10 left out");
            assert_eq!(ledger.months[0].days[0].heading, "Wed, Aug 12");
        },
    );
}

#[test]
fn all_time_is_the_default_and_filters_nothing() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"b","ts":1778544000,"amount":-4.33,"asset":"USD","label":"Starbucks","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 2, "three months apart, both drawn");
            assert_eq!(ledger.filter.range_summary, "All time");
        },
    );
}

/// The two filters are an intersection: a row has to be in the category *and*
/// in the range.
#[test]
fn a_category_and_a_range_apply_together() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"b","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"},
            {"id":"c","ts":1783814400,"amount":-4.33,"asset":"USD","label":"Starbucks","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
        < Event::FromUi { event: EventUi::RangeSelected { kind: RangeSelection::This { unit: RangeUnit::Month } } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let shown = rows(ledger);

            assert_eq!(shown.len(), 1, "August's food — not July's, and not the Uber");
            assert_eq!(shown[0].title, "McDonald's");
        },
    );
}

/// The chip counts are computed through the time range, or a chip would offer
/// rows the range has already ruled out.
#[test]
fn the_chip_counts_follow_the_time_range() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
            {"id":"b","ts":1783814400,"amount":-4.33,"asset":"USD","label":"Starbucks","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let food = ledger.filter.categories.iter().find(|c| c.id == "FoodAndDrink").expect("a chip per category");

            assert_eq!(food.count, "2", "both of them, over all time");
        },

        < Event::FromUi { event: EventUi::RangeSelected { kind: RangeSelection::This { unit: RangeUnit::Month } } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            let food = ledger.filter.categories.iter().find(|c| c.id == "FoodAndDrink").expect("a chip per category");

            assert_eq!(food.count, "1", "August's alone");
        },
    );
}

/// Revealed by default. The phone has no eye any more, so a covered figure
/// would be one nothing on the screen could uncover.
#[test]
fn figures_start_revealed() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[{"id":"acc","name":"Plaid Checking","balance":100.0,"group":"depository"}]}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { view: balances }, .. } } => {
            assert!(!balances.masked);
        },
    );
}

/// Covering still works, and the figure must not travel in the clear — what a
/// covered headline carries is dots, not a number nobody drew.
#[test]
fn covering_hides_the_figure() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[{"id":"acc","name":"Plaid Checking","balance":100.0,"group":"depository"}]}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::MaskToggled },
        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { view: balances }, .. } } => {
            assert!(balances.masked);
            assert_eq!(balances.headline, "••••••");
        },
    );
}

/// One switch for the whole app: covering on the Ledger and staying revealed
/// on Recurring would be a strange thing to have to explain.
#[test]
fn the_mask_is_one_switch_across_screens() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "streams":[{"id":"s","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::MaskToggled },
        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            assert!(recurring.masked, "switched on the Ledger, covered here");
            assert_eq!(recurring.headline, "••••••");
        },
    );
}

/// Ledger rows are never covered — a list of grey bars says nothing, and the
/// figure worth hiding is the total rather than a $4.33 coffee.
#[test]
fn the_mask_leaves_ledger_rows_alone() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-4.33,"asset":"USD","label":"Starbucks","category":"FoodAndDrink"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::MaskToggled },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger)[0].amount, "−$4.33", "covered everywhere else, not here");
        },
    );
}

#[test]
fn balances_lead_with_what_is_held() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[
                {"id":"acc-1","name":"Plaid Checking","balance":100.0,"group":"depository"},
                {"id":"acc-2","name":"Plaid Saving","balance":200.0,"group":"depository"}]}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { view: balances }, .. } } => {
            assert_eq!(balances.headline_label, "Held");
            assert_eq!(balances.headline, "$300.00", "the two balances, added");
        },
    );
}

/// A mortgage among the accounts you hold reads as something you owe. Debts
/// get their own block, after everything held.
#[test]
fn debts_are_split_out_and_come_last() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[
                {"id":"acc-1","name":"Plaid Mortgage","balance":56302.06,"group":"loan"},
                {"id":"acc-2","name":"Plaid Checking","balance":100.0,"group":"depository"}]}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { view: balances }, .. } } => {
            let blocks: Vec<bool> = balances.connections.iter().map(|c| c.is_debts).collect();

            assert_eq!(blocks, [false, true], "held first, owed after");

            let debts = &balances.connections[1];

            assert_eq!(debts.total, "−$56,302.06");
            assert_eq!(debts.total_label, "1 account owed");
            assert!(debts.accounts.iter().all(|a| a.owed));
        },
    );
}

#[test]
fn a_credit_card_shows_what_is_used_of_its_limit() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[
                {"id":"acc","name":"Plaid Credit Card","subtype":"credit card","balance":410.0,"limit":2000.0,"group":"credit"}]}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { view: balances }, .. } } => {
            let row = &balances.connections[0].accounts[0];

            assert_eq!(row.amount, "−$410.00");
            assert_eq!(row.utilisation.as_deref(), Some("$410.00 of $2,000.00 limit used"));
        },
    );
}

/// A line of credit's useful half is how much room is left, not how much is
/// gone.
#[test]
fn a_line_of_credit_shows_what_is_left() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[
                {"id":"acc","name":"Plaid Home Equity","subtype":"home equity","balance":2352.50,"available":47647.50,"limit":50000.0,"group":"credit"}]}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { view: balances }, .. } } => {
            let row = &balances.connections[0].accounts[0];

            assert_eq!(row.utilisation.as_deref(), Some("$47,647.50 still available"));
        },
    );
}

/// The kind, the last four digits and what is available read as one phrase —
/// and Plaid's `401k` is not what a person calls it.
#[test]
fn an_account_subtitle_reads_as_a_sentence() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,
            "connections":[{"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[
                {"id":"acc-1","name":"Plaid Checking","subtype":"checking","mask":"0000","balance":100.0,"available":100.0,"group":"depository"},
                {"id":"acc-2","name":"Plaid 401k","subtype":"401k","mask":"6666","balance":23631.98,"group":"investment"}]}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Balances } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Balances { view: balances }, .. } } => {
            let accounts = &balances.connections[0].accounts;

            assert_eq!(accounts[0].subtitle, "Checking ·0000 · $100.00 available");
            assert_eq!(accounts[1].subtitle, "401(k) ·6666");
        },
    );
}

/// The grid is as many rows as the month has weeks in it. August 2026 starts
/// on a Saturday and runs 31 days, which is six; July is five. No transaction
/// is involved in either fact, so the snapshot carries none.
#[test]
fn the_calendar_is_as_tall_as_its_month() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            assert_eq!(recurring.calendar.current.weeks.len(), 6, "August 2026");
            assert_eq!(recurring.calendar.prev.weeks.len(), 5, "July 2026");
        },
    );
}

/// The months either side arrive whole, so a swipe has something to land on.
#[test]
fn the_months_either_side_arrive_whole() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let calendar = &recurring.calendar;

            assert_eq!(calendar.prev.label, "July 2026");
            assert_eq!(calendar.current.label, "August 2026");
            assert_eq!(calendar.next.label, "September 2026");
        },
    );
}

/// A day is one date wherever it is drawn, so a day borrowed by the month next
/// to it carries the same id — otherwise tapping it in one month would open
/// nothing in the other.
#[test]
fn a_day_has_one_id_wherever_it_is_drawn() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let calendar = &recurring.calendar;

            // August starts on a Saturday, so its first row borrows the end of
            // July — the same days July's own grid ends with.
            let borrowed: Vec<&str> = calendar.current.weeks[0].iter().filter(|d| d.outside).map(|d| d.id.as_str()).collect();
            let julys: Vec<&str> = calendar.prev.weeks.iter().flatten().filter(|d| !d.outside).map(|d| d.id.as_str()).collect();

            // Jul 27 to Jul 31 2026, as the day numbers a tap sends back.
            assert_eq!(borrowed, ["20661", "20662", "20663", "20664", "20665"]);
            assert!(borrowed.iter().all(|id| julys.contains(id)), "and July draws the same five");
        },
    );
}

/// Tapping a day opens a card for it, and tapping it again shuts it.
#[test]
fn tapping_a_day_opens_it_and_tapping_it_again_shuts_it() {
    let mut state = app();

    eval!(&mut state,
        // Two streams land on Aug 22, so the card has something to list.
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s1","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","next_date":1787356800,"is_active":true},
            {"id":"s2","label":"Spotify","category":"Entertainment","amount":-11.99,"frequency":"MONTHLY","next_date":1787356800,"is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            assert!(recurring.day_card.is_none(), "nothing tapped yet");
        },

        < Event::FromUi { event: EventUi::CalendarDaySelected { id: "20687".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let card = recurring.day_card.as_ref().expect("the day that was tapped opens");

            assert_eq!(card.rows.len(), 2, "both streams that land on it");

            let picked: Vec<&str> = recurring.calendar.current.weeks.iter().flatten().filter(|d| d.picked).map(|d| d.id.as_str()).collect();

            assert_eq!(picked, ["20687"], "and it reads as picked");
        },

        < Event::FromUi { event: EventUi::CalendarDaySelected { id: "20687".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            assert!(recurring.day_card.is_none(), "and tapping it again shuts it");
        },
    );
}

/// The screen leads with what the month comes to — everything going out and
/// everything coming in, netted, and signed so a month that pays for itself
/// reads as one.
#[test]
fn recurring_leads_with_the_monthly_commitment() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s1","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","is_active":true},
            {"id":"s2","label":"Rent","category":"RentAndUtilities","amount":-1500.00,"frequency":"MONTHLY","is_active":true},
            {"id":"s3","label":"Payroll","category":"Income","amount":2000.00,"frequency":"MONTHLY","is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            assert_eq!(recurring.headline_label, "Every month");
            assert_eq!(recurring.headline, "$484.01", "2000.00 against 15.99 and 1500.00");
        },
    );
}

/// A grid of whole weeks, every row seven days, so the columns line up under
/// their headings.
#[test]
fn the_calendar_draws_whole_weeks() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let lengths: Vec<usize> = recurring.calendar.current.weeks.iter().map(|w| w.len()).collect();

            assert_eq!(lengths, [7, 7, 7, 7, 7, 7]);
        },
    );
}

/// August 2026 starts on a Saturday, so the first row is five borrowed days
/// and then the 1st.
#[test]
fn the_calendar_starts_the_month_on_its_own_weekday() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let week = &recurring.calendar.current.weeks[0];

            let labels: Vec<&str> = week.iter().map(|d| d.label.as_str()).collect();
            let dimmed: Vec<bool> = week.iter().map(|d| d.outside).collect();

            assert_eq!(labels, ["27", "28", "29", "30", "31", "1", "2"]);
            assert_eq!(dimmed, [true, true, true, true, true, false, false]);
        },
    );
}

/// Exactly one day is today, and it is the 19th.
#[test]
fn the_calendar_marks_today() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let today: Vec<&str> = recurring.calendar.current.weeks.iter().flatten().filter(|d| d.today).map(|d| d.label.as_str()).collect();

            assert_eq!(today, ["19"]);
        },
    );
}

/// Only dates the source predicts. Two streams that land on the same day are
/// two marks on it, a stream without a date is no mark at all, and nothing is
/// projected forward from a frequency.
#[test]
fn the_calendar_marks_only_the_dates_plaid_gave_us() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s1","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","next_date":1787356800,"is_active":true},
            {"id":"s2","label":"Spotify","category":"Entertainment","amount":-11.99,"frequency":"MONTHLY","next_date":1787356800,"is_active":true},
            {"id":"s3","label":"Rent","category":"RentAndUtilities","amount":-1500.00,"frequency":"MONTHLY","is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let marked: Vec<(&str, usize)> = recurring.calendar.current.weeks.iter().flatten()
                .filter(|d| !d.marks.is_empty())
                .map(|d| (d.id.as_str(), d.marks.len()))
                .collect();

            assert_eq!(marked, [("20687", 2)], "Aug 22, the two with a date, and nowhere else");

            // A mark carries the category token the breakdown above colours by.
            let tokens: Vec<&str> = recurring.calendar.current.weeks.iter().flatten()
                .flat_map(|d| d.marks.iter().map(|m| m.as_str()))
                .collect();

            assert!(tokens.iter().all(|t| t.starts_with("c-")), "{tokens:?}");
        },
    );
}

/// The bar and the legend are two views of one split, so the shares have to
/// add up to the whole bar, biggest first.
#[test]
fn the_breakdown_covers_the_whole_bar() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s1","label":"Rent","category":"RentAndUtilities","amount":-1500.00,"frequency":"MONTHLY","is_active":true},
            {"id":"s2","label":"Netflix","category":"Entertainment","amount":-500.00,"frequency":"MONTHLY","is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let shares: Vec<f64> = recurring.breakdown.iter().map(|s| s.share).collect();

            assert_eq!(shares, [0.75, 0.25], "1500 and 500 of 2000, biggest first");
        },
    );
}

/// The bar and the list are two views of one split, so the groups have to run
/// in the same order the bar's segments do.
#[test]
fn stream_groups_run_biggest_first_like_the_bar() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s1","label":"Netflix","category":"Entertainment","amount":-500.00,"frequency":"MONTHLY","is_active":true},
            {"id":"s2","label":"Rent","category":"RentAndUtilities","amount":-1500.00,"frequency":"MONTHLY","is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let list: Vec<&str> = recurring.groups.iter().map(|g| g.name.as_str()).collect();
            let bar: Vec<&str> = recurring.breakdown.iter().map(|s| s.label.as_str()).collect();

            assert_eq!(list, bar, "the bar and the list must not disagree");
            assert_eq!(recurring.groups[0].total, "$1,500.00", "the bigger group leads");
        },
    );
}

/// What comes in is grouped after everything going out, however big it is:
/// it is a different kind of thing, not a bigger one.
#[test]
fn money_coming_in_is_grouped_after_everything_going_out() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s1","label":"Payroll","category":"Income","amount":5000.00,"frequency":"MONTHLY","is_active":true},
            {"id":"s2","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let last = recurring.groups.last().expect("two groups");

            assert!(last.streams.iter().all(|s| s.amount.starts_with('+')), "the $5,000 comes last");
        },
    );
}

/// A stream says when it next lands, in the words the sketch uses.
#[test]
fn a_stream_says_when_it_next_lands() {
    let mut state = app();

    eval!(&mut state,
        // Aug 22 — three days after the snapshot's clock.
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","next_date":1787356800,"is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            let row = &recurring.groups[0].streams[0];

            assert_eq!(row.when, "next Aug 22 · in 3 days");
            assert!(row.soon, "three days out is soon");
        },
    );
}

/// Soon is a fortnight, not a season: a stream further out than that is drawn
/// like any other.
#[test]
fn a_distant_stream_is_not_marked_soon() {
    let mut state = app();

    eval!(&mut state,
        // Sep 30 — six weeks out.
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","next_date":1790726400,"is_active":true}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            assert!(!recurring.groups[0].streams[0].soon);
        },
    );
}

/// Nothing is on screen until data arrives, and the wording has to say
/// "loading" rather than "nothing here".
#[test]
fn an_empty_app_says_it_is_loading() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Ledger } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(ledger.empty.as_deref(), Some("Loading…"));
        },
    );
}

#[test]
fn a_failed_load_tells_the_user_and_still_draws() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::RefreshRequested },
        > Action::ToSource { action: ActionSource::LoadSnapshot { force: true } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::LoadFailed { message: "the server is not running".to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::ShowError { .. } },
    );
}

/// Whatever was already loaded stays on screen: a dropped refresh should not
/// empty the app.
#[test]
fn a_failure_after_a_good_load_keeps_the_data() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
            {"id":"a","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}]}"#.to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromSource { event: EventSource::LoadFailed { message: "network unreachable".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
            assert_eq!(rows(ledger).len(), 1, "the list still has its row");
            assert!(ledger.empty.is_none());
        },
        > Action::ToUi { action: ActionUi::ShowError { .. } },
    );
}

/// A snapshot we cannot read is a failed load, not a panic.
#[test]
fn an_unreadable_snapshot_does_not_crash() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: "{ not json at all".to_string() } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { .. }, .. } },
    );

    assert_eq!(state.entry_count(), 0);
}

/// The reason for all of this: if every decision is a pure function of state
/// and an event, a log of the events is a recording of the session. Replay it
/// and the same thing happens — the one test here that drives two machines and
/// compares them, rather than saying what either one did.
#[test]
fn the_same_events_give_the_same_actions() {
    let events = || {
        vec![
            Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"entries":[
                {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
                {"id":"b","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}]}"#.to_string() } },
            Event::FromUi { event: EventUi::CategoryToggled { id: "FoodAndDrink".to_string() } },
            Event::FromUi { event: EventUi::RangeSelected { kind: RangeSelection::This { unit: RangeUnit::Month } } },
            Event::FromUi { event: EventUi::MaskToggled },
            Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        ]
    };

    let run = || {
        let mut state = app();
        let mut actions = Vec::new();

        state.handle_events(&events(), &mut actions);

        format!("{actions:?}")
    };

    assert_eq!(run(), run(), "a replay lands where it did");
}

/// Every event draws, so the view never has to guess whether to redraw.
#[test]
fn every_ui_event_produces_a_render() {
    let mut state = app();

    eval!(&mut state,
        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::MaskToggled },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::FilterGroupToggled { group: FilterGroup::Categories } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoryToggled { id: "Travel".to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CategoriesCleared },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::RangeSelected { kind: RangeSelection::This { unit: RangeUnit::Month } } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CalendarMonthStepped { step: -1 } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::CalendarDaySelected { id: "20687".to_string() } },
        > Action::ToUi { action: ActionUi::Render { .. } },

        < Event::FromUi { event: EventUi::Ticked },
        > Action::ToUi { action: ActionUi::Render { .. } },
    );
}

/// The server's clock wins over the device's. A phone with a wrong date would
/// otherwise render a confidently wrong "in 3 days".
#[test]
fn the_servers_clock_wins_over_the_devices() {
    let mut state = app();

    // A device that thinks it is a year earlier than the server.
    state.set_now(NOW.saturating_sub_secs(365 * 86_400));

    eval!(&mut state,
        < Event::FromSource { event: EventSource::SnapshotLoaded { json: r#"{"now":1787097600,"streams":[
            {"id":"s","label":"Netflix","category":"Entertainment","amount":-15.99,"frequency":"MONTHLY","next_date":1787356800,"is_active":true}]}"#.to_string() } },
        < Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Recurring } },
        > Action::ToUi { action: ActionUi::Render { .. } },
        > Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Recurring { view: recurring }, .. } } => {
            assert_eq!(recurring.groups[0].streams[0].when, "next Aug 22 · in 3 days", "the server's clock decides");
        },
    );
}
