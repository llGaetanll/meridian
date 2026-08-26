/// A share, drawn on the rule under the thing it belongs to.
///
/// Square ends, no inset, standing on the line rather than straddling it. The
/// breakdown draws one per category and the first sync draws one for the whole
/// job — the same control, because a proportion on a line the page already had
/// is one idea and not two.
///
/// It fills whatever width it is given, so the inset belongs to the caller:
/// under a category it starts at the measure the words are set to, and on the
/// sync screen at the column's own edge.

import SwiftUI

struct Track: View {
    /// 0..1. Geometry rather than a formatted percentage, which is the
    /// exception `docs/FFI.md` carves out for a drawn figure.
    let share: Double

    var body: some View {
        ZStack(alignment: .bottomLeading) {
            RowRule()

            GeometryReader { geometry in
                Rectangle()
                    .fill(Theme.accentSoft)
                    // No negative spread: on something four points tall it
                    // would eat the whole glow.
                    .shadow(color: Theme.accentSoft.opacity(0.55), radius: 5)
                    .frame(width: max(3, geometry.size.width * share), height: 4)
                    .animation(.easeOut(duration: 0.35), value: share)
            }
            .frame(height: 4)
            .padding(.bottom, 1)
        }
        .frame(height: 5)
    }
}
