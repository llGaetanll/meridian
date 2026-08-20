//! `eval!` — the way state machines are tested.
//!
//! A state machine test is a conversation: events go in, actions come out,
//! more events go in. Written by hand that is a wall of `Vec::new()`,
//! `handle_events` and index arithmetic, and the shape of the conversation is
//! the thing that gets lost.
//!
//! ```ignore
//! eval!(&mut state,
//!     < EventUi::Started;
//!
//!     < EventUi::ScreenOpened { screen: Screen::Ledger },
//!     < EventUi::CategoryToggled { id: "FoodAndDrink".into() },
//!     @r"
//!     [1] ToSource { action: LoadSnapshot { force: false } }
//!     [2] ToUi { action: Render { … } }
//!     ",
//! );
//! ```
//!
//! `;` ends a batch: everything before it goes into `handle_events` together,
//! and the next `<` starts a fresh call. It has to be written out rather than
//! inferred from a blank line, because a proc macro sees tokens and not
//! whitespace — `proc_macro2`'s span line numbers are only real when it parses
//! source itself, and under a real compiler they are all zero.
//!
//! The numbers in the snapshot say which batch each action came from.
//!
//! `<` is an event going in. A run of adjacent events is one batch; the whole
//! conversation is asserted by the single `@"…"` at the end — an **inline**
//! `insta` snapshot, written by `cargo insta test --accept` and reviewed in the
//! diff like any other code.
//!
//! One snapshot per `eval!` rather than one per batch, because `cargo insta`
//! rewrites source by span and every assertion a proc macro expands to shares
//! the macro's call site. Two inline assertions in one expansion means it can
//! only ever fill in one of them. The batches are labelled inside the snapshot
//! instead, which reads better anyway — the whole exchange is one block.
//!
//! Inline rather than a snapshot file, per `docs/STYLE.md`: the expected output
//! sits beside the events that produced it, so a test reads top to bottom
//! without opening a second file, and a reviewer sees a behaviour change in the
//! same hunk as the code change.
//!
//! Snapshots also mean events and actions need no `PartialEq`, which keeps that
//! derive from spreading through every type they touch.
//!
//! Time is not part of this syntax. A machine takes `now` as an argument to
//! `handle_events`, and a test that cares about time sets it with the machine's
//! own test-only setter before calling `eval!`.

use proc_macro::TokenStream;
use quote::quote;
use syn::Expr;
use syn::LitStr;
use syn::Token;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;

struct Eval {
    /// The machine, as `&mut something`.
    state: Expr,

    /// One `Vec<Expr>` per batch.
    batches: Vec<Vec<Expr>>,

    /// The one `@"…"` for the whole conversation. Absent until `cargo insta`
    /// fills it in.
    snapshot: Option<LitStr>,
}

impl Parse for Eval {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let state: Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut batches: Vec<Vec<Expr>> = Vec::new();
        let mut snapshot = None;

        // Set once a batch is closed; the next event opens a fresh one.
        let mut open_batch = false;

        while !input.is_empty() {
            if input.peek(Token![<]) {
                input.parse::<Token![<]>()?;
                let event: Expr = input.parse()?;

                if open_batch {
                    match batches.last_mut() {
                        Some(batch) => batch.push(event),
                        None => unreachable!("open_batch implies a batch exists"),
                    }
                } else {
                    batches.push(vec![event]);
                    open_batch = true;
                }

                // `< event;` — the semicolon closes the batch and terminates
                // the item, so no comma follows it.
                if input.peek(Token![;]) {
                    input.parse::<Token![;]>()?;
                    open_batch = false;
                    continue;
                }
            } else if input.peek(Token![@]) {
                input.parse::<Token![@]>()?;

                if snapshot.is_some() {
                    return Err(input.error("only one snapshot per eval!"));
                }

                snapshot = Some(input.parse::<LitStr>()?);
            } else {
                return Err(
                    input.error("expected `<` for an event, `;` to end a batch, or `@\"…\"`")
                );
            }

            if input.is_empty() {
                break;
            }

            input.parse::<Token![,]>()?;
        }

        Ok(Eval {
            state,
            batches,
            snapshot,
        })
    }
}

/// Drive a state machine and snapshot what it does. See the module docs.
#[proc_macro]
pub fn eval(input: TokenStream) -> TokenStream {
    let Eval {
        state,
        batches,
        snapshot,
    } = parse_macro_input!(input as Eval);

    // An empty literal is what a new test is written with; `cargo insta test
    // --accept` replaces it in the source.
    let snapshot = snapshot.unwrap_or_else(|| LitStr::new("", proc_macro2::Span::call_site()));

    let steps = batches.iter().enumerate().map(|(i, events)| {
        let batch = i + 1;

        quote! {
            {
                let __events = vec![#(#events),*];

                let mut __actions = ::std::vec::Vec::new();
                __eval_state.handle_events(__eval_now, &__events, &mut __actions);

                for __action in &__actions {
                    __eval_log.push(format!("[{}] {:#?}", #batch, __action));
                }
            }
        }
    });

    let expanded = quote! {
        {
            let __eval_state = #state;

            // Whatever the machine already holds. A test that cares about time
            // sets it beforehand, which keeps the clock out of this syntax.
            let __eval_now = __eval_state.now();

            let mut __eval_log: ::std::vec::Vec<::std::string::String> = ::std::vec::Vec::new();

            #(#steps)*

            ::insta::assert_snapshot!(__eval_log.join("\n"), @#snapshot);
        }
    };

    expanded.into()
}
