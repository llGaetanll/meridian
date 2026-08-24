/// A category, as the pill the sketch draws.
///
/// Two shapes, one look: on a transaction row it is a label, and in the filter
/// grid it is a button carrying its count. Colour alone carries the selected
/// state — there is no tick — which is why an unselected chip is the neutral
/// inset background rather than a paler version of the category's colour.

import SwiftUI

/// The label on a row.
struct CategoryTag: View {
    let label: String
    let token: String

    var body: some View {
        Text(label)
            .font(.system(size: 11.5, weight: .semibold))
            .padding(.horizontal, 9)
            .padding(.vertical, 3)
            .background(Theme.categoryTint(token), in: Capsule())
            .foregroundStyle(Theme.category(token))
    }
}

/// The button in the filter grid.
struct CategoryChip: View {
    let chip: ViewCategoryChip

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        Button {
            runtime.ui(.categoryToggled(id: chip.id))
        } label: {
            HStack(spacing: 7) {
                Text(chip.count)
                    .font(.system(size: 10, weight: .bold))
                    .monospacedDigit()
                    .foregroundStyle(chip.selected ? Theme.category(chip.token) : Theme.textFaint)
                    .padding(.horizontal, 5)
                    .frame(minWidth: 19, minHeight: 19)
                    .background(
                        chip.selected
                            ? Theme.category(chip.token).opacity(0.22)
                            : Color.white.opacity(0.07),
                        in: Capsule()
                    )

                Text(chip.label)
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(chip.selected ? Theme.category(chip.token) : Theme.textMuted)
            }
            .padding(.leading, 4)
            .padding(.trailing, 12)
            .padding(.vertical, 4)
            .background(
                chip.selected ? Theme.categoryTint(chip.token) : Theme.bgInput,
                in: Capsule()
            )
            // A category with nothing in it stays visible but reads as
            // unavailable. It is still tappable: the core knows the count is
            // zero and says so, and a chip that refused the tap would be a
            // second rule to keep in step.
            .opacity(chip.empty ? 0.4 : 1)
        }
        .buttonStyle(.plain)
    }
}
