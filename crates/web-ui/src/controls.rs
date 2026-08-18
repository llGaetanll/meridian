//! Interactive controls: buttons, segmented ranges, tab strips, search.

use leptos::prelude::*;

use crate::Icon;
use crate::IconKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BtnVariant {
    #[default]
    Default,
    Primary,
}

#[component]
pub fn Btn(
    #[prop(optional)] variant: BtnVariant,
    #[prop(optional)] block: bool,
    #[prop(optional)] on_click: Option<Callback<()>>,
    children: Children,
) -> impl IntoView {
    let mut class = String::from("btn");
    if variant == BtnVariant::Primary {
        class.push_str(" primary");
    }
    if block {
        class.push_str(" block");
    }

    view! {
        <button
            class=class
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb.run(());
                }
            }
        >
            {children()}
        </button>
    }
}

/// A boxed set of mutually exclusive choices, e.g. `1D 1W 1M 1Y`.
///
/// Owns its selection unless the caller passes a signal to drive it.
#[component]
pub fn Seg(
    options: Vec<&'static str>,
    #[prop(optional)] selected: Option<RwSignal<usize>>,
    #[prop(optional, default = 0)] default_selected: usize,
    #[prop(optional)] on_select: Option<Callback<usize>>,
) -> impl IntoView {
    let selected = selected.unwrap_or_else(|| RwSignal::new(default_selected));

    view! {
        <div class="seg">
            {options
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    view! {
                        <button
                            class=move || if selected.get() == i { "active" } else { "" }
                            on:click=move |_| {
                                selected.set(i);
                                if let Some(cb) = on_select {
                                    cb.run(i);
                                }
                            }
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// An unboxed set of mutually exclusive choices, used above tables and charts.
#[component]
pub fn Tabs(
    options: Vec<&'static str>,
    #[prop(optional)] selected: Option<RwSignal<usize>>,
    #[prop(optional, default = 0)] default_selected: usize,
    #[prop(optional)] on_select: Option<Callback<usize>>,
) -> impl IntoView {
    let selected = selected.unwrap_or_else(|| RwSignal::new(default_selected));

    view! {
        <div class="tabs">
            {options
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    view! {
                        <button
                            class=move || if selected.get() == i { "active" } else { "" }
                            on:click=move |_| {
                                selected.set(i);
                                if let Some(cb) = on_select {
                                    cb.run(i);
                                }
                            }
                        >
                            {label}
                        </button>
                    }
                })
                .collect_view()}
        </div>
    }
}

/// A text button that reads like a link, with an optional trailing chevron.
#[component]
pub fn LinkButton(
    #[prop(optional)] chevron: bool,
    #[prop(optional)] on_click: Option<Callback<()>>,
    children: Children,
) -> impl IntoView {
    view! {
        <button
            class="linkish"
            on:click=move |_| {
                if let Some(cb) = on_click {
                    cb.run(());
                }
            }
        >
            {children()}
            {chevron.then(|| view! { <Icon kind=IconKind::ChevronDown /> })}
        </button>
    }
}

#[component]
pub fn SearchBar(
    #[prop(into, default = String::from("Search assets, trades, merchants…"))] placeholder: String,
    #[prop(into, default = String::from("⌘K"))] shortcut: String,
) -> impl IntoView {
    view! {
        <div class="searchrow">
            <div class="search">
                <Icon kind=IconKind::Search />
                <input placeholder=placeholder />
                <span class="kbd">{shortcut}</span>
            </div>
        </div>
    }
}
