/// The chip.
///
/// One shape everywhere — categories, filters, the summary band — and the
/// frame is the point: an outer hairline, three points of ground, and the fill
/// inset inside that. Selected is a thing sitting in a frame rather than a
/// thing whose border changed colour.
///
/// A transcription of `.chip` in `sketches/shared/components.css`. No glow: on
/// a row of nine of them it was nine little lamps.

import SwiftUI

struct Chip: View {
    let label: String

    /// How many transactions match. It sits at the left of the label and in
    /// the money face, so a row of chips has one figure style with the rest of
    /// the app.
    var count: String?

    var selected = false

    /// Smaller, for a band or a summary rather than a control you aim at.
    var small = false

    /// Nothing matches. The chip stays visible and reads as unavailable.
    var empty = false

    /// Absent on a chip that only labels something.
    var action: (() -> Void)?

    var body: some View {
        if let action {
            Button(action: action) { frame }
                .buttonStyle(.plain)
        } else {
            frame
        }
    }

    private var frame: some View {
        fill
            .padding(small ? 2 : 3)
            .background(Theme.bgPanel, in: RoundedRectangle(cornerRadius: radius))
            .overlay(
                RoundedRectangle(cornerRadius: radius)
                    .stroke(
                        // The border is warmed by the same amount as the fill,
                        // so the frame does not fight what is inside it.
                        selected ? Theme.accentSoft.opacity(0.34) : Theme.border,
                        lineWidth: 1
                    )
            )
            .opacity(empty ? 0.45 : 1)
    }

    private var fill: some View {
        HStack(spacing: 8) {
            if let count {
                Text(count)
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .foregroundStyle(selected ? Theme.accentBright.opacity(0.68) : Theme.textFaint)
            }

            label(self.label)
        }
        .padding(.horizontal, small ? 8 : 10)
        .padding(.vertical, small ? 3 : 5)
        .background(
            selected ? Theme.accentTint : Color.clear,
            in: RoundedRectangle(cornerRadius: small ? 6 : 7)
        )
    }

    /// The label, in a slot the width of its heavier weight.
    ///
    /// Selecting a chip sets its label semibold, which is a point or two wider
    /// than medium — enough to re-measure the row, re-pack the lines, and move
    /// every chip on the screen. A chip you tapped should not move, and nor
    /// should its neighbours, so the slot is the size the label takes at its
    /// widest and the weight changes inside it.
    private func label(_ text: String) -> some View {
        Text(text)
            .font(.system(size: size, weight: .semibold))
            .lineLimit(1)
            .hidden()
            .overlay(alignment: .leading) {
                Text(text)
                    .font(.system(size: size, weight: selected ? .semibold : .medium))
                    .foregroundStyle(selected ? Theme.accentBright : Theme.textMuted)
                    .lineLimit(1)
                    .fixedSize()
            }
    }

    private var size: CGFloat { small ? 11.5 : 12.5 }

    private var radius: CGFloat { small ? 8 : Theme.radiusMd }
}

/// A category in the filter, which is a chip that sends its id back.
struct CategoryChip: View {
    let chip: ViewCategoryChip

    var small = false

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        Chip(
            label: chip.label,
            count: small ? nil : chip.count,
            selected: chip.selected,
            small: small,
            empty: chip.empty
        ) {
            runtime.ui(.categoryToggled(id: chip.id))
        }
    }
}
