/// Where it goes: a table you read down, with each row's share standing on the
/// rule under it.
///
/// Square ends, resting on the line rather than straddling it, and starting at
/// the measure the words are set to rather than at the edge of the panel — so
/// a share is a share of the line you are reading rather than of the screen.
///
/// `share` is the exception `docs/FFI.md` carves out: geometry crosses as a
/// ratio rather than as a formatted string, because each platform lays it out
/// with its own API.

import SwiftUI

struct Split: View {
    let slices: [ViewBreakdownSlice]

    /// The page's own inset, which the rules give back so they run the full
    /// width of the screen.
    private static let inset: CGFloat = 20

    var body: some View {
        VStack(spacing: 0) {
            ForEach(slices, id: \.self) { slice in
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    Text(slice.label)
                        .font(.system(size: 13.5))
                        .foregroundStyle(Theme.textMuted)
                        .lineLimit(1)
                        .truncationMode(.tail)

                    Spacer(minLength: 0)

                    Text(slice.amount)
                        .font(.system(size: 13.5, weight: .semibold, design: .monospaced))
                }
                .padding(.horizontal, Self.inset)
                .padding(.vertical, 12)

                Track(share: slice.share)
                    .padding(.horizontal, Self.inset)
            }
        }
    }
}
