/// What somebody sees when the session is good and has not been paid for.
///
/// **Not an error screen.** The account exists, the token works, and the
/// server is answering — it refuses every route that feeds a screen, so there
/// is nothing behind this to draw. It stands in front of first run for the
/// same reason: connecting a bank is one of those routes.
///
/// Built out of first run's column, which is the design language's screen that
/// says one thing. Nothing here can take money yet, so the only control is the
/// way out.

import SwiftUI

struct PaywallScreen: View {
    let view: ViewPaywall

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn {
            OnbFill(upTo: 66)

            BrandMark(size: 50)
                .foregroundStyle(Theme.accentBright)
                .padding(.bottom, 20)

            OnbTitle(text: view.title)
            OnbBody(runs: view.body)

            OnbRows(rows: view.rows)
                .padding(.top, 22)

            OnbFill()

            OnbActions {
                Button(view.signOut) { runtime.ui(.signOutRequested) }
                    .buttonStyle(.block)
            }
        }
    }
}
