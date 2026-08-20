//! Print the snapshot the fixtures produce, so `phone-state`'s tests can run
//! against exactly what the server would have sent.
//!
//! ```sh
//! cargo run -p server-main --example snapshot > crates/phone-state/tests/snapshot.json
//! ```
//!
//! `now` is written as 0 and the tests substitute a fixed clock, so nothing
//! about them depends on when this was generated.
//!
//! Written with 4-space indentation, per `docs/STYLE.md`: this file is read by
//! people as much as by tests.

fn main() {
    let fetched = server_main::fetch_fixtures(api_common::Timestamp::EPOCH);

    // Applied through a ledger rather than serialised straight from the diff,
    // so the fixture is ordered exactly as the running server orders it —
    // newest first. Writing `fetched.added` here would emit Plaid's own order
    // and quietly disagree with what `/snapshot` returns.
    let mut ledger = api_ledger::Ledger::default();
    ledger.apply(fetched.added, fetched.modified, &fetched.removed);

    let snapshot = server_main::WireSnapshot::new(
        ledger.entries(),
        &fetched.connections,
        &fetched.streams,
        api_common::Timestamp::EPOCH,
        server_main::WireSource::Fixture,
    );

    let mut out = Vec::new();
    let indent = serde_json::ser::PrettyFormatter::with_indent(b"    ");
    let mut ser = serde_json::Serializer::with_formatter(&mut out, indent);

    serde::Serialize::serialize(&snapshot, &mut ser).expect("serialises");

    println!("{}", String::from_utf8(out).expect("json is utf-8"));
}
