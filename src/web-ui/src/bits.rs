//! Small presentational pieces: pills, chips, cards, stat rows, bars.

use leptos::prelude::*;

use crate::Tone;

/// A rounded status badge, e.g. `+18.40%` or `Buy`.
#[component]
pub fn Pill(#[prop(optional)] tone: Tone, children: Children) -> impl IntoView {
    view! { <span class=format!("pill {}", tone.filled_class())>{children()}</span> }
}

/// A bordered label, e.g. `ETF · USD · NYSE Arca`.
#[component]
pub fn Chip(#[prop(optional)] swatch: bool, children: Children) -> impl IntoView {
    view! {
        <span class="chip">
            {swatch.then(|| view! { <span class="swatch" /> })}
            {children()}
        </span>
    }
}

/// A boxed panel.
#[component]
pub fn Card(#[prop(optional, into)] class: String, children: Children) -> impl IntoView {
    view! { <div class=format!("card {class}")>{children()}</div> }
}

/// Head of an unboxed block: a title on the left, a quiet note on the right.
#[component]
pub fn SectionHead(
    #[prop(into)] title: String,
    #[prop(optional, into)] sub: Option<String>,
) -> impl IntoView {
    view! {
        <div class="section-head">
            <h2 class="section-title">{title}</h2>
            {sub.filter(|sub| !sub.is_empty())
                .map(|sub| view! { <span class="card-sub">{sub}</span> })}
        </div>
    }
}

/// A block in the right rail: uppercase title, optional note, free content.
#[component]
pub fn RailSection(
    #[prop(into)] title: String,
    #[prop(optional, into)] sub: Option<String>,
    children: Children,
) -> impl IntoView {
    view! {
        <div class="rail-section">
            <div class="rail-head">
                <span class="rail-title">{title}</span>
                {sub.filter(|sub| !sub.is_empty())
                    .map(|sub| view! { <span class="card-sub">{sub}</span> })}
            </div>
            {children()}
        </div>
    }
}

/// A vertical list of key/value rows.
#[component]
pub fn Stats(children: Children) -> impl IntoView {
    view! { <div class="stats">{children()}</div> }
}

#[component]
pub fn StatRow(
    #[prop(into)] label: String,
    #[prop(into)] value: String,
    #[prop(optional)] tone: Tone,
) -> impl IntoView {
    view! {
        <div class="stat-row">
            <span class="k">{label}</span>
            <span class=format!("v num {}", tone.class())>{value}</span>
        </div>
    }
}

/// A single-segment progress bar used for allocation shares.
#[component]
pub fn AllocBar(
    /// Filled share, `0.0..=1.0`.
    share: f64,
    #[prop(optional, into)] color: Option<String>,
) -> impl IntoView {
    let width = (share.clamp(0.0, 1.0)) * 100.0;
    let style = match color {
        Some(color) => format!("width:{width:.1}%;background:{color}"),
        None => format!("width:{width:.1}%"),
    };

    view! {
        <div class="alloc-bar">
            <span style=style />
        </div>
    }
}

/// The square asset badge shown next to a ticker.
#[component]
pub fn AssetBadge(#[prop(into)] symbol: String) -> impl IntoView {
    view! { <span class="logo">{symbol}</span> }
}
