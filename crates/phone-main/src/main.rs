use leptos::prelude::*;

mod app;
mod fake;
mod pages;
mod shell;
mod state;

use app::App;

fn main() {
    // Before anything else: a log that starts after the first request is a log
    // missing the request that broke.
    api_trace::init(api_trace::Config::default());

    tracing::info!(
        target: "app",
        version = env!("CARGO_PKG_VERSION"),
        "meridian phone starting",
    );

    mount_to_body(App)
}
