//! The settings window, and the confirmation that can stand over it.
//!
//! A window and not a page: nothing in here belongs to the page underneath,
//! and taking somebody off the ledger to change their name is a navigation
//! they then have to undo. It opens in the middle of the app over a scrim, and
//! the page it covers is still there when it shuts.
//!
//! The rail down its left has one page on it today. It is drawn anyway, so
//! that the second one is a row added to a window that already has a shape
//! rather than a window redrawn around it.

use client_state::EventUi;
use client_state::ViewAccountDelete;
use client_state::ViewAccountName;
use client_state::ViewSettings;
use leptos::prelude::*;

use web_ui::Icon;
use web_ui::IconKind;

/// The window, over whichever page is up.
///
/// Shut by the scrim, by the cross, and by Escape — three ways out, because a
/// window with an unsaved field in it is one people press Escape at. Nothing
/// is lost either way: the draft is the machine's and it survives the shutting.
#[component]
pub fn SettingsWindow(
    #[prop(into)] view: Signal<Option<ViewSettings>>,
    open: RwSignal<bool>,
    ui: Callback<EventUi>,
) -> impl IntoView {
    // Shutting the window while the confirmation is up takes the deletion back
    // too. A flow left half-open behind a window nobody can see is a flow that
    // resumes the next time the window is opened, which is not what pressing
    // Escape asked for.
    let shut = move || {
        if view.with(|view| view.as_ref().is_some_and(|view| view.delete.is_some())) {
            ui.run(EventUi::DeleteCancelled);
        }

        open.set(false);
    };

    let keys = window_event_listener(leptos::ev::keydown, move |event| {
        if event.key() == "Escape" && open.get_untracked() {
            shut();
        }
    });

    on_cleanup(move || keys.remove());

    move || {
        let showing = open.get().then(|| view.get()).flatten()?;

        let confirm = showing.delete.clone();
        let name = showing.name.clone();
        let delete_label = showing.delete_label.clone();

        Some(view! {
            <div class="ui settings-window">
                <div class="scrim" on:click=move |_| shut()>
                    <div class="dialog" on:click=|event| event.stop_propagation()>
                        <div class="rail">
                            <div class="rail-head">{showing.title}</div>

                            <button class="item on">
                                <Icon kind=IconKind::User />
                                {showing.account_label}
                            </button>
                        </div>

                        // The pane is the form, so Enter in any field on it
                        // saves it and the one button at the foot is what the
                        // whole page is for.
                        <form
                            class="pane"
                            on:submit=move |event| {
                                event.prevent_default();
                                ui.run(EventUi::NameSubmitted);
                            }
                        >
                            // The rail says which page this is, so the pane
                            // holds only the way out of it: a heading here
                            // would be the same two words twice.
                            <div class="pane-head">
                                <button
                                    type="button"
                                    class="shut"
                                    aria-label=showing.close_label
                                    on:click=move |_| shut()
                                >
                                    <Icon kind=IconKind::Close />
                                </button>
                            </div>

                            {name.as_ref().map(|name| view! { <NameSetting name=name.clone() ui=ui /> })}

                            // The foot of the page, however short the page is:
                            // the door with nothing behind it at one end and
                            // the one thing this page is for at the other.
                            <div class="pane-foot">
                                <div class="rule-sec" />

                                <div class="pane-acts">
                                    <button
                                        type="button"
                                        class="btn grave"
                                        on:click=move |_| ui.run(EventUi::DeleteAccountRequested)
                                    >
                                        <Icon kind=IconKind::Trash />
                                        {delete_label}
                                    </button>

                                    // Drawn in both states. Dead until the
                                    // field says something that is not already
                                    // saved, so the window's shape does not
                                    // change on the first keystroke.
                                    {name.map(|name| {
                                        view! {
                                            <button
                                                type="submit"
                                                class="btn primary"
                                                class:dead=!name.can_save
                                                disabled=!name.can_save
                                            >
                                                {name.save_label}
                                            </button>
                                        }
                                    })}
                                </div>
                            </div>
                        </form>
                    </div>
                </div>

                {confirm.map(|confirm| view! { <Confirm confirm=confirm ui=ui /> })}
            </div>
        })
    }
}

/// The name the app calls you by, and the field it is changed in.
///
/// Saved on the button at the foot of the page or on Enter, never on a
/// keystroke: a name saved as it is typed is half a dozen writes and a chip
/// that flickers through every prefix of it.
///
/// The password managers are told to leave it alone. A field asking for a name
/// is a field they offer to fill with an identity, and a vault icon sitting in
/// the corner of the one box on this page is somebody else's furniture in our
/// window.
#[component]
fn NameSetting(name: ViewAccountName, ui: Callback<EventUi>) -> impl IntoView {
    view! {
        <div class="setting">
            <label>{name.label}</label>

            <div class="setting-row">
                <span class="field">
                    <span class="in">
                        <input
                            type="text"
                            autocomplete="off"
                            autocapitalize="words"
                            spellcheck="false"
                            data-1p-ignore
                            data-lpignore="true"
                            data-bwignore
                            data-form-type="other"
                            placeholder=name.field.placeholder
                            prop:value=name.field.value
                            on:input=move |event| {
                                ui.run(EventUi::NameTyped { text: event_target_value(&event) })
                            }
                        />
                    </span>
                </span>
            </div>
        </div>
    }
}

/// The delete confirmation: the "are you sure", and then the emailed code.
///
/// The one dialog in the app that asks rather than shows, so it is the narrow
/// one with no rail. It stands over the settings window when that is what
/// opened it, and over the page on the phone.
#[component]
pub fn Confirm(confirm: ViewAccountDelete, ui: Callback<EventUi>) -> impl IntoView {
    let has_code = confirm.code.is_some();

    view! {
        <div class="scrim">
            <div class="dialog ask">
                <div class="pane">
                    <h2 class="say-title">{confirm.title}</h2>
                    <p class="say-body">{confirm.body}</p>

                    {confirm.code.map(|digits| view! {
                        <span class="field digits">
                            <span class="in">
                                <input
                                    type="text"
                                    inputmode="numeric"
                                    autocomplete="one-time-code"
                                    prop:value=digits
                                    on:input=move |event| ui.run(EventUi::DeleteCodeTyped {
                                        digits: event_target_value(&event),
                                    })
                                />
                            </span>
                        </span>
                    })}

                    {confirm.error.map(|error| view! { <p class="say-fault">{error}</p> })}

                    <div class="acts">
                        // The loud button is the safe one.
                        {confirm.cancel_label.map(|label| view! {
                            <button
                                class="btn primary"
                                on:click=move |_| ui.run(EventUi::DeleteCancelled)
                            >
                                {label}
                            </button>
                        })}
                        {confirm.confirm_label.map(|label| view! {
                            <button
                                class="btn grave"
                                on:click=move |_| ui.run(match has_code {
                                    true => EventUi::DeleteCodeSubmitted,
                                    false => EventUi::DeleteConfirmed,
                                })
                            >
                                {label}
                            </button>
                        })}
                        {confirm.busy.then(|| view! {
                            <button class="btn busy" disabled>"One moment…"</button>
                        })}
                    </div>
                </div>
            </div>
        </div>
    }
}
