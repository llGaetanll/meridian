use leptos::ev;
use leptos::prelude::*;

/// The widest window drawn as the phone. `css/phone.css` breaks at the same
/// width, and the two have to agree.
const PHONE: f64 = 720.0;

/// Whether the window is as narrow as a phone, following it as it is resized.
///
/// Purely visual: it says which layout draws the machine's screen, and never
/// what is on it.
pub fn narrow() -> Signal<bool> {
    let narrow = RwSignal::new(measure());

    let handle = window_event_listener(ev::resize, move |_| {
        let now = measure();

        if narrow.get_untracked() != now {
            narrow.set(now);
        }
    });

    on_cleanup(move || handle.remove());

    narrow.into()
}

fn measure() -> bool {
    window()
        .inner_width()
        .ok()
        .and_then(|width| width.as_f64())
        .is_some_and(|width| width <= PHONE)
}
