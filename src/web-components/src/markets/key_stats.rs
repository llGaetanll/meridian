use leptos::prelude::*;
use web_ui::RailSection;
use web_ui::StatRow;
use web_ui::Stats;

/// Free-form key/value facts about an asset, straight from its data source.
#[component]
pub fn KeyStats(
    #[prop(into)] source: String,
    stats: Vec<(String, String)>,
    #[prop(into, default = String::from("Key stats"))] title: String,
) -> impl IntoView {
    view! {
        <RailSection title=title sub=source>
            <Stats>
                {stats
                    .into_iter()
                    .map(|(label, value)| view! { <StatRow label=label value=value /> })
                    .collect_view()}
            </Stats>
        </RailSection>
    }
}
