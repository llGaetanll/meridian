/// The whole app: one value, matched on.
///
/// There is no router and no page state. The core says which screen it is on
/// and hands over everything that screen shows, so this is a `switch` over the
/// last thing it rendered — the same shape as `crates/phone-main/src/app.rs`.
///
/// The shell is built here rather than by each screen, and there is one of it.
/// Returning a whole screen from each `case` gives every case its own
/// identity, and SwiftUI answers a change of identity by throwing the old view
/// away and building a new one — the header, the scroll view, the dock, and
/// the UIKit view the finger is resting on, on every tab. What differs between
/// screens is a title, a panel and a body, so those are what the switches
/// return.
///
/// The switches are exhaustive with no `default:`. Add a `ViewScreen` variant
/// in Rust, regenerate, and this stops compiling until the new screen is
/// drawn.

import SwiftUI

struct AppView: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()

            if let screen = runtime.screen {
                PhoneShell(
                    title: Self.title(of: screen),
                    subtitle: Self.subtitle(of: screen),
                    panel: { Self.panel(of: screen) },
                    content: { Self.content(of: screen) }
                )
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

    /// The big title. `nil` on Home, which shows the mark instead.
    private static func title(of screen: ViewScreen) -> String? {
        switch screen {
        case .home: nil
        case .ledger: "Ledger"
        case .recurring: "Recurring"
        case .balances: "Balances"
        }
    }

    private static func subtitle(of screen: ViewScreen) -> String? {
        switch screen {
        case .home: nil
        case .ledger(let view): view.subtitle
        case .recurring(let view): view.subtitle
        case .balances(let view): view.subtitle
        }
    }

    /// Drawn above the body. Only the Ledger has one.
    @ViewBuilder
    private static func panel(of screen: ViewScreen) -> some View {
        switch screen {
        case .home: EmptyView()
        case .ledger(let view): LedgerPanel(filter: view.filter)
        case .recurring: EmptyView()
        case .balances: EmptyView()
        }
    }

    @ViewBuilder
    private static func content(of screen: ViewScreen) -> some View {
        switch screen {
        case .home(let view): HomeScreen(view: view)
        case .ledger(let view): LedgerScreen(view: view)
        case .recurring(let view): RecurringScreen(view: view)
        case .balances(let view): BalancesScreen(view: view)
        }
    }
}
