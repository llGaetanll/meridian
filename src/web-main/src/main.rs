use leptos::prelude::*;

mod app;
mod fake;
mod pages;

use app::App;

fn main() {
    mount_to_body(App)
}
