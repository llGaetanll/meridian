use leptos::prelude::*;

mod app;
mod util;

use app::App;

fn main() {
    mount_to_body(App)
}
