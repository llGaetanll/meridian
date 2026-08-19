//! Print the snapshot the fixtures produce, so `api-core`'s tests can run
//! against exactly what the server would have sent.
//!
//! ```sh
//! cargo run -p srv-main --example snapshot > crates/api-core/tests/snapshot.json
//! ```
//!
//! `now` is written as 0 and the tests substitute a fixed clock, so nothing
//! about them depends on when this was generated.

#[path = "../src/plaid_io.rs"]
mod plaid_io;
#[path = "../src/project.rs"]
mod project;

fn main() {
    let fetched = plaid_io::fetch_fixtures(0);

    let snapshot = api_wire::Snapshot::new(
        &fetched.entries,
        &fetched.connections,
        &fetched.streams,
        0,
        api_wire::SnapshotSource::Fixture,
    );

    println!("{}", serde_json::to_string(&snapshot).expect("serialises"));
}
