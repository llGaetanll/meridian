/// The breakdown bar and its legend — two views of one split, so they are
/// drawn from one list.
///
/// `share` is the exception `docs/FFI.md` carves out: geometry crosses as a
/// ratio rather than as a formatted string, because each platform lays it out
/// with its own API.

import SwiftUI

struct Breakdown: View {
    let slices: [ViewBreakdownSlice]

    var legend = false

    /// Home draws the same thing smaller, as a preview.
    var compact = false

    var body: some View {
        GeometryReader { geometry in
            HStack(spacing: 2) {
                ForEach(slices, id: \.self) { slice in
                    Theme.category(slice.token)
                        .frame(width: width(of: slice, in: geometry.size.width))
                }
            }
        }
        .frame(height: compact ? 7 : 8)
        .clipShape(Capsule())

        if legend {
            let columns = [
                GridItem(.flexible(), spacing: compact ? 14 : 18),
                GridItem(.flexible(), spacing: compact ? 14 : 18),
            ]

            LazyVGrid(columns: columns, alignment: .leading, spacing: compact ? 6 : 7) {
                ForEach(slices, id: \.self) { slice in
                    HStack(spacing: 7) {
                        Circle()
                            .fill(Theme.category(slice.token))
                            .frame(width: 7, height: 7)

                        Text(slice.label)
                            .foregroundStyle(Theme.textMuted)
                            .lineLimit(1)
                            .truncationMode(.tail)

                        Spacer(minLength: 0)

                        Text(slice.amount)
                            .fontWeight(.semibold)
                            .monospacedDigit()
                    }
                }
            }
            .font(.system(size: compact ? 11 : 11.5))
        }
    }

    /// The gaps between segments come out of the space the shares divide, so
    /// the bar fills its width exactly rather than overflowing by a pixel per
    /// slice.
    private func width(of slice: ViewBreakdownSlice, in total: CGFloat) -> CGFloat {
        let gaps = CGFloat(max(0, slices.count - 1)) * 2

        return max(0, (total - gaps) * slice.share)
    }
}
