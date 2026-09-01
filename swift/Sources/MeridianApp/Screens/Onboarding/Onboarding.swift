/// First run: the road that ends with a bank connected.
///
/// A switch over the arm the core handed over, the same shape `AppView` is.
/// There is no navigation stack and no step counter here — which screen this
/// is, is a fact the machine holds, so a back tap is an event rather than a
/// pop and a replay of the log lands on the screen it landed on the first time.
///
/// Plaid's own page is not among them. The bank sign-in happens on
/// `secure.plaid.com` in a web session the app opens and does not draw, so
/// there is a screen either side of it and nothing for it.
///
/// The switch is exhaustive with no `default:`. Add a `ViewOnboarding` variant
/// in Rust, regenerate, and this stops compiling until the new screen is drawn.

import SwiftUI

struct OnboardingScreen: View {
    let view: ViewOnboarding

    var body: some View {
        switch view {
        case .welcome(let title, let runs, let action, let legal):
            WelcomeScreen(title: title, runs: runs, action: action, legal: legal)

        case .email(let step, let title, let runs, let field, let action):
            EmailScreen(step: step, title: title, runs: runs, field: field, action: action)

        case .code(let step, let title, let runs, let cells, let fault, let action, let resend):
            CodeScreen(
                step: step,
                title: title,
                runs: runs,
                cells: cells,
                fault: fault,
                action: action,
                resend: resend
            )

        case .bank(let step, let title, let runs, let rows, let action, let legal):
            BankScreen(step: step, title: title, runs: runs, rows: rows, action: action, legal: legal)

        case .sync(let step, let title, let runs, let progress, let rows):
            SyncScreen(step: step, title: title, runs: runs, progress: progress, rows: rows)

        case .ready(let title, let runs, let tally, let another, let open):
            ReadyScreen(title: title, runs: runs, tally: tally, another: another, open: open)

        case .failed(let step, let title, let runs, let rows, let action):
            FailedScreen(step: step, title: title, runs: runs, rows: rows, action: action)
        }
    }
}
