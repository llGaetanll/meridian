//! `eval!` — the way state machines are tested.
//!
//! A state machine test is a conversation: events go in, actions come out, more
//! events go in. `<` is an event going in, `>` is an action expected out.
//!
//! ```ignore
//! eval!(&mut state,
//!     < Event::FromUi { event: EventUi::Started },
//!     > Action::ToSource { action: ActionSource::LoadSnapshot },
//!     > Action::ToUi { action: ActionUi::Render { .. } },
//! );
//! ```
//!
//! The syntax in full — batching, patterns, `@` for pinning a value, blocks
//! that bind — and the rules for writing these tests are in `docs/STYLE.md`.

use proc_macro::TokenStream;
use quote::quote;
use syn::Block;
use syn::Expr;
use syn::LitStr;
use syn::Pat;
use syn::Token;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;

/// What a `>` line says about one action.
enum Expected {
    /// `> Action::ToUi { .. }`, and the block after `=>` if the line carried
    /// one.
    ///
    /// Boxed because a `Pat` is two orders of magnitude larger than the other
    /// variant, and a parse tree is built once and never in a hot path.
    Shape(Box<Pat>, Option<Box<Block>>),

    /// `> @"ToUi { … }"` — the text an action's `{:?}` must equal.
    Debug(LitStr),
}

/// A run of events and the actions they must produce.
struct Batch {
    events: Vec<Expr>,
    actions: Vec<Expected>,
}

struct Eval {
    /// The machine, as `&mut something`.
    state: Expr,
    batches: Vec<Batch>,
}

impl Parse for Eval {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let state: Expr = input.parse()?;
        input.parse::<Token![,]>()?;

        let mut batches: Vec<Batch> = Vec::new();

        while !input.is_empty() {
            if input.peek(Token![<]) {
                input.parse::<Token![<]>()?;
                let event: Expr = input.parse()?;

                // An event after an action starts the next batch.
                let open = matches!(batches.last(), Some(batch) if batch.actions.is_empty());

                if !open {
                    batches.push(Batch {
                        events: Vec::new(),
                        actions: Vec::new(),
                    });
                }

                match batches.last_mut() {
                    Some(batch) => batch.events.push(event),
                    None => unreachable!("a batch was just pushed"),
                }
            } else if input.peek(Token![>]) {
                input.parse::<Token![>]>()?;

                let action = if input.peek(Token![@]) {
                    input.parse::<Token![@]>()?;

                    Expected::Debug(input.parse()?)
                } else {
                    // A pattern rather than an expression, so `..` and `_` work.
                    let pattern = Pat::parse_multi_with_leading_vert(input)?;

                    let then = if input.peek(Token![=>]) {
                        input.parse::<Token![=>]>()?;

                        Some(Box::new(input.parse::<Block>()?))
                    } else {
                        None
                    };

                    Expected::Shape(Box::new(pattern), then)
                };

                match batches.last_mut() {
                    Some(batch) => batch.actions.push(action),
                    None => {
                        return Err(input.error("an action needs at least one `<` event first"));
                    }
                }
            } else {
                return Err(input.error("expected `<` for an event or `>` for an action"));
            }

            if input.is_empty() {
                break;
            }

            // A block ends the line on its own brace, so the comma after one is
            // as optional as it is on a `match` arm.
            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Eval { state, batches })
    }
}

/// Drive a state machine and check what it does. See the module docs.
#[proc_macro]
pub fn eval(input: TokenStream) -> TokenStream {
    let Eval { state, batches } = parse_macro_input!(input as Eval);

    let steps = batches.iter().enumerate().map(|(i, batch)| {
        let events = &batch.events;
        let batch_number = i + 1;
        let expected = batch.actions.len();

        // One check per action, in order. Written out rather than looped so a
        // failure names the position and prints what did not match.
        let checks = batch.actions.iter().enumerate().map(|(j, action)| {
            let position = j + 1;

            match action {
                Expected::Shape(pattern, then) => {
                    // `quote` puts a space around every token, so the pattern in
                    // a failure message would read `Action :: ToUi { action : … }`.
                    // Tightening the punctuation costs nothing and the message is
                    // the only thing anyone sees when a test breaks.
                    // `quote` breaks a long pattern over lines, which reads
                    // badly in a one-line message.
                    let written = quote!(#pattern)
                        .to_string()
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .replace(" :: ", "::")
                        .replace(" : ", ": ")
                        .replace(" , ", ", ");

                    let matched = match then {
                        // The bindings are the point: whatever the pattern took
                        // out of the action is what the block asserts on.
                        Some(block) => quote!(#pattern => #block),
                        None => quote!(#pattern => {}),
                    };

                    quote! {
                        match __actions.get(#j) {
                            Some(__action) => match __action {
                                #matched
                                _ => panic!(
                                    "batch {}, action {}: expected `{}`, got {:#?}",
                                    #batch_number,
                                    #position,
                                    #written,
                                    __action,
                                ),
                            },
                            None => unreachable!("the count was checked first"),
                        }
                    }
                }

                // `{:?}` rather than `{:#?}`: one action is one line, both in
                // the test and in the message a failure prints.
                Expected::Debug(written) => quote! {
                    match __actions.get(#j) {
                        Some(__action) => {
                            let __shown = ::std::format!("{:?}", __action);

                            assert!(
                                __shown == #written,
                                "batch {}, action {}:\n    expected `{}`\n         got `{}`",
                                #batch_number,
                                #position,
                                #written,
                                __shown,
                            );
                        }
                        None => unreachable!("the count was checked first"),
                    }
                },
            }
        });

        quote! {
            {
                let __events = vec![#(#events),*];

                let mut __actions = ::std::vec::Vec::new();
                __eval_state.handle_events(&__events, &mut __actions);

                // Checked before the patterns so that a missing or extra action
                // reports as that, rather than as a mismatch three lines down.
                assert_eq!(
                    __actions.len(),
                    #expected,
                    "batch {} produced {} actions, expected {}: {:#?}",
                    #batch_number,
                    __actions.len(),
                    #expected,
                    __actions,
                );

                #(#checks)*
            }
        }
    });

    let expanded = quote! {
        {
            let __eval_state = #state;

            #(#steps)*
        }
    };

    expanded.into()
}
