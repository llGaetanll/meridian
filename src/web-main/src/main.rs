use leptos::prelude::*;

mod app;
mod fake;
mod io;
mod pages;
mod state;

use app::App;

fn main() {
    mount_to_body(App)
}
