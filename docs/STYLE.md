# Style

House rules that a formatter cannot enforce. `rustfmt` (nightly, run by the
pre-commit hook) owns everything mechanical; this document owns the rest.

## Comments

**Comments in code are uncommon.** Most code needs none — the names and the
shape say it. A comment earns its place where something is unusual: a
constraint from outside, a trap, a line that looks wrong and is not.

**Say what, not why.** A comment states what the code does or what holds here.
Reasoning goes in the commit message, or in a document if it is a rule. In a
file it goes stale and it crowds out the code around it.

```rust
// Absent rather than null. Plaid reads either as "from the beginning", and
// only absent is documented.
```

**Say what is, not what changed.** A comment describes the code as it stands,
never the edit that produced it — the reader has `git log` and `git blame`, and
those stay right when the code moves on. The same goes for words that only mean
anything next to a diff: *now*, *no longer*, *used to*, *replaced*, *instead
of*, *moved*. If the old shape was a trap somebody could fall into again, say
what the trap is.

**No banner comments.** Not these:

```rust
// ── the ledger ──────────────────────────────────────────────────────────────
```

They have to be re-padded when renamed, they encourage a file to grow past the
point where it should have been split, and they carry nothing a `mod` does not.
If a file needs sections, it needs to be two files.

**No issue numbers.** Not `(#24)`, not `see #40`. Code outlives an issue, and a
number is not a reason. If the issue explained something worth knowing, write
the something down.

**Doc comments on anything public**, and on private items whose purpose the
name does not give. One line of summary that reads as a sentence, and a few
lines at most after it.

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
a missing row. Reserve it for what is provable from the lines just above;
anything depending on input or IO gets a `Result`.

**`debug_assert!` and `debug_assert_eq!` are encouraged**, especially in state
machines. They document an invariant, fail loudly in tests and debug builds,
and compile away in release.

## State machines

The rules a machine follows are in [ARCHITECTURE.md](ARCHITECTURE.md). This is
the naming and layout that goes with them.

| Thing | Type | File |
| --- | --- | --- |
| The machine | `StateApp`, `StateServer` | `state_app.rs`, `state_server.rs` |
| The IO half | `RuntimeApp`, `RuntimeServer` | `runtime_app.rs`, `runtime_server.rs` |
| Events | `EventUi`, `EventSource` | `event_ui.rs`, `event_source.rs` |
| Actions | `ActionUi`, `ActionSource` | `action_ui.rs`, `action_source.rs` |
| View values | `ViewLedger`, `ViewTxRow` | `view_ledger.rs`, `view_tx_row.rs` |
| Wire types | `WireEntry` | `wire_entry.rs` |

The prefix goes first in both the type and the file name, so a directory
listing sorts by kind and says what it is at a glance.

**One type per file**, and one event or action per file with it. A machine's
file holds the machine; its supporting types — what it is waiting on, what a
call came to, where it is in its own life — are each their own file, because a
reader looking for the machine should find the machine. One that several crates
need goes to `api-common` instead.

## Tests

**`assert!` and `assert_eq!` are fine in tests**, and a one-line assertion on a
value is usually the clearest thing to write. What they are not for is driving a
state machine.

**Event and action testing goes through `eval!`** (`crates/api-eval`):

```rust
eval!(&mut state,
    < EventRouter::SnapshotRequested { force: false },
    > Action::ToSource { action: ActionSource::FetchBalances { item_id: "item-1" } },

    < EventRouter::SnapshotRequested { force: true },
    > Action::ToRouter { action: ActionRouter::Serve },
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
`PartialEq`.

**A `>` line that starts with `@` pins the whole action instead**, as a raw
string compared against its `{:?}`:

```rust
> @r#"ToSource { action: SendCode { email: "al@example.com" } }"#,
```

A pattern cannot reach a value: it cannot compare a `String` to a sentence, or
say that six boxes are red, or that a countdown reads `Resend in 0:18`. Use `@`
where a value is the claim and a pattern where the shape is; most tests use
both. Debug prints variants unqualified, so the string reads `ToSource { … }`
where the pattern would say `Action::ToSource { … }`. The way to write one is to
put in `@r#""#`, run the test, and paste back what it says it got — after
reading it, since what it got may be the bug.

A screen that runs to hundreds of lines of `Debug` is not worth pinning this
way; the Ledger's 48 rows stay `Render { .. }` and the assertions on them read
the view. First run's screens are small, and `tests/state_onboarding.rs` pins
every one of them whole.

**A `>` line may bind and carry a block**, which runs when the action matches:

```rust
> Action::ToUi { action: ActionUi::Render { screen: ViewScreen::Ledger { view: ledger }, .. } } => {
    assert_eq!(rows(ledger).len(), 2, "the Uber is filtered out");
},
```

This is what a screen too large to pin uses. The pattern takes out the one part
the test is about, the block says the one thing, and the rest of the render is
neither written down nor asserted.

**Every event on a `<` line is written out in full.** No helper that builds one,
no local holding one, no closure — the whole event, where it is sent.
`requested(false, vec![facts(None, false)])` cannot be read without going to
look up two definitions, which is worst in exactly the test somebody is reading
because it broke. A helper is fine *around* a test — the machine it starts from,
the screen it reads back afterwards — but not on a `<` line.

**A long event may be broken across lines.** Written out in full is the rule;
one line is not. `eval!` parses tokens, so formatting is free:

```rust
eval!(&mut state,
    < Event::FromRouter {
        request_id: 1,
        event: EventRouter::SnapshotRequested {
            force: false,
            items: vec![ItemFacts {
                item_id: "item-1".to_string(),
                cursor: None,
                synced_at: None,
                login_required: false,
            }],
        },
    },
    > Action::ToSource { call_id: 1, action: ActionSource::FetchBalances { .. } },
);
```

Time is not part of the syntax either: it is an event like any other, so a test
that is *about* time sends one.

```rust
< Event::FromProcess { event: EventProcess::Tick { now: Timestamp::from_secs(T0.as_secs() + CALL_GOOD_FOR_SECS) } },
```

That is what makes "we waited long enough" a test rather than a stopwatch.
`EventProcess::Tick` on the server, `EventRuntime::Tick` on the phone, and
neither machine has another way to be told the time.

**Nothing searches the action list.** No `.iter().find(…)` for the render, no
`.filter(…).count()` of the fetches, and no helper that hands back "the screen
that came out" — a test that goes looking never says which action carried what
it found, or how many others there were. The `>` lines are already every action
in order, so "only the first turn fetches" is the list itself rather than a
count of it.

**A test carries the smallest fixture that can make its claim.** Two
transactions on one day is what "a day heading carries its net" is about; forty
of them say the same thing while making the expected value something only the
fixture knows. `assert_eq!(shown.len(), 9)` with a comment reading *nine food
transactions in the captured feed* is a fact about a file, not a claim about the
machine, and it breaks whenever that file is regenerated. So a shared fixture is
usually the wrong shape — each test writes the snapshot it needs, and a screen
whose claim needs no records asks for none.

**A test sends only the events its claim needs.** An event that changes nothing
the test looks at is a reader's problem: they have to work out whether it
matters. The same goes for the tail after the claim — a test about whether a
fetch goes out stops at the `>` line that says so, and what happens when the
fetch lands is the next test's claim.

**A bug is reproduced before it is fixed.** Write the test first, run it, and
watch it fail for the reason you think it fails. A test written after the fix
proves the code does what it now does; a test written before it proves the bug
was real and is gone.

The architecture supplies this rather than asking for it. Everything that
decides anything lives in a state machine, and a state machine is events in and
actions out with no clock and no IO — so every logical bug here is some sequence
of events producing the wrong actions, and that sequence is an `eval!` test. If
a bug seems not to be reproducible that way, the behaviour is in a runtime that
should not be deciding it. The bug where one transaction appeared three times
was six lines: two `TransactionsLoaded` events carrying an entry the ledger
already held, and `assert_eq!(state.entries().len(), 2)`.

**`eval!` tests live in the crate's `tests/` directory**, not in a
`#[cfg(test)] mod tests` at the foot of the file they test. A machine's tests
grow with every event it learns to handle, and a state machine is a file people
read end to end.

**A machine's tests are split by the flow they are about, not by the file they
came from.** `docs/API_FLOW.md` letters the server's flows A to F, so
`crates/server-main/tests/` has `flow_a_signing_in.rs`,
`flow_b_opening_a_bank_sign_in.rs` and the rest, each opening with the flow's
happy path end to end and following it with that flow's lettered alternatives.
`crates/client-state/tests/state_onboarding.rs` is the same rule one file
earlier: first run is a flow, so it is a file.

What belongs to no flow is named `state_*` after the thing it is about — the
machine's own life (`state_lifecycle`), and the rules that guard every flow at
once (`state_limits`, `state_webhook_trust`). A test goes in the flow file when
a reader looking up that flow would want it there.

**Fixtures shared across those files live in `tests/fixtures/mod.rs`** — a
directory module, so Cargo does not compile it as a test binary of its own. It
holds only what this document already calls a helper around a test: the machine
a test starts from, the clock it is set to, a signature the machine will
believe. Nothing in it builds an event.

Unit tests on a value stay where they are, in a module beside the thing they
test. The rule is about machines, whose tests are conversations rather than
assertions on a return value.

**No snapshot testing.** Not inline, not in files, and no tool that writes the
expected value for you or updates it when it changes. A snapshot is accepted
rather than decided, and a suite of them is a suite nobody has read. `@` is not
that: it is written in by the person making the claim, one action at a time. It
does cost what a snapshot costs — add a field to a view and every `@` carrying
it fails — and that is the trade taken knowingly. When a change breaks a pile of
them, read the new value before pasting it in; that reading is the difference.

## Logging

**No `println!` or `eprintln!` anywhere.** Everything goes through `tracing`, so
every line gets a level, a target, structured fields and the commit it came
from. A print has none of those, and it is exactly the line missing from the log
we are handed. The exception is a command whose output *is* the answer — an
example, or a build script — which writes to stdout. See [LOGGING.md](LOGGING.md).

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
- `util-*` — helpers shared across crates that belong nowhere else. None exist
  yet, and it is not yet clear that any are needed; reach for `api-common`
  first.
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
and that cache is what lets a clean checkout build with no database at all — so
a query changed without regenerating it is a build that works on your machine
and nowhere else.

**No lints anywhere.** Not "no new lints" — none. A warning that is genuinely
wrong gets `#[expect(lint, reason = "…")]` with the reason written out, so the
next person knows it was considered rather than ignored.

## Imports

One `use` per item, as `rustfmt.toml` enforces with `imports_granularity`.
Groups are std, then external, then workspace, then `crate`.
