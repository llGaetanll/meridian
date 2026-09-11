use client_state::ViewIconRow;
use client_state::ViewRowTone;
use leptos::prelude::*;
use web_ui::Glyph;

/// The design language's row, drawn from the machine's `ViewIconRow`: an icon,
/// a line, a line under it, and something on the right.
///
/// The tone is the machine's answer and the only thing that colours it. Each
/// part redraws only when it changes, so a row that a render moves on keeps
/// its element, and a spinner in it keeps turning rather than starting over.
#[component]
pub fn IconRow(#[prop(into)] row: Signal<ViewIconRow>) -> impl IntoView {
    let icon = Memo::new(move |_| row.with(|row| row.icon.clone()));
    let tone = Memo::new(move |_| row.with(|row| row.tone));
    let title = Memo::new(move |_| row.with(|row| row.title.clone()));
    let sub = Memo::new(move |_| row.with(|row| row.sub.clone()));
    let value = Memo::new(move |_| row.with(|row| row.value.clone()));

    // A second line that is a sentence lifts the icon to the first line.
    let class = move || {
        let top = if sub.with(Option::is_some) {
            " top"
        } else {
            ""
        };

        let tone = match tone.get() {
            ViewRowTone::Plain => "",
            ViewRowTone::Done => " done",
            ViewRowTone::Running => " now",
            ViewRowTone::Waiting => " wait",
        };

        format!("row{top}{tone}")
    };

    view! {
        <div class=class>
            <Glyph name=icon />
            <div class="rt">
                {move || title.get()}
                {move || sub.get().map(|sub| view! { <div class="rs">{sub}</div> })}
            </div>
            {move || value.get().map(|value| view! { <span class="rv">{value}</span> })}
        </div>
    }
}
