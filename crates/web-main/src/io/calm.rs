use leptos::prelude::window;

/// Whether the reader has asked for less movement.
///
/// Read at the moment something is about to move rather than followed, so a
/// change to the setting takes at the next thing the app animates.
pub fn calm() -> bool {
    window()
        .match_media("(prefers-reduced-motion: reduce)")
        .ok()
        .flatten()
        .is_some_and(|query| query.matches())
}
