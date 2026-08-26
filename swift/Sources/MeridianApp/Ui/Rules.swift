/// The two rules, and both of them run the full width of the screen.
///
/// Strong between the parts of a page; faint between rows and between groups
/// of rows. A rule that stops short of the edge reads as a third kind of line,
/// which is why the rows carry their own inset and these do not.

import SwiftUI

/// Between the parts of a page.
struct SectionRule: View {
    var body: some View {
        Rectangle()
            .fill(Theme.borderStrong)
            .frame(height: 1)
    }
}

/// Between rows, and between groups of rows.
struct RowRule: View {
    var body: some View {
        Rectangle()
            .fill(Theme.divider)
            .frame(height: 1)
    }
}
