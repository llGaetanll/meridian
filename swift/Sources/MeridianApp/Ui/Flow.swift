/// A row of things that wraps, which is `flex-wrap: wrap` and which SwiftUI
/// has no stack for.
///
/// The category chips keep their natural width and wrap onto the next line —
/// a grid would give them equal columns and leave "Food & drink" and "Rent &
/// utilities" the same width as "Travel".

import SwiftUI

struct Flow: Layout {
    var spacing: CGFloat = 7

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.replacingUnspecifiedDimensions().width
        let rows = rows(of: subviews, in: width)

        let height = rows
            .map { row in row.map(\.size.height).max() ?? 0 }
            .reduce(0) { $0 + $1 + spacing }

        return CGSize(width: width, height: max(0, height - spacing))
    }

    func placeSubviews(
        in bounds: CGRect,
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout ()
    ) {
        var y = bounds.minY

        for row in rows(of: subviews, in: bounds.width) {
            var x = bounds.minX

            for item in row {
                item.subview.place(
                    at: CGPoint(x: x, y: y),
                    anchor: .topLeading,
                    proposal: ProposedViewSize(item.size)
                )

                x += item.size.width + spacing
            }

            y += (row.map(\.size.height).max() ?? 0) + spacing
        }
    }

    private struct Item {
        let subview: LayoutSubview
        let size: CGSize
    }

    private func rows(of subviews: Subviews, in width: CGFloat) -> [[Item]] {
        var rows: [[Item]] = [[]]
        var x: CGFloat = 0

        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)

            // The first item on a line always goes on it, however wide it is:
            // wrapping it to the next line would leave an empty one.
            if !rows[rows.count - 1].isEmpty, x + size.width > width {
                rows.append([])
                x = 0
            }

            rows[rows.count - 1].append(Item(subview: subview, size: size))
            x += size.width + spacing
        }

        return rows
    }
}
