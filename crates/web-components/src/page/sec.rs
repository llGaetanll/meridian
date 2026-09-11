use leptos::prelude::*;
use web_ui::Tone;

/// One part of a page: a strong rule across the column, and a heading with its
/// figure at the right.
#[component]
pub fn Sec(
    #[prop(into)] title: Signal<String>,
    /// The figure at the right of the heading.
    #[prop(optional, into)]
    value: Option<Signal<String>>,
    /// Money takes a colour in a section heading and nowhere else on the page.
    #[prop(optional)]
    tone: Tone,
    children: Children,
) -> impl IntoView {
    let value = value.map(|value| {
        view! { <span class=format!("v money {}", tone.class())>{value}</span> }
    });

    view! {
        <div class="sec">
            <div class="sec-title">
                <span>{title}</span>
                {value}
            </div>
            {children()}
        </div>
    }
}
