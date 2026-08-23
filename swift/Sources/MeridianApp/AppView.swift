/// The whole app: one value, matched on.
///
/// There is no router and no page state. The core says which screen it is on,
/// so this is a `switch` over the last thing it rendered — the same shape as
/// `crates/phone-main/src/app.rs`.
///
/// Every screen is empty for now, so nothing here reads the view the core
/// hands over. The switch is still exhaustive with no `default:`: add a
/// `ViewScreen` variant in Rust, regenerate, and this stops compiling until
/// the new screen is drawn.

import SwiftUI

struct AppView: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()

            if let screen = runtime.screen {
                switch screen {
                case .home: EmptyScreen()
                case .ledger: EmptyScreen(title: "Ledger")
                case .recurring: EmptyScreen(title: "Recurring")
                case .balances: EmptyScreen(title: "Balances")
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
