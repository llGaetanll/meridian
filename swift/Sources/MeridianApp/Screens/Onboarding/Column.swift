/// The column every screen of first run is.
///
/// A word saying where you are, the body, and the action on the bottom edge
/// under the thumb. Nothing on these screens scrolls: a step that needs a
/// scrollbar is asking too much at once.
///
/// The controls in them are the design language's — the button, the field, the
/// code, the mark, the row, the share. What is here is only where they sit.

import SwiftUI

struct OnbColumn<Content: View>: View {
    /// The chevron, on the screens that have somewhere to go back to.
    var back: (() -> Void)?

    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // In the status bar's shadow: these screens have no title bar, and
            // one chevron does not earn one.
            if let back {
                Button(action: back) {
                    Icon(name: "back", size: 20, weight: 2.2)
                        .foregroundStyle(Theme.textMuted)
                        .frame(width: 44, height: 40, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .padding(.leading, 18)
                .padding(.top, 4)
            }

            VStack(alignment: .leading, spacing: 0) {
                content
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
            .padding(.horizontal, 24)
            .padding(.top, 6)
            .padding(.bottom, 30)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(Theme.bg)
    }
}

/// Eats the slack, so the actions sit on the bottom edge whatever is above.
struct OnbFill: View {
    /// How much it is allowed to eat. Unbounded is the usual case; a cap is
    /// how a screen that leads with a mark keeps it off the top edge without
    /// pinning it to a number.
    var upTo: CGFloat = .infinity

    var body: some View {
        Spacer(minLength: 10)
            .frame(maxHeight: upTo)
    }
}

/// Where you are, which is the only reason the flow says how long it is.
struct OnbStep: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 11, weight: .semibold))
            .tracking(0.99)
            .textCase(.uppercase)
            .foregroundStyle(Theme.textFaint)
            .padding(.bottom, 22)
    }
}

struct OnbTitle: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 28, weight: .bold))
            .tracking(-0.84)
            .fixedSize(horizontal: false, vertical: true)
    }
}

/// A glyph beside the title, on a screen that is the result of something
/// rather than a step towards it: the tick where it finished, the warning
/// where it did not.
///
/// On the title's line and not above it, because the glyph is an adjective on
/// the sentence rather than a thing of its own — stacked, it read as a mark at
/// the head of the screen and the eye stopped at it twice. The glyph carries
/// the colour, so the title does not have to.
struct OnbOutcome: View {
    let icon: String
    let tint: Color
    let title: String

    var body: some View {
        HStack(alignment: .center, spacing: 13) {
            Icon(name: icon, size: 30, weight: 1.9)
                .foregroundStyle(tint)

            OnbTitle(text: title)
        }
    }
}

/// The body, in runs.
///
/// `strong` is the sketch's `<b>` — the address on the code screen, Plaid's
/// name on the bank screen. Concatenated rather than laid out side by side, so
/// an emphasised phrase wraps with the sentence it is part of.
struct OnbBody: View {
    let runs: [ViewRun]

    var body: some View {
        runs
            .reduce(Text("")) { text, run in
                text
                    + Text(run.text)
                    .fontWeight(run.strong ? .semibold : .regular)
                    .foregroundStyle(run.strong ? Theme.text : Theme.textMuted)
            }
            .font(.system(size: 14.5))
            .fixedSize(horizontal: false, vertical: true)
            // The measure the sketch sets, which is about 32 characters.
            .frame(maxWidth: 280, alignment: .leading)
            .padding(.top, 10)
    }
}

/// What the app is waiting for, under the field it is waiting on.
struct OnbFault: View {
    let message: String

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Icon(name: "warn", size: 15, weight: 1.9)
                .padding(.top, 2)

            Text(message)
                .fixedSize(horizontal: false, vertical: true)
        }
        .font(.system(size: 13))
        .foregroundStyle(Theme.down)
        .padding(.top, 13)
    }
}

/// The actions, stacked. One violet at most, always last.
struct OnbActions<Content: View>: View {
    @ViewBuilder let content: Content

    var body: some View {
        VStack(spacing: 10) { content }
    }
}

/// Small print, at the point of consent rather than in a settings screen.
struct OnbLegal: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.system(size: 11.5))
            .lineSpacing(4)
            .multilineTextAlignment(.center)
            .foregroundStyle(Theme.textFaint)
            .frame(maxWidth: .infinity)
            .padding(.top, 16)
    }
}

/// What arrived, on the last screen of the flow.
///
/// Money-faced without being money: they are a column you read down, which is
/// the whole reason that face exists.
struct OnbTally: View {
    let rows: [ViewTally]

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            ForEach(rows, id: \.self) { row in
                HStack(alignment: .firstTextBaseline, spacing: 14) {
                    Text(row.value)
                        .font(.system(size: 22, weight: .semibold, design: .monospaced))
                        .frame(minWidth: 46, alignment: .trailing)

                    Text(row.label)
                        .font(.system(size: 13.5))
                        .foregroundStyle(Theme.textMuted)
                }
                .padding(.vertical, 12)
            }
        }
        .padding(.top, 20)
    }
}

/// A button's label with a glyph in front of it.
///
/// Named around SwiftUI's own `Label`, which pairs a title with an icon the
/// system chooses a layout for. This one is the sketch's: a fixed gap and the
/// glyph at the button's size.
struct IconLabel: View {
    let icon: String
    let text: String

    var body: some View {
        HStack(spacing: 8) {
            Icon(name: icon, size: 16, weight: 1.9)
            Text(text)
        }
    }
}

/// The rows a screen of first run lists, whatever they are about.
struct OnbRows: View {
    let rows: [ViewIconRow]

    var body: some View {
        VStack(spacing: 0) {
            ForEach(rows, id: \.self) { row in
                IconRow(row: row)
            }
        }
    }
}
