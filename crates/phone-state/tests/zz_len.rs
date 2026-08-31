use api_common::Timestamp;
use phone_state::*;
const NOW: Timestamp = Timestamp::from_secs(1_787_097_600);
#[test]
fn len() {
    let mut state = StateApp::new();
    let mut actions = Vec::new();
    state.handle_events_at(Timestamp::EPOCH, &[Event::FromSource { event: EventSource::SessionLoaded { email: Some("al@example.com".to_string()) } }], &mut actions);
    actions.clear();
    let json = include_str!("snapshot.json").replace("\"now\": 0", &format!("\"now\": {}", NOW.as_secs()));
    state.handle_events_at(NOW, &[
        Event::FromUi { event: EventUi::Started },
        Event::FromSource { event: EventSource::SnapshotLoaded { json } },
        Event::FromUi { event: EventUi::ScreenOpened { screen: Screen::Ledger } },
    ], &mut actions);
    let s = format!("{:?}", actions.last().unwrap());
    println!("ledger render: {} chars", s.len());
}
