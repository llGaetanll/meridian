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
    let opening: String?
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

            OpenBankButton(icon: "bank", text: action, opening: opening)

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
///
/// `open` is the second way out, and it is the machine's call whether there is
/// one: reached from Add another bank there is a whole app behind this screen,
/// and reached from first run there is not.
struct FailedScreen: View {
    let step: String
    let title: String
    let runs: [ViewRun]
    let rows: [ViewIconRow]
    let action: String
    let opening: String?
    let open: String?

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

            // The pair reads the way it does on Ready, which is the screen this
            // one stands in for: the quieter way out above, the button the
            // screen is about below.
            OnbActions {
                if let open {
                    Button(open) { runtime.ui(.onboardingFinished) }
                        .buttonStyle(.block)
                }

                // The icon is honest about where the tap goes: Plaid's page is
                // over the web, so trying again leaves the app the same way it
                // did the first time.
                OpenBankButton(icon: "ext", text: action, opening: opening)
            }
        }
    }
}

/// The button that opens Plaid, on both screens that have one.
///
/// A second or so passes between the tap and the sheet: the server has to
/// create a Link session, then iOS has to raise a web view over the app.
/// Nothing moved on screen during it, which reads as a button that missed the
/// tap — so it says what it is doing, and stops taking taps while it does.
///
/// `opening` is the machine's, not this view's, and so are its words. Whether
/// a bank sign-in is in flight is a fact about the flow, and the flow is where
/// the screens are decided.
private struct OpenBankButton: View {
    let icon: String
    let text: String
    let opening: String?

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        Button {
            runtime.ui(.bankConnectRequested)
        } label: {
            if let opening {
                HStack(spacing: 9) {
                    Spinner(size: 17)

                    Text(opening)
                }
            } else {
                IconLabel(icon: icon, text: text)
            }
        }
        .buttonStyle(.primaryBlock)
        .disabled(opening != nil)
    }
}
