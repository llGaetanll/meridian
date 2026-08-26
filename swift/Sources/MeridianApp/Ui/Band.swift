/// What a page's headline becomes once it has scrolled away.
///
/// One shape on all three screens: a rule under it, the page's own inset, and
/// one line of it. Nothing above it — what is above a band is the title, and a
/// title does not need a line drawn under it to be a title. What goes in differs — the Ledger's is what the filter
/// is set to, the other two are the figure the page opens on — but a band is a
/// band, so the height and the boundaries are here rather than in each of them.
///
/// It stands on the page's own ground. No panel and no glass: a rule under it
/// is what separates it, the same as every other boundary in the app.

import SwiftUI

struct Band<Content: View>: View {
    @ViewBuilder let content: Content

    /// One line, whatever is on it.
    static var height: CGFloat { 44 }

    var body: some View {
        VStack(spacing: 0) {
            content
                .frame(maxWidth: .infinity, alignment: .leading)
                .frame(height: Self.height)
                .padding(.horizontal, pageInset)

            SectionRule()
        }
        .background(Theme.bg)
    }
}

/// A figure and the word that says which way it goes.
///
/// The word carries the direction, so the figure does not carry a sign as
/// well. Money takes a colour here and on the section headings, nowhere else.
struct FlowFigure: View {
    let amount: String
    let word: String
    let colour: Color

    var size: CGFloat = 12.5

    var body: some View {
        HStack(spacing: 5) {
            Text(amount)
                .font(.system(size: size, weight: .semibold, design: .monospaced))
                .foregroundStyle(colour)

            Text(word)
                .font(.system(size: size))
                .foregroundStyle(Theme.textFaint)
        }
    }
}
