# The FFI boundary

The phone apps get their views from Swift and Kotlin, because a webview does
not feel native and feel is not negotiable on a phone. Everything that decides
anything stays in Rust, shared by all five targets. UniFFI is what joins them.

This is the reasoning and the rules. `docs/ARCHITECTURE.md` covers the event
and action loop the boundary is shaped around.

## What is where

| Layer | Language | Shared by |
| --- | --- | --- |
| `api-*`, `src-*` | Rust | web, desktop, iOS, Android |
| `client-state` — events, actions, the machine | Rust | all of them |
| Web and desktop views | Rust (Leptos) | web, macOS, Windows, Linux |
| iOS views | Swift | iOS |
| Android views | Kotlin | Android |

Two non-Rust surfaces, both view-only. Nothing with a decision in it is
written twice.

## The shape of the boundary

Narrow in logic, not in types. One object, one method that matters:

```rust
impl Core {
    pub fn handle_events(&self, events: Vec<Event>) -> Vec<Action>
}
```

The aim is that **nothing above the boundary decides anything**. Swift and
Kotlin render; they do not sort, format, or work out what a row says. So the
types that cross carry finished values — `TxSummary` holds the date as the
string the row shows, not a timestamp for two platforms to format the same way
twice. Counterintuitively this makes the type surface a little larger, and that
is fine: it is the logic surface that has to stay small.

The chart is the exception, when it comes. Geometry crosses as points, not as
SVG strings, because each platform draws with its own API and pans and pinches
at 60fps without asking Rust anything. One call per data change, none per frame.

## What crosses, and what does not

Crosses cleanly:

- Plain records (`uniffi::Record`) and data enums (`uniffi::Enum`)
- `String`, `f64`, `u32`, `Vec<T>`, `Option<T>`
- Exported objects (`uniffi::Object`), which must be `Send + Sync` — hence the
  `Mutex` around `State` in `lib.rs`. The machine has no opinion about threads.

Does not cross, and what to do instead:

- **Borrows.** `handle_events(&mut self, &[Event], &mut Vec<Action>)` is the
  internal signature, kept because replay and tests want the buffer reuse. The
  exported one owns its arguments and allocates. Keep both.
- **`Range<T>`.** Not a UniFFI type. `api-chart` will need a `PriceRange`
  record when the chart moves down.
- **Lifetimes, generics, most traits.** Monomorphise at the boundary.
- **Panics.** A panic across FFI is not a Swift `throws` or a Kotlin exception.
  Return `Result` from boundary functions; never let a panic reach one.

## What the compiler guarantees

Strong at both ends and across the interface:

- Rust is Rust. Exhaustive matches, borrow checking, no nulls.
- The generated Swift and Kotlin are type-checked by their own compilers, so
  passing the wrong type into a binding fails to build.
- **Enum exhaustiveness carries across.** A Rust enum becomes a Swift enum with
  associated values and a Kotlin sealed class. Add an `Action` variant,
  regenerate, and a `switch` without a `default` fails to compile until it is
  handled. This is the single most valuable property here — it is what stops
  the two view layers drifting from the core. Do not write `default:` or
  `else ->` over these types; that throws it away deliberately.
- `Option<T>` becomes a Swift optional and a Kotlin nullable, both enforced.
- Memory management across the boundary is UniFFI's, not hand-rolled `unsafe`.

## The one seam, and what actually covers it

Nothing forces regeneration. Change a Rust type, skip the script, and Swift
happily type-checks against stale generated code. No rust-analyzer lint can see
this, because nothing in the Rust source is wrong — it is a property of the
*relationship* between the Rust and the generated files.

UniFFI covers part of it at runtime. The bindings carry a contract version and
a per-function checksum, and refuse to load a library that disagrees:

```swift
if (uniffi_client_state_checksum_method_core_handle_events() != 6646) {
    return InitializationResult.apiChecksumMismatch
}
```

**Read the limit carefully.** Those checksums hash *function signatures*. Adding
a variant to an enum a function already takes does not change the signature, so
the checksum does not move — verified by experiment, not assumed. The mismatch
fires at startup, loudly, when a signature changes; it says nothing about enum
variants drifting. And it is a runtime check either way, not a compile-time one.

So the real protection is layered, in this order:

1. **Regenerate.** `./scripts/bindings.sh`. The generated files are committed,
   so `git status` afterwards is the check: a dirty tree means someone changed
   the core without regenerating.
2. **Exhaustive switches** in Swift and Kotlin catch the added-variant case the
   checksum misses — but only if no `default` arm is written. Worth a SwiftLint
   and detekt rule once those exist.
3. **Checksums** catch signature drift at startup.

A CI check that regenerates and fails on a diff would close step 1 properly.
Deliberately not set up yet — see below.

## Running it

```
./scripts/bindings.sh
```

Builds `client-state` and regenerates both languages from the built library. The
`uniffi-bindgen` binary lives in `client-state` itself, so the generator version can
never disagree with the runtime version.

Generated output is committed:

- `swift/Sources/Meridian/` — `client_state.swift`, the C header, the modulemap
- `kotlin/src/main/kotlin/uniffi/client_state/client_state.kt`

The core is built in three shapes, which is what `crate-type` in
`crates/client-state/Cargo.toml` says: a **staticlib** for iOS, which links the
core into the app binary rather than loading one beside it; a **cdylib** for
Android, and for `uniffi-bindgen`, which reads the built library rather than
the source; and a plain **lib** so the Rust front-ends can depend on it
directly. The cdylib is the platform's own file — `.dylib` on macOS, `.so`
everywhere else — which is why `bindings.sh` works out the name rather than
spelling one.

## Proving it works

```
./scripts/kotlin-smoke.sh
```

Compiles the generated Kotlin against the real `libclient_state.so` and runs it:
events in, actions out, values checked on the way back. It has been run and it
passes, so the boundary described above is not a plan — it works.

Two things it demonstrates on purpose:

- The `when` blocks in `kotlin/src/test/kotlin/Smoke.kt` have no `else`. Adding
  an `Action` variant in Rust and regenerating breaks this file until the case
  is handled. That is the enum-exhaustiveness guarantee, in a form that fails
  loudly rather than one described in a document.
- A fieldless Rust enum (`SourceAction`) comes out as a Kotlin `enum class`
  (`SourceAction.LOAD_TXS`), while one with fields comes out as a sealed class
  matched with `is`. Worth knowing before writing a `when` against either.

Swift has no equivalent script, and does not need one: the iOS app is the
proof. `./scripts/ios-run.sh` compiles the generated Swift, links the core and
runs it in the simulator against real data, which is everything the smoke test
would check and a screen to look at as well. Android has no app yet, which is
why Kotlin still has the script.

Two things the Swift side settled that were open questions before it was
written:

- A nested Rust enum arrives as a nested Swift one, so matching an action is
  `case .toUi(let action)` and then a second `switch`. Whether that would be
  pleasant to write against was a real doubt; it is fine, and `if case` covers
  the one-armed checks in a line.
- `Option<T>` is a Swift optional and the compiler enforces it — `logoUrl`
  cannot be read without saying what happens when there is none.

## What is deliberately not done yet

- **No CI.** The preference is lints in the editor over a pipeline. Note that
  the one thing CI would actually catch — stale bindings — is exactly the thing
  a lint cannot see. Revisit when the tree drifting becomes a real problem
  rather than a hypothetical one.
- **No app on either side.** Only the generated bindings and a smoke test.
- **No SwiftUI or Compose yet.** The bindings compile and run on both sides,
  and nothing draws with them.
