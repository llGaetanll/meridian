use leptos::prelude::*;

/// One entry of a keyed list, read back out of the list as it stands.
///
/// `<For>` builds an entry once per key and keeps it, so an entry handed its
/// item as a plain value would go on drawing the first one it was given.
/// Reading it through the list by key is what lets a render update it in
/// place. The first value stands in for the moment between the item leaving
/// the list and `<For>` taking its entry away.
pub fn live<T>(list: Signal<Vec<T>>, first: T, key: fn(&T) -> &str) -> Signal<T>
where
    T: Clone + Send + Sync + 'static,
{
    let id = key(&first).to_string();

    Signal::derive(move || {
        list.with(|items| items.iter().find(|item| key(item) == id).cloned())
            .unwrap_or_else(|| first.clone())
    })
}
