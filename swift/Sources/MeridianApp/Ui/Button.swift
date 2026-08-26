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
}

extension ButtonStyle where Self == QuietButton {
    /// No frame and no edge, so it presses down a point of its own. For an
    /// action that must be available without being the loudest thing in its
    /// row.
    static var quiet: QuietButton { QuietButton() }
}

struct RaisedButton: ButtonStyle {
    var primary = false

    var small = false

    func makeBody(configuration: Configuration) -> some View {
        let shape = RoundedRectangle(cornerRadius: Theme.radiusSm)

        configuration.label
            .font(.system(size: small ? 11.5 : 13, weight: .semibold))
            .foregroundStyle(primary ? Color.white : Theme.text)
            .padding(.horizontal, small ? 11 : 14)
            .padding(.vertical, small ? 6 : 8)
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
