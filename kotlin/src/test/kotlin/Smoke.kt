// Proves the FFI boundary actually works: Kotlin sends events into the Rust
// state machine and gets whole screens back, with the data intact.
//
// Run with ./scripts/kotlin-smoke.sh. Not a Gradle project yet — there is no
// Android app to attach it to, and a compile-and-run is enough to catch the
// boundary breaking.
//
// The `when` blocks below deliberately have no `else`. That is the guarantee
// worth protecting: add a variant to Action or ViewScreen in Rust, regenerate,
// and this stops compiling until the new case is handled. See docs/FFI.md.
//
// It doubles as the shortest worked example of what an Android view layer has
// to do, which is very little: send an event, match the action, draw the
// strings it was handed. Every number below arrives already formatted.

import uniffi.phone_state.*

fun main() {
    val core = Core()

    // 2026-08-19, the day the sketches are drawn against. The clock arrives as
    // an event rather than being read, which is what makes a replay land in the
    // same place.
    core.handleEvents(listOf(Event.FromRuntime(EventRuntime.Tick(1_787_097_600UL))))

    // ── starting up ────────────────────────────────────────────────────────

    val started = core.handleEvents(listOf(Event.FromUi(EventUi.Started)))

    for (action in started) {
        println("action: ${describe(action)}")
    }

    // Starting must ask for data, or the app sits empty forever.
    check(
        started.any { it is Action.ToSource && it.action is ActionSource.LoadSnapshot },
    ) { "starting should ask for a snapshot" }

    // ── feeding a snapshot back in ─────────────────────────────────────────

    // The snapshot the server sends, written out here the way the Rust tests
    // write theirs: the fewest records that exercise the shapes crossing FFI.
    val snapshot = """
        {
            "now": 1787097600,
            "entries": [
                {"id":"a","ts":1786492800,"amount":-12.00,"asset":"USD","label":"McDonald's","category":"FoodAndDrink"},
                {"id":"b","ts":1786492800,"amount":-5.40,"asset":"USD","label":"Uber","category":"Transportation"}
            ],
            "connections": [
                {"id":"c","name":"First Platypus Bank","initials":"FP","status":"ok","accounts":[
                    {"id":"acc-1","name":"Plaid Checking","balance":100.0,"group":"depository"},
                    {"id":"acc-2","name":"Plaid Mortgage","balance":56302.06,"group":"loan"}
                ]}
            ]
        }
    """.trimIndent()

    val loaded = core.handleEvents(listOf(Event.FromSource(EventSource.SnapshotLoaded(snapshot))))

    val home = loaded.filterIsInstance<Action.ToUi>()
        .map { it.action }
        .filterIsInstance<ActionUi.Render>()
        .last()

    val homeView = home.screen
    check(homeView is ViewScreen.Home) { "the app opens on Home" }

    // Every figure arrives as text. Kotlin formats nothing.
    println("home balances: ${homeView.view.balances.headline} (${homeView.view.balances.detail})")
    println("home recurring: ${homeView.view.recurring.headline} (${homeView.view.recurring.detail})")
    println("home ledger rows: ${homeView.view.ledger.size}")

    // Covered by default, so the app can be opened in public.
    check(homeView.view.balances.masked) { "figures start covered" }
    check(!homeView.view.balances.headline.contains("8")) {
        "a covered figure must not travel in the clear"
    }

    // ── revealing, and switching screens ───────────────────────────────────

    core.handleEvents(listOf(Event.FromUi(EventUi.MaskToggled)))

    val balances = core.handleEvents(
        listOf(Event.FromUi(EventUi.ScreenOpened(Screen.BALANCES))),
    ).render()

    val balancesView = balances.screen
    check(balancesView is ViewScreen.Balances)

    println("held: ${balancesView.view.headline} — ${balancesView.view.subtitle}")
    check(balancesView.view.headline == "$100.00") {
        "expected the snapshot's held total, got ${balancesView.view.headline}"
    }

    // The dock says which tab is lit; the view does not work it out.
    val lit = balances.dock.items.filter { it.selected }
    check(lit.size == 1 && lit.first().screen == Screen.BALANCES)
    println("dock: ${balances.dock.items.joinToString { if (it.selected) "[${it.label}]" else it.label }}")

    // Debts come back as their own blocks, already split and totalled.
    val debts = balancesView.view.connections.filter { it.isDebts }
    check(debts.isNotEmpty()) { "the snapshot has a mortgage in it" }
    println("owed: ${debts.first().total} across ${debts.first().accounts.size} accounts")

    // ── filtering ──────────────────────────────────────────────────────────

    val filtered = core.handleEvents(
        listOf(
            Event.FromUi(EventUi.ScreenOpened(Screen.LEDGER)),
            Event.FromUi(EventUi.CategoryToggled("FoodAndDrink")),
        ),
    ).render()

    val ledgerView = filtered.screen
    check(ledgerView is ViewScreen.Ledger)

    val rows = ledgerView.view.months.flatMap { it.days }.flatMap { it.rows }
    println("food & drink: ${rows.size} rows, first is ${rows.first().title} ${rows.first().amount}")

    check(rows.all { it.category == "Food & drink" }) { "the filter should narrow the list" }
    check(ledgerView.view.filter.active) { "the panel should say it is filtering" }

    // Grouping arrives done: months, then days, then rows.
    val month = ledgerView.view.months.first()
    println("grouped under ${month.name} ${month.year} / ${month.days.first().heading}")

    // ── Option<T> is a Kotlin nullable, enforced ───────────────────────────

    val logo: String? = rows.first().logoUrl
    println("nullable logo: $logo")

    println("SMOKE OK")
}

/// Pulls the last `Render` out of a batch of actions.
///
/// A real view layer does exactly this: the core sends the whole screen on
/// every event, so the newest render is all there is to draw.
private fun List<Action>.render(): ActionUi.Render =
    filterIsInstance<Action.ToUi>()
        .map { it.action }
        .filterIsInstance<ActionUi.Render>()
        .last()

/// Exhaustive `when` with no `else` — the guarantee that matters. Add a
/// variant to Action in Rust, regenerate, and this stops compiling until the
/// new case is handled.
private fun describe(action: Action): String = when (action) {
    is Action.ToUi -> when (val a = action.action) {
        is ActionUi.Render -> "render ${describeScreen(a.screen)}"
        is ActionUi.ShowError -> "error: ${a.message}"
    }

    is Action.ToSource -> when (val a = action.action) {
        is ActionSource.LoadSnapshot -> "load snapshot (force=${a.force})"
    }
}

private fun describeScreen(screen: ViewScreen): String = when (screen) {
    is ViewScreen.Home -> "home"
    is ViewScreen.Ledger -> "ledger"
    is ViewScreen.Recurring -> "recurring"
    is ViewScreen.Balances -> "balances"
}
