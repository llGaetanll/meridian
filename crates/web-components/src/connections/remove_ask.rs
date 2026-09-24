use client_state::EventUi;
use client_state::ViewConnectionRemove;
use leptos::prelude::*;
use web_ui::Glyph;

/// The "are you sure" under a bank's trash: the question at the head of a
/// menu, and the answer as its two rows, side by side.
///
/// A card under the button rather than a dialog over the page — one bank of
/// four is not something the whole window has to stop for, and a card standing
/// where it was pressed says which bank without the question having to be read
/// to find out. Deleting the account is the other way round, and stays a
/// dialog.
///
/// It closes on a press anywhere else, which is a listener on the window
/// rather than a sheet of nothing over the page, as the account chip's card
/// does. The listener lives as long as the card does: mounted when the machine
/// says a bank is being removed, gone when it says one is not. A press while
/// the removal is out closes nothing — the machine refuses to take it back,
/// because the bank may already be gone at Plaid.
#[component]
pub fn RemoveAsk(remove: ViewConnectionRemove, ui: Callback<EventUi>) -> impl IntoView {
    let cancel = move || ui.run(EventUi::BankRemoveCancelled);

    let away = window_event_listener(leptos::ev::click, move |_| cancel());
    on_cleanup(move || away.remove());

    let keys = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" {
            cancel();
        }
    });

    on_cleanup(move || keys.remove());

    // The safe one first, so it is the left of the two and the one that cannot
    // be undone is the one furthest from the press that opened the card. The
    // rule and the rows go together while the removal is out, rather than
    // standing empty under the sentence.
    let acts = (!remove.busy).then(|| {
        let keep = remove.cancel_label.clone();
        let take = remove.confirm_label.clone();

        view! {
            <div class="cut"></div>

            <div class="acts">
                {keep
                    .map(|label| {
                        view! {
                            <button class="item" on:click=move |_| cancel()>
                                <Glyph name="back" />
                                {label}
                            </button>
                        }
                    })}
                {take
                    .map(|label| {
                        view! {
                            <button
                                class="item grave"
                                on:click=move |_| ui.run(EventUi::BankRemoveConfirmed)
                            >
                                <Glyph name="trash" />
                                {label}
                            </button>
                        }
                    })}
            </div>
        }
    });

    view! {
        <div class="menu conn-ask" on:click=|event| event.stop_propagation()>
            <p class="say">
                <b>{remove.title}</b>
                {remove.body}
            </p>

            {remove.error.map(|error| view! { <p class="fault">{error}</p> })}

            {acts}
        </div>
    }
}
