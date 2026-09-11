use leptos::prelude::*;

/// The figure a page opens on, and what it is a figure for.
///
/// No page on the web opens on its own name — the lit nav says where you are —
/// so this is the first thing in the page's column.
#[component]
pub fn Hero(
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    /// Money arriving and leaving, unsigned: the words beside them carry the
    /// direction. The line is drawn only where the page has both.
    #[prop(optional, into)]
    flow_in: Option<Signal<String>>,
    #[prop(optional, into)] flow_out: Option<Signal<String>>,
    /// What stands at the right of the figure, such as Balances' actions.
    #[prop(optional)]
    children: Option<Children>,
) -> impl IntoView {
    let figure = view! {
        <div class="k">{label}</div>
        <div class="v money">{value}</div>
    };

    let diff = flow_in.zip(flow_out).map(|(flow_in, flow_out)| {
        view! {
            <div class="diff">
                <span class="in">
                    <b class="money">{flow_in}</b>
                    "in"
                </span>
                <span class="out">
                    <b class="money">{flow_out}</b>
                    "out"
                </span>
            </div>
        }
    });

    match children {
        Some(children) => view! {
            <div class="hero">
                <div class="hero-row">
                    <div>{figure} {diff}</div>
                    {children()}
                </div>
            </div>
        }
        .into_any(),
        None => view! { <div class="hero">{figure} {diff}</div> }.into_any(),
    }
}
