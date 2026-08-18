// Proves the FFI boundary actually works: Kotlin sends events into the Rust
// state machine and gets actions back, with the data intact.
//
// Run with ./scripts/kotlin-smoke.sh. Not a Gradle project yet — there is no
// Android app to attach it to, and a compile-and-run is enough to catch the
// boundary breaking.
//
// The `when` blocks below deliberately have no `else`. That is the guarantee
// worth protecting: add a variant to Action in Rust, regenerate, and this
// stops compiling until the new case is handled. See docs/FFI.md.

import uniffi.api_core.*

fun main() {
    val core = Core()

    // Opening the screen should ask a source for the transactions.
    val opened = core.handleEvents(
        listOf(Event.FromUi(UiEvent.ScreenOpened(Screen.TRANSACTIONS)))
    )

    // Exhaustive `when` with no `else` — the guarantee that matters. If a
    // variant is added to Action in Rust and this is regenerated, this stops
    // compiling until it is handled.
    for (action in opened) {
        val described = when (action) {
            is Action.ToUi -> when (val a = action.action) {
                is UiAction.ShowTxs -> "show ${a.txs.size} txs"
                is UiAction.ShowError -> "error: ${a.message}"
            }
            is Action.ToSource -> when (action.action) {
                SourceAction.LOAD_TXS -> "load txs"
            }
        }
        println("action: $described")
    }

    // Feed a loaded transaction back in and see it come out as a view action.
    val loaded = core.handleEvents(
        listOf(
            Event.FromSource(
                SourceEvent.TxsLoaded(
                    listOf(
                        TxSummary(
                            kind = "Trade",
                            date = "Jun 12, 2026",
                            from = "6,124.00 USD",
                            to = "10.00 SPY",
                        )
                    )
                )
            )
        )
    )

    val first = loaded.first()
    check(first is Action.ToUi)
    val ui = first.action
    check(ui is UiAction.ShowTxs)
    println("rendered: ${ui.txs.first().kind} ${ui.txs.first().to}, from=${ui.txs.first().from}")

    // Option<String> arrived as a Kotlin nullable.
    val absent: String? = ui.txs.first().from
    println("nullable from: $absent")

    println("SMOKE OK")
}
