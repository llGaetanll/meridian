use client_state::ViewOnboarding;
use leptos::prelude::*;

use super::IconRows;
use super::Runs;
use super::read;
use super::read::part;

/// The one slow screen, and it happens once per bank. The rows climb because
/// the pages actually land, and the bar is the design language's share,
/// standing on a rule.
///
/// Nothing to press: the bank is connected, and stopping here would leave an
/// account holding half a history.
#[component]
pub(super) fn Syncing(view: Signal<ViewOnboarding>) -> impl IntoView {
    let step = part(view, |view| read::step(view).to_string());
    let title = part(view, |view| read::title(view).to_string());
    let progress = part(view, read::progress);

    view! {
        <div class="say-step">{move || step.get()}</div>
        <div class="say-title">{move || title.get()}</div>
        <Runs view=view />

        <div class="sync-bar rule-row">
            <span class="track">
                <i style=move || format!("width:{:.1}%", progress.get().clamp(0.0, 1.0) * 100.0) />
            </span>
        </div>

        <IconRows view=view />
    }
}
