use client_state::ViewTxRow;
use leptos::prelude::*;

/// One transaction, in columns: who, what kind, which account, how much.
///
/// Pending recedes as a whole row, so it reads as one provisional thing rather
/// than a row with some grey parts in it.
#[component]
pub fn TxRow(row: ViewTxRow, #[prop(into)] on_select: Callback<String>) -> impl IntoView {
    let id = row.id;

    let mark = match row.logo_url {
        Some(url) => view! { <img src=url alt="" /> }.into_any(),
        None => row.initials.into_any(),
    };

    view! {
        <div class="tx" class:pending=row.pending on:click=move |_| on_select.run(id.clone())>
            <span class="mark">{mark}</span>
            <span class="tx-name">{row.title}</span>
            <span class="tx-cat">
                <span class="t">{row.category}</span>
                {row.pending.then(|| view! { <span class="pending-tag">"Pending"</span> })}
            </span>
            <span class="tx-acct">{row.account}</span>
            <span class="tx-amt money" class:in=row.is_inflow>
                {row.amount}
            </span>
        </div>
    }
}
