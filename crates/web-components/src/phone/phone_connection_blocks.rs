use client_state::EventUi;
use client_state::ViewConnection;
use leptos::prelude::*;

use super::PhoneConnectionBlock;
use crate::live;

/// Every block on one side of the page, in the order the machine put them.
#[component]
pub fn PhoneConnectionBlocks(
    #[prop(into)] connections: Signal<Vec<ViewConnection>>,
    ui: Callback<EventUi>,
) -> impl IntoView {
    view! {
        <div class="blocks">
            <For
                each=move || connections.get()
                key=|connection| connection.id.clone()
                children=move |first| {
                    view! {
                        <PhoneConnectionBlock
                            connection=live(connections, first, |c| &c.id)
                            ui=ui
                        />
                    }
                }
            />
        </div>
    }
}
