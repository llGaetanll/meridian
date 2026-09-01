/// Step 1: who you are. One door, and no password anywhere in it.
///
/// There is no Sign up and no Sign in. An address that exists signs in and one
/// that does not signs up, and until the code comes back the server cannot tell
/// them apart anyway — which removes the commonest first-run dead end, a
/// returning user tapping Create account and being told they already exist.

import Combine
import SwiftUI

struct WelcomeScreen: View {
    let title: String
    let runs: [ViewRun]
    let action: String
    let legal: String

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn {
            OnbFill(upTo: 66)

            BrandMark(size: 50)
                .foregroundStyle(Theme.accentBright)
                .padding(.bottom, 20)

            OnbTitle(text: title)
            OnbBody(runs: runs)

            OnbFill()

            Button {
                runtime.ui(.emailChosen)
            } label: {
                IconLabel(icon: "mail", text: action)
            }
            .buttonStyle(.primaryBlock)

            OnbLegal(text: legal)
        }
    }
}

struct EmailScreen: View {
    let step: String
    let title: String
    let runs: [ViewRun]
    let field: ViewField
    let action: String

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn(back: { runtime.ui(.backPressed) }) {
            OnbStep(text: step)
            OnbTitle(text: title)
            OnbBody(runs: runs)

            // The keyboard covers the bottom third the moment this opens, so
            // the button sits under the field rather than on an edge the thumb
            // cannot reach.
            Field(
                field: field,
                onChange: { runtime.ui(.emailTyped(text: $0)) },
                onSubmit: { runtime.ui(.emailSubmitted) }
            )
            .padding(.top, 24)

            if let fault = field.fault {
                OnbFault(message: fault)
            }

            Button(action) { runtime.ui(.emailSubmitted) }
                .buttonStyle(.primaryBlock)
                .padding(.top, 12)

            OnbFill()
        }
    }
}

/// Six boxes and no Continue: there is nothing to check before the sixth digit,
/// so the sixth digit submits. iOS fills it from the notification, which makes
/// this screen a flash for most people.
struct CodeScreen: View {
    let step: String
    let title: String
    let runs: [ViewRun]
    let cells: [ViewCodeCell]
    let fault: String?
    let resend: ViewResend

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn(back: { runtime.ui(.backPressed) }) {
            OnbStep(text: step)
            OnbTitle(text: title)
            OnbBody(runs: runs)

            CodeField(cells: cells, onChange: { runtime.ui(.codeTyped(digits: $0)) })
                .padding(.top, 26)

            if let fault {
                OnbFault(message: fault)
            }

            switch resend {
            case .waiting(let text, let wrongAddress):
                HStack(spacing: 0) {
                    Text("\(text) · ")
                        .foregroundStyle(Theme.textFaint)

                    // The address is one tap from being fixed. A typo in the
                    // step before is otherwise a dead end: the code never
                    // arrives and nothing here says why.
                    Button(wrongAddress) { runtime.ui(.backPressed) }
                        .buttonStyle(.quiet)
                }
                .font(.system(size: 13))
                .padding(.top, 10)

            case .ready(let label):
                Button(label) { runtime.ui(.resendRequested) }
                    .buttonStyle(.block)
                    .padding(.top, 16)
            }

            OnbFill()
        }
    }
}
