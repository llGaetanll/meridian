# Style

House rules that a formatter cannot enforce. `rustfmt` (nightly, run by the
pre-commit hook) owns everything mechanical; this document owns the rest.

Where a rule has a reason, the reason is given — a rule nobody understands is a
rule that gets worked around.

## Comments

**No banner comments.** Not these:

```rust
// ── the ledger ──────────────────────────────────────────────────────────────
```

They add a maintenance burden (they have to be re-padded when renamed), they
encourage files to grow past the point where they should have been split, and
they carry no information a `mod` or a heading in the file's doc comment does
not. If a file needs sections, it probably needs to be two files.

**Say why, not what.** The code already says what it does. A comment earns its
place by explaining a decision, a constraint, or a trap:

```rust
// Absent rather than null: Plaid reads a null cursor as "from the beginning",
// which is the same thing, but only absent is documented.
```

**Doc comments on anything public**, and on private items whose purpose is not
obvious from the name. Start with a one-line summary that reads as a sentence.

## Panics

**Production code does not panic.** The server must not fall over because an
invariant we assumed turned out to be false in someone else's data, and the
state machine must not take the app down mid-session.

- No `assert!` / `assert_eq!` in non-test code.
- No `unwrap()` or `expect()` on anything that depends on input, IO, or data
  from a source. Return a `Result`, or handle the absent case.
- `expect()` is acceptable only for genuine programmer invariants that cannot
  depend on runtime data — a `Mutex` poisoned by another panic, a regex literal
  compiled at startup. Say why in the message.

**When a case is genuinely impossible, say so with `unreachable!`:**

```rust
let Some(month) = months.last_mut() else {
    unreachable!("a month was just pushed")
};
```

Better than `expect()`, which reads as "this might fail and I am choosing to
die", and better than silently skipping, which hides a broken invariant behind
a missing row. `unreachable!` says the case cannot happen and names the reason.
Reserve it for what is provable from the lines just above; anything that
depends on input or IO gets a `Result`.

**`debug_assert!` and `debug_assert_eq!` are encouraged**, especially in state
machines. They document an invariant, they fail loudly in tests and in debug
builds, and they compile away in release. If you are tempted to write
`assert_eq!` to check state, write `debug_assert_eq!` instead.

## State machines

The shape is in [ARCHITECTURE.md](ARCHITECTURE.md); this is the naming and
layout that goes with it.

| Thing | Type | File |
| --- | --- | --- |
| The machine | `StateApp`, `StateServer` | `state_app.rs`, `state_server.rs` |
| The IO half | `RuntimeApp`, `RuntimeServer` | `runtime_app.rs`, `runtime_server.rs` |
| Events | `EventUi`, `EventSource` | `event_ui.rs`, `event_source.rs` |
| Actions | `ActionUi`, `ActionSource` | `action_ui.rs`, `action_source.rs` |
| View values | `ViewLedger`, `ViewTxRow` | `view_ledger.rs`, `view_tx_row.rs` |
| Wire types | `WireEntry` | `wire_entry.rs` |

The prefix goes first in both the type and the file name so that a directory
listing sorts by kind and says what it is at a glance.

**One event or action per file.** A file per variant group scales; a single
`events.rs` does not.

**Fields are private.** State changes only through `handle_events`. A `pub`
field is a way to mutate state without an event, which is the one thing the
whole design is trying to prevent.

**Split a machine when a screen owns state nothing else reads**, and not
before. `StateLedger` exists because the filter belongs to the Ledger alone;
the entries, connections, streams and mask stayed on `StateApp` because more
than one screen reads each of them and Home reads all four. A sub-machine owns
a screen's *choices*, never its data — two copies of the ledger are two copies
that can disagree.

Sub-machines do not handle events themselves. The app's `handle_events` routes
the ones that are theirs, so there is still exactly one way anything changes.

**Derive as little as possible on events and actions.** `Debug` is needed for
logging. `Clone` only if something genuinely needs to own one — they are passed
by reference all the way down. **`PartialEq` in particular is not needed**:
`eval!` matches actions as patterns rather than comparing them, and a derive on
a top-level enum spreads a bound through every type it touches.

**No clock, no IO, no randomness inside the machine.** Time comes in as an
argument to `handle_events`. This is what makes a log of events a recording of
a session.

## Tests

**`assert!` and `assert_eq!` are fine in tests**, and a one-line assertion on a
value is usually the clearest thing to write. What they are not for is driving a
state machine.

**Event and action testing goes through `eval!`** (`crates/api-eval`):

```rust
eval!(&mut state,
    < EventClient::SnapshotRequested { force: false },
    > Action::ToSource { action: ActionSource::FetchPlaid { cursor: None } },

    < EventClient::SnapshotRequested { force: true },
    > Action::ToClient { action: ActionClient::Serve },
);
```

`<` is an event going in, `>` is an action expected out. Adjacent events go into
one `handle_events` call, and the `>` lines after them are exactly what it must
produce — in order, nothing extra, nothing missing. A `<` after a `>` starts the
next batch.

The `>` lines are **patterns**, so `..` and `_` cover the parts that do not
matter:

```rust
> Action::ToUi { action: ActionUi::Render { .. } }
```

The macro expands to `matches!`, which is why events and actions need no
`PartialEq` — pattern matching requires no trait at all, and keeping that derive
off them is what stops it spreading through every type they touch.

Time is not part of the syntax: `eval!` reads the clock off the machine, so a
test that is *about* time calls the machine's `set_now` first.

**A bug is reproduced before it is fixed.** Write the test first, run it, and
watch it fail for the reason you think it fails. Only then change the code. A
test written after the fix proves the code does what it now does; a test written
before it proves the bug was real and is gone.

This is not a discipline the architecture asks you to supply. Everything that
decides anything lives in a state machine, and a state machine is events in and
actions out with no clock and no IO — so every logical bug in this codebase is
some sequence of events producing the wrong actions, and that sequence is an
`eval!` test. If a bug seems not to be reproducible that way, the behaviour is
in a runtime that should not be deciding it.

The bug where the same transaction appeared three times was two `PlaidLoaded`
events carrying an entry the ledger already held:

```rust
eval!(&mut state,
    < loaded(vec![entry("a"), entry("b")]),
    > Action::ToClient { action: ActionClient::Serve },

    < loaded(vec![entry("a")]),
    > Action::ToClient { action: ActionClient::Serve },
);

assert_eq!(state.entries().len(), 2, "a transaction must appear once");
```

Six lines, no fixture and no server. Reproducing it that small is also what
located it: the phone's machine assigns a snapshot wholesale and could never
have doubled anything, which left one caller of `Ledger::apply` to look at.

**`eval!` tests live in the crate's `tests/` directory**, not in a
`#[cfg(test)] mod tests` at the foot of the file they test. A machine's tests
grow with every event it learns to handle, and a state machine is a file people
have to read end to end — the two do not belong in one file. One file per
machine, named after it: `crates/phone-state/tests/state_app.rs`.

Unit tests on a value stay where they are, in a module beside the thing they
test. The rule is about machines, whose tests are conversations rather than
assertions on a return value.

**No snapshot testing.** Not inline, not in files. A snapshot asserts on a
string, which means it passes or fails on formatting and `Debug` output that
nobody chose deliberately; a pattern says what the test actually cares about and
ignores the rest.

## JSON

**Formatted with 4-space indentation.** A one-line file is unreadable in a diff
and unreviewable by a person, and these files exist to be read — they are the
test corpus.

```sh
python3 -m json.tool --indent 4 in.json out.json
```

**Committed only when small, and trimmed to what the test needs.** A captured
API response is mostly fields we never parse; keeping them bloats every diff
that touches the file and lives in the history forever. Two rules:

- **Drop fields nothing reads.** Trimming Plaid's `/transactions/sync` capture
  to the eleven fields `src-plaid` parses took it from 115 KB to 32 KB.
- **Keep only as many records as the test needs.** If one entry proves the
  behaviour, commit one. Counts are the exception — a test asserting "48
  transactions across 4 months" needs all 48.

A capture that is still large after both is a sign the test wants a
hand-written fixture instead.

## Naming

- `api-*` — shared types and pure logic. No IO, no program.
- `src-*` — one external source each. IO only.
- `server-*`, `phone-*`, `web-*` — programs. Each owns its state machine, its
  runtime, and its own wire types.
- Spell words out. `server-main`, not `srv-main`.

## Commit messages

**One title, and at most two sentences under it.** The title says what changed;
the description says why, only where the why is not obvious.

Long commit messages read as thorough and are not. They restate the diff, argue
with decisions already made, and bury the one sentence that mattered — and the
next person greps the log rather than reading it.

## Before every commit

Both of these, every time. The pre-commit hook runs the formatter, but it is not
a substitute for checking:

```sh
cargo +nightly fmt --all
cargo clippy --workspace --all-targets
cargo sqlx prepare --workspace     # only when a SQL query changed
```

**Nightly**, because `rustfmt.toml` uses unstable options — stable `cargo fmt`
silently skips them and leaves the formatting half-applied, which shows up as
noise in the next diff.

`cargo sqlx prepare` regenerates `.sqlx`, the checked-query cache at the repo
root. Every `sqlx::query!` is verified against the live schema at compile time,
and that cache is what lets a clean checkout build with no database at all —
so a query changed without regenerating it is a build that works on your
machine and nowhere else. Only needed when the SQL or the schema moved.

**No lints anywhere.** Not "no new lints" — none. A warning that is genuinely
wrong gets `#[expect(lint, reason = "…")]` with the reason written out, so the
next person knows it was considered rather than ignored.

## Imports

One `use` per item, as `rustfmt.toml` enforces with `imports_granularity`.
Groups are std, then external, then workspace, then `crate`.
