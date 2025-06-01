use leptos::prelude::*;
use leptos_meta::Stylesheet;

#[component]
pub fn App() -> impl IntoView {
    view! {
        <Stylesheet id="leptos" href="/style/output.css"/>
        <div class="flex flex-row h-screen bg-gray-950">
            <div class="flex flex-1">
                <h1 class="text-3xl font-bold text-white">"BTC"</h1>
            </div>
            <div class="flex flex-col w-[300px] border-l border-gray-900">
                <div class="bg-red-300 h-[100px]">
                    <h1>"1BTC -> 100,000USD"</h1>
                </div>
            </div>
        </div>
    }
}
