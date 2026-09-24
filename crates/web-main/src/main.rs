use leptos::prelude::*;

mod app;
mod fake;
mod frame;
mod io;
mod pages;
mod phone;
mod runtime_app;
mod state;
mod tab;

use app::App;

fn main() {
    // Before anything else: a log that starts after the first request is a log
    // missing the request that broke.
    api_trace::init(api_trace::Config::default());

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "meridian starting");

    // Before the first paint: a page that draws dark and then turns light is
    // a page that flashes.
    web_components::restore_theme();

    mount_to_body(App)
}
