use leptos::html;
use leptos::prelude::*;
use web_ui::Glyph;

use super::CardWells;

/// Where a card is typed: the number across the measure, the date and the code
/// under it, and why a payment was refused beneath them.
///
/// The wells are drawn empty. Whoever mounts this puts the payment provider's
/// frames in them, so a card number never passes through this app — which is
/// also why nothing here reads what was typed.
#[component]
pub fn CardForm(
    /// Handed the wells once all three are in the document.
    mounted: Callback<CardWells>,

    /// While a payment is out: the card it is paying with cannot change.
    #[prop(into)]
    locked: Signal<bool>,

    #[prop(into)] fault: Signal<Option<String>>,
) -> impl IntoView {
    let number = NodeRef::<html::Div>::new();
    let expiry = NodeRef::<html::Div>::new();
    let cvc = NodeRef::<html::Div>::new();

    let handed = StoredValue::new(false);

    Effect::new(move || {
        let (Some(number), Some(expiry), Some(cvc)) = (number.get(), expiry.get(), cvc.get())
        else {
            return;
        };

        if handed.get_value() {
            return;
        }

        handed.set_value(true);

        mounted.run(CardWells {
            number,
            expiry,
            cvc,
        });
    });

    view! {
        <div class=move || if locked.get() { "card-entry locked" } else { "card-entry" }>
            <div class="field">
                <div class="in">
                    <Glyph name="card" />
                    <div node_ref=number></div>
                </div>
            </div>
            <div class="field">
                <div class="in">
                    <div node_ref=expiry></div>
                </div>
            </div>
            <div class="field">
                <div class="in">
                    <div node_ref=cvc></div>
                </div>
            </div>
        </div>

        {move || {
            fault
                .get()
                .map(|text| {
                    view! {
                        <div class="say-fault">
                            <Glyph name="warn" />
                            {text}
                        </div>
                    }
                })
        }}
    }
}
