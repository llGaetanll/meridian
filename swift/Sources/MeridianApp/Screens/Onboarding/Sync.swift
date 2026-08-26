/// Step 3: the wait, and the end of the road.
///
/// The first sync is the only slow screen in the app, and it happens once per
/// bank. `/transactions/sync` pages the entire history the first time and
/// returns almost nothing every time after, so a spinner would make a one-off
/// minute feel like a broken app.

import SwiftUI

struct SyncScreen: View {
    let step: String
    let title: String
    let runs: [ViewRun]
    let progress: Double
    let rows: [ViewIconRow]

    var body: some View {
        OnbColumn {
            OnbStep(text: step)
            OnbTitle(text: title)
            OnbBody(runs: runs)

            // The counts climb because the pages actually land: each row is a
            // call finishing, not an animation. The bar is the design
            // language's share, standing on a rule the way it does under a
            // category.
            Track(share: progress)
                .padding(.top, 26)

            OnbRows(rows: rows)
                .padding(.top, 10)

            OnbFill()
        }
        // No Cancel. There is nothing to go back to — the bank is connected
        // and the token stored, so stopping here would leave an account
        // holding half a history.
    }
}

/// Counts rather than a tick and a slogan.
///
/// They are the first proof the thing works, and they set the expectation for
/// the Ledger the next tap opens: 48 rows and three months, so a thin-looking
/// August reads as a short history rather than a broken app.
///
/// No balance. Balances are covered by default so the app can be opened in
/// public, and putting the figure on the last screen of setup would undo that
/// on day one.
struct ReadyScreen: View {
    let title: String
    let runs: [ViewRun]
    let tally: [ViewTally]
    let another: String
    let open: String

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn {
            OnbFill(upTo: 56)

            Icon(name: "tick-circle", size: 40, weight: 1.9)
                .foregroundStyle(Theme.up)

            OnbTitle(text: title)
                .padding(.top, 18)

            OnbBody(runs: runs)

            OnbTally(rows: tally)

            OnbFill()

            OnbActions {
                // Add another bank rather than Done: most people have more
                // than one, and the second connection will never be likelier
                // than it is right now.
                Button(another) { runtime.ui(.bankConnectRequested) }
                    .buttonStyle(.block)

                Button(open) { runtime.ui(.onboardingFinished) }
                    .buttonStyle(.primaryBlock)
            }
        }
    }
}
