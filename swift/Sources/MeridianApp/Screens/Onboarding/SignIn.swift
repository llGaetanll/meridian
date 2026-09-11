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
    let fault: String?
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

            if let fault {
                OnbFault(message: fault)
            }

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

/// Six boxes and a Continue that appears once all six are in.
///
/// Typing does not submit. It used to — the sixth digit went straight to the
/// server — and the typo is what that missed: a wrong sixth digit spent the
/// guess before the user had seen what they had typed.
struct CodeScreen: View {
    let step: String
    let title: String
    let runs: [ViewRun]
    let cells: [ViewCodeCell]
    let fault: String?
    let action: String?
    let resend: ViewResend

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        OnbColumn(back: { runtime.ui(.backPressed) }) {
            OnbStep(text: step)
            OnbTitle(text: title)
            OnbBody(runs: runs)

            CodeField(
                cells: cells,
                onChange: { runtime.ui(.codeTyped(digits: $0)) },
                onSubmit: { runtime.ui(.codeSubmitted) }
            )
            .padding(.top, 26)

            if let fault {
                OnbFault(message: fault)
            }

            // Absent until the code is complete, because there is nothing to
            // submit before that. The machine decides which; this draws it.
            if let action {
                Button(action) { runtime.ui(.codeSubmitted) }
                    .buttonStyle(.primaryBlock)
                    .padding(.top, 12)
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
