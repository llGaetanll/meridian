/// Step 2: the bank. The screen this whole flow exists for.
///
/// Everything before it is bookkeeping; this is where a stranger decides
/// whether to sign in to their bank on their phone. So the four lines are true
/// ones, including the third, which is the one apps like this usually leave
/// out — Meridian sees your transactions and stores them. That is the product,
/// and an app that implies otherwise has told its first lie on the screen where
/// it is asking to be trusted.
///
/// No skip. There is no Meridian without a bank in it — every screen in the app
/// is a view of transactions — so an app that let someone past this would open
/// on three empty tabs.

import SwiftUI

struct BankScreen: View {
    let step: String
    let title: String
    let runs: [ViewRun]
    let rows: [ViewIconRow]
    let action: String
    let legal: String

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn {
            OnbStep(text: step)
            OnbTitle(text: title)
            OnbBody(runs: runs)

            OnbRows(rows: rows)
                .padding(.top, 18)

            OnbFill()

            Button {
                runtime.ui(.bankConnectRequested)
            } label: {
                IconLabel(icon: "bank", text: action)
            }
            .buttonStyle(.primaryBlock)

            OnbLegal(text: legal)
        }
    }
}

/// The common ending, not a rare one: people close the sheet to go and find a
/// password, or their bank's own login fails.
///
/// Since there is no skip, this screen carries the way out instead — the
/// account is already made, and closing the app here loses nothing. That is the
/// difference between insisting on a bank and trapping someone on a screen.
struct FailedScreen: View {
    let step: String
    let title: String
    let runs: [ViewRun]
    let rows: [ViewIconRow]
    let action: String

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn {
            OnbStep(text: step)

            OnbFill(upTo: 36)

            Icon(name: "warn", size: 36, weight: 1.9)
                .foregroundStyle(Theme.amber300)

            OnbTitle(text: title)
                .padding(.top, 18)

            OnbBody(runs: runs)

            OnbRows(rows: rows)
                .padding(.top, 20)

            OnbFill()

            // The icon is honest about where the tap goes: Plaid's page is over
            // the web, so trying again leaves the app the same way it did the
            // first time.
            Button {
                runtime.ui(.bankConnectRequested)
            } label: {
                IconLabel(icon: "ext", text: action)
            }
            .buttonStyle(.primaryBlock)
        }
    }
}
