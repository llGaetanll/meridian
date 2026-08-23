/// The whole app: one value, matched on.
///
/// There is no router and no page state. The core says which screen it is on,
/// so this is a `switch` over the last thing it rendered — the same shape as
/// `crates/phone-main/src/app.rs`.
///
/// The shell is built here rather than by each screen, and there is one of it.
/// A `switch` that returns a whole screen gives every case its own identity,
/// and SwiftUI answers a change of identity by throwing the old view away and
/// building a new one — the header, the scroll view, the dock, and the UIKit
/// view the finger is resting on, on every tab. What changes between screens
/// is a title and a body, so only those change.
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
                PhoneShell(title: Self.title(of: screen)) {
                    EmptyScreen()
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

    /// The big title, or `nil` on Home, which shows the mark instead.
    private static func title(of screen: ViewScreen) -> String? {
        switch screen {
        case .home: nil
        case .ledger: "Ledger"
        case .recurring: "Recurring"
        case .balances: "Balances"
        }
    }
}
