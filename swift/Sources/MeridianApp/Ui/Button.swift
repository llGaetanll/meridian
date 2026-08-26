/// The button.
///
/// Everything that does something stands on a solid line of its own darker
/// shade and drops onto it when pressed. If it stands up it does something; if
/// it lies flat it says something — which is the whole difference between a
/// button and a chip.
///
/// The edge has to be darker than the button and lighter than the page, or it
/// is invisible against one of them. That is why the quiet button sits a step
/// up from the ground it stands on rather than flush with it.

import SwiftUI

extension ButtonStyle where Self == RaisedButton {
    static var raised: RaisedButton { RaisedButton() }

    /// Solid violet, and one action per screen: the thing you are here to do.
    static var primary: RaisedButton { RaisedButton(primary: true) }

    /// The phone's one action, at the size of a target you hit without
    /// looking. The edge, the corner and the two violets are untouched — this
    /// is the same button, not a phone button.
    static var primaryBlock: RaisedButton {
        RaisedButton(primary: true, size: .large, block: true)
    }

    /// The same, quiet. Never more than one violet on a screen, so a second
    /// action is this one.
    static var block: RaisedButton { RaisedButton(size: .large, block: true) }
}

extension ButtonStyle where Self == QuietButton {
    /// No frame and no edge, so it presses down a point of its own. For an
    /// action that must be available without being the loudest thing in its
    /// row.
    static var quiet: QuietButton { QuietButton() }
}

struct RaisedButton: ButtonStyle {
    /// The three sizes the stylesheet has: `.sm`, the default, and `.lg`.
    enum Size {
        case small
        case regular
        case large

        var text: CGFloat {
            switch self {
            case .small: 11.5
            case .regular: 13
            case .large: 15
            }
        }

        var padding: (CGFloat, CGFloat) {
            switch self {
            case .small: (11, 6)
            case .regular: (14, 8)
            case .large: (18, 13)
            }
        }

        /// A larger button takes the larger corner, so the radius stays in
        /// proportion to the thing it is rounding.
        var radius: CGFloat {
            self == .large ? Theme.radiusMd : Theme.radiusSm
        }
    }

    var primary = false

    var size: Size = .regular

    /// Takes the width of whatever it is in.
    var block = false

    func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: size.radius)

        configuration.label
            .font(.system(size: size.text, weight: .semibold))
            .foregroundStyle(primary ? Color.white : Theme.text)
            .frame(maxWidth: block ? .infinity : nil)
            .padding(.horizontal, size.padding.0)
            .padding(.vertical, size.padding.1)
            .background(face, in: shape)
            .overlay(shape.stroke(primary ? Theme.accentStrong : Theme.slate800, lineWidth: 1))
            .background {
                shape
                    .fill(primary ? Theme.accentEdge : Theme.bgRaised)
                    .offset(y: 2)
                    .opacity(configuration.isPressed ? 0 : 1)
            }
            .offset(y: configuration.isPressed ? 2 : 0)
    }

    /// Two steps of the same shade — enough to catch a highlight across the
    /// top, not enough to read as a gradient.
    private var face: LinearGradient {
        LinearGradient(
            colors: primary
                ? [Theme.accentSoft, Theme.accent]
                : [Theme.borderStrong, Theme.bgInput],
            startPoint: .top,
            endPoint: .bottom
        )
    }
}

struct QuietButton: ButtonStyle {
    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(Theme.accentBright)
            .padding(.horizontal, 4)
            .padding(.vertical, 6)
            .opacity(configuration.isPressed ? 0.75 : 1)
            .offset(y: configuration.isPressed ? 1 : 0)
    }
}
