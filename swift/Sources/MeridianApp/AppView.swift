/// The whole app: one value, matched on.
///
/// There is no router and no page state. The core says which screen it is on
/// and hands over everything that screen shows, so this is a `switch` over the
/// last thing it rendered. It is the only place a `ViewScreen` becomes a
/// screen, and Kotlin's will be the same switch.
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

    /// Whether the opening screen has had its moment. Presentation rather than
    /// a decision, so it is the view's and not the machine's.
    @State private var opened = false

    var body: some View {
        ZStack {
            Theme.bg.ignoresSafeArea()

            if let screen = runtime.screen, opened {
                Self.app(of: screen)
            } else {
                BootScreen()
                    .transition(.opacity)
            }
        }
        .task {
            try? await Task.sleep(for: .milliseconds(BootScreen.hold))
            withAnimation(.easeInOut(duration: 0.3)) { opened = true }
        }
        .preferredColorScheme(.dark)
        .foregroundStyle(Theme.text)
        .tint(Theme.accentSoft)
    }

    /// First run and the paywall get no shell. Their screens are not tabs, so
    /// there is no header to put over them, no band to slide in and no dock to
    /// float under them — first run is a road with an end rather than a place
    /// in the app you can navigate back to, and behind the paywall there is no
    /// app to navigate.
    @ViewBuilder
    private static func app(of screen: ViewScreen) -> some View {
        switch screen {
        case .onboarding(let view):
            OnboardingScreen(view: view)
                .transition(.opacity)

        case .paywall(let view):
            PaywallScreen(view: view)
                .transition(.opacity)

        case .ledger, .recurring, .balances, .account:
            PhoneShell(
                title: Self.title(of: screen),
                screen: Self.title(of: screen),
                band: { scroller in Self.band(of: screen, scroller: scroller) },
                content: { Self.content(of: screen) }
            )
        }
    }

    /// The three below are only ever reached through the shell, which `app` is
    /// what routes to — so first run's arm in each is what keeps the switch
    /// exhaustive rather than a screen anyone draws.
    private static func title(of screen: ViewScreen) -> String {
        switch screen {
        case .ledger: "Ledger"
        case .recurring: "Recurring"
        case .balances: "Balances"
        case .account: "Account"
        case .onboarding, .paywall: ""
        }
    }

    /// What a page's headline becomes once it has scrolled away. Every screen
    /// has one; the Ledger's is what its filter is set to, the other two are
    /// the figure the page opens on.
    @ViewBuilder
    private static func band(of screen: ViewScreen, scroller: ScrollViewProxy) -> some View {
        switch screen {
        case .ledger(let view): LedgerBand(filter: view.filter, scroller: scroller)
        case .recurring(let view): RecurringBand(view: view)
        case .balances(let view): BalancesBand(view: view)
        // No headline to stand in for, so nothing on this screen wears
        // `bandAnchor()` and the band is never asked for.
        case .account: EmptyView()
        case .onboarding, .paywall: EmptyView()
        }
    }

    @ViewBuilder
    private static func content(of screen: ViewScreen) -> some View {
        switch screen {
        case .ledger(let view): LedgerScreen(view: view)
        case .recurring(let view): RecurringScreen(view: view)
        case .balances(let view): BalancesScreen(view: view)
        case .account(let view): AccountScreen(view: view)
        case .onboarding, .paywall: EmptyView()
        }
    }
}
