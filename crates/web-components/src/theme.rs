//! Which theme the browser is drawing in, and the switch that turns it over.
//!
//! Not the machine's. Nothing in `client-state` knows a colour, and a theme is
//! not a fact about the account — it is a fact about the screen somebody is
//! reading, which is why it is kept here and per browser rather than sent to
//! the server and shared between a laptop and a phone.
//!
//! One attribute on `<html>`, which is all `theme.css` reads, and one key in
//! `localStorage` so the next visit opens the way this one ended. A browser
//! that refuses storage draws the dark theme and forgets, which is the same
//! thing an incognito window does with the session.

use web_sys::Storage;

const THEME_KEY: &str = "meridian.theme";

const LIGHT: &str = "light";
const DARK: &str = "dark";

/// Put the kept theme back on the page. Called once, as the app starts, before
/// anything is drawn — a page that paints dark and then turns light is a page
/// that flashes.
pub fn restore() {
    apply(kept());
}

/// Whether the light theme is the one on.
pub fn is_light() -> bool {
    kept() == LIGHT
}

/// Turn it over, and answer with the theme now on.
pub fn toggle() -> &'static str {
    let next = match kept() == LIGHT {
        true => DARK,
        false => LIGHT,
    };

    apply(next);

    if let Some(storage) = storage()
        && storage.set_item(THEME_KEY, next).is_err()
    {
        tracing::error!("the theme could not be kept; the next visit opens in the dark one");
    }

    next
}

/// The theme the browser is holding. Dark is the app's, and what anything
/// with nothing kept gets.
fn kept() -> &'static str {
    let light = storage()
        .and_then(|storage| storage.get_item(THEME_KEY).ok().flatten())
        .is_some_and(|kept| kept == LIGHT);

    match light {
        true => LIGHT,
        false => DARK,
    }
}

/// Dark is the absence of the attribute, not a value of it: `theme.css` gives
/// `:root` the dark roles and only overrides them under `[data-theme=light]`,
/// so a page that says nothing is already right.
fn apply(theme: &str) {
    let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    else {
        return;
    };

    let done = match theme == LIGHT {
        true => root.set_attribute("data-theme", LIGHT),
        false => root.remove_attribute("data-theme"),
    };

    if done.is_err() {
        tracing::error!("the theme could not be set on the page");
    }
}

/// `None` where the browser has none to give: storage turned off, or a
/// private window that refuses it.
fn storage() -> Option<Storage> {
    web_sys::window()?.local_storage().ok()?
}
