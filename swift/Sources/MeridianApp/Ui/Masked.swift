/// A figure that can be covered.
///
/// The core has already replaced the value with dots when masked, so the real
/// figure is not in the view at all while it is covered — stricter than the
/// sketch, where it sat underneath at zero opacity. What is left to draw is
/// the affordance: a solid bar, unmistakably "withheld" rather than "typed",
/// with the reveal label written onto the bar itself because the bar is
/// already a surface.

import SwiftUI

struct Masked: View {
    let value: String
    let masked: Bool

    /// Tapping reveals. The Home and hero figures are; a figure inside a row
    /// is not.
    var tappable = false

    /// The bar is heavier on a hero figure and carries a soft sheen.
    var hero = false

    var size: CGFloat = 15

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        Group {
            if masked {
                bar
            } else {
                Text(value)
                    .font(.system(size: size, weight: .bold, design: .monospaced))
            }
        }
        .contentShape(Rectangle())
        .onTapGesture {
            if tappable {
                runtime.ui(.maskToggled)
            }
        }
    }

    private var bar: some View {
        // The bar covers the figure's own body, so it is as tall as the text
        // and as wide as the label it has to hold.
        RoundedRectangle(cornerRadius: hero ? 8 : 5)
            .fill(fill)
            .frame(minWidth: tappable ? min(210, size * 7) : 92, maxWidth: .infinity)
            .frame(height: size * 1.1)
            .overlay {
                if tappable {
                    HStack(spacing: 8) {
                        Icon(name: "eye", size: 17)
                        Text("Tap to reveal")
                            .font(.system(size: 15, weight: .medium))
                    }
                    .foregroundStyle(Theme.textMuted)
                }
            }
            .fixedSize(horizontal: tappable, vertical: false)
    }

    private var fill: AnyShapeStyle {
        guard hero, !tappable else {
            return AnyShapeStyle(Theme.bgInput)
        }

        return AnyShapeStyle(
            LinearGradient(
                colors: [Theme.bgInput, Theme.borderStrong, Theme.bgInput],
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
        )
    }
}
