use leptos::prelude::*;

mod app;
mod runtime_app;
mod runtime_io;
mod screens;
mod ui;

use app::App;

fn main() {
    // Before anything else: a log started later is missing what broke.
    api_trace::init(api_trace::Config::default());

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "meridian phone starting",
    );

    mount_to_body(App)
}
