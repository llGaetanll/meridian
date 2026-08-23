/// The whole app: one value, matched on.
///
/// There is no router and no page state. The core says which screen it is on
/// and hands over everything that screen shows, so this is a `switch` over the
/// last thing it rendered — the same shape as `crates/phone-main/src/app.rs`.
///
/// The switch is exhaustive with no `default:`. Add a `ViewScreen` variant in
/// Rust, regenerate, and this stops compiling until the new screen is drawn.

import SwiftUI

struct AppView: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()

            if let screen = runtime.screen {
                switch screen {
                case .home(let view): HomeScreen(view: view)
                case .ledger(let view): LedgerScreen(view: view)
                case .recurring(let view): RecurringScreen(view: view)
                case .balances(let view): BalancesScreen(view: view)
                }
            } else {
                Text("Starting…")
                    .font(.system(size: 13.5))
                    .foregroundStyle(Theme.textFaint)
            }
        }
        .preferredColorScheme(.dark)
        .foregroundStyle(Theme.text)
        .tint(Theme.accentSoft)
    }
}
