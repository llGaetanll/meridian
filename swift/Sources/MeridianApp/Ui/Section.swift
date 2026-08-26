/// A part of a page: a strong rule, air, and a heading over what follows.
///
/// The rule runs the full width of the screen, so this is drawn around rows
/// that carry their own inset rather than inside a padded column.

import SwiftUI

struct Sec<Content: View>: View {
    /// Absent where the section is only a boundary — the calendar has no
    /// heading, being the thing above it drawn a second way.
    var title: String?

    /// The figure at the right of the heading.
    var value: String?

    /// Money takes a colour in a section heading and nowhere else on the page.
    var valueColour: Color = Theme.text

    /// The first section on a page sits against what is above it.
    var first = false

    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            SectionRule()

            VStack(alignment: .leading, spacing: 0) {
                if let title {
                    HStack(alignment: .firstTextBaseline, spacing: 12) {
                        Text(title)
                            .font(.system(size: 11, weight: .semibold))
                            .tracking(0.88)
                            .textCase(.uppercase)

                        Spacer(minLength: 0)

                        if let value {
                            Text(value)
                                .font(.system(size: 13, weight: .bold, design: .monospaced))
                                .foregroundStyle(valueColour)
                        }
                    }
                    .foregroundStyle(Theme.textFaint)
                    .padding(.horizontal, pageInset)
                    .padding(.bottom, 14)
                }

                content
            }
            .padding(.top, 26)
        }
        .padding(.top, first ? 0 : 24)
    }
}

/// The figure a page opens on, and what it is a figure for.
struct Hero: View {
    let label: String
    let value: String

    /// The two directions under it, where the page has both. The words carry
    /// the sign, so the figures do not.
    var flowIn: String?
    var flowOut: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(label)
                .font(.system(size: 11, weight: .semibold))
                .tracking(0.88)
                .textCase(.uppercase)
                .foregroundStyle(Theme.textFaint)

            Text(value)
                .font(.system(size: 34, weight: .bold, design: .monospaced))
                .tracking(-1.19)
                .padding(.top, 4)

            if let flowIn, let flowOut {
                HStack(spacing: 16) {
                    FlowFigure(amount: flowIn, word: "in", colour: Theme.up)
                    FlowFigure(amount: flowOut, word: "out", colour: Theme.down)
                }
                .padding(.top, 8)
            }
        }
        .padding(.horizontal, pageInset)
        .padding(.top, 12)
        .padding(.bottom, 22)
    }
}
