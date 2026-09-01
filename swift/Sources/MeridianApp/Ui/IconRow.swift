/// An icon, a line, a line under it, and something on the right.
///
/// Every list in first run is this one row: what connecting a bank means, what
/// the first sync has fetched, what is still true after a failure. The tone is
/// the core's answer and the only thing that colours it — a row does not work
/// out whether it is the one happening now.

import SwiftUI

struct IconRow: View {
    let row: ViewIconRow

    var body: some View {
        HStack(alignment: row.sub == nil ? .center : .top, spacing: 13) {
            Group {
                if row.tone == .running {
                    Spinner()
                } else {
                    Icon(name: row.icon, size: 18)
                }
            }
            .foregroundStyle(colour)
            .padding(.top, row.sub == nil ? 0 : 1)

            VStack(alignment: .leading, spacing: 3) {
                Text(row.title)
                    .font(.system(size: 14.5, weight: row.tone == .waiting ? .medium : .semibold))
                    .foregroundStyle(row.tone == .waiting ? Theme.textFaint : Theme.text)

                if let sub = row.sub {
                    Text(sub)
                        .font(.system(size: 12.5))
                        .foregroundStyle(Theme.textFaint)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }

            Spacer(minLength: 8)

            if let value = row.value {
                Text(value)
                    .font(.system(size: 12.5))
                    .foregroundStyle(Theme.textFaint)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(.vertical, 12)
    }

    private var colour: Color {
        switch row.tone {
        case .plain: Theme.textFaint
        case .done: Theme.up
        case .running: Theme.accentBright
        case .waiting: Theme.bars
        }
    }
}

/// The glyph that turns. A three-quarter arc, so the turning is the whole of
/// what says "working" — which is why it is drawn rather than borrowed from
/// `ProgressView`: one spinner in the app, and it is the one on the sheet.
///
/// Used by the sync rows and by the button that opens Plaid, so it takes a
/// size rather than assuming the row's.
struct Spinner: View {
    var size: CGFloat = 18

    @State private var turned = false

    var body: some View {
        Icon(name: "spin", size: size, weight: 2.4)
            .rotationEffect(.degrees(turned ? 360 : 0))
            .onAppear {
                withAnimation(.linear(duration: 0.9).repeatForever(autoreverses: false)) {
                    turned = true
                }
            }
    }
}
