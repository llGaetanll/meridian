/// Liquid glass, where the platform has it.
///
/// The dock is glass in the sketch and in the web app, which fakes it with a
/// backdrop blur and two inset highlights. iOS 26 has the real thing, so this
/// asks for it and keeps the fake as the floor — the app's deployment target
/// is iOS 17, and a dock that vanishes on an older phone would be worse than
/// one that is merely blurred.

import SwiftUI

extension View {
    @ViewBuilder
    func glass(in shape: some Shape) -> some View {
        if #available(iOS 26.0, *) {
            self.glassEffect(.regular, in: shape)
        } else {
            self
                .background(.ultraThinMaterial, in: shape)
                .overlay(shape.stroke(Color.white.opacity(0.14), lineWidth: 1))
        }
    }
}

/// Groups glass shapes so they blend into each other rather than each being
/// its own pane. The dock and the search button sit side by side, which is
/// exactly the case this exists for.
struct GlassGroup<Content: View>: View {
    var spacing: CGFloat = 10

    @ViewBuilder let content: Content

    var body: some View {
        if #available(iOS 26.0, *) {
            GlassEffectContainer(spacing: spacing) {
                HStack(spacing: spacing) { content }
            }
        } else {
            HStack(spacing: spacing) { content }
        }
    }
}
