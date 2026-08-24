/// The phone's frame: a header, a scrolling body, and the dock floating over
/// it.
///
/// A transcription of `crates/phone-main/src/ui/shell.rs`, minus the status
/// bar it draws — that one is scenery so a browser reads as a phone, and this
/// is a phone.

import SwiftUI

struct PhoneShell<Panel: View, Content: View>: View {
    var title: String

    /// Drawn above the body — the filter panel, on the one screen that has
    /// one.
    @ViewBuilder var panel: Panel

    @ViewBuilder var content: Content

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(spacing: 0) {
            Header(title: title)

            // A banner rather than a screen: whatever was loaded before is
            // still worth looking at, so the failure sits above it instead of
            // replacing it.
            if let message = runtime.error {
                Banner(message: message)
            }

            ScrollView {
                VStack(alignment: .leading, spacing: 0) {
                    panel
                    content
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                // Clears the floating dock, so the last row is reachable.
                .padding(.bottom, 132)
            }
            .scrollIndicators(.hidden)
            .refreshable { runtime.ui(.refreshRequested) }
        }
        .background(Theme.bg)
        .overlay(alignment: .bottom) { Dock() }
    }
}

/// The shell with nothing between the header and the body, which is every
/// screen but the Ledger.
extension PhoneShell where Panel == EmptyView {
    init(title: String, @ViewBuilder content: () -> Content) {
        self.init(title: title, panel: { EmptyView() }, content: content)
    }
}

private struct Header: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.system(size: 30, weight: .bold))
            .tracking(-0.9)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 20)
            .padding(.top, 10)
            .padding(.bottom, 2)
    }
}

private struct Banner: View {
    let message: String

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        HStack(spacing: 10) {
            Text(message)
                .font(.system(size: 12.5))
                .foregroundStyle(Theme.textMuted)

            Spacer(minLength: 0)

            Button("Retry") { runtime.ui(.refreshRequested) }
                .font(.system(size: 12.5, weight: .semibold))
                .foregroundStyle(Theme.down)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(Theme.downTint, in: RoundedRectangle(cornerRadius: Theme.radiusSm))
        .overlay(
            RoundedRectangle(cornerRadius: Theme.radiusSm)
                .stroke(Theme.down.opacity(0.25), lineWidth: 1)
        )
        .padding(.horizontal, 20)
        .padding(.top, 8)
    }
}

/// The dock. Which tab is lit is the core's answer, not a comparison here.
private struct Dock: View {
    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        GlassGroup(spacing: 10) {
            HStack(spacing: 2) {
                // Keyed on the tab, not on the whole item — `selected`
                // changes on every tab, and keying on it would make each
                // button a new button whenever the selection moved.
                ForEach(runtime.dock?.items ?? [], id: \.screen) { item in
                    Button {
                        runtime.ui(.screenOpened(screen: item.screen))
                    } label: {
                        VStack(spacing: 3) {
                            Icon(name: item.icon, size: 20)
                            Text(item.label).font(.system(size: 9.5, weight: .medium))
                        }
                        .foregroundStyle(
                            item.selected ? Theme.accentBright : Color.white.opacity(0.62)
                        )
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 6)
                        .background {
                            if item.selected {
                                RoundedRectangle(cornerRadius: 22)
                                    .fill(Theme.accentSoft.opacity(0.16))
                            }
                        }
                        // The whole tab, not the ink in it. An `Icon` is a
                        // stroked path, and SwiftUI hit-tests a stroke on the
                        // stroke — so without this the target is a few 1.5pt
                        // lines, and only the selected tab, which has a filled
                        // background, is comfortably tappable.
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 6)
            .frame(height: 62)
            .glass(in: Capsule())

            // The sketch draws this button but never the screen behind it, so
            // it stays inert rather than opening something invented.
            Button {} label: {
                Icon(name: "search", size: 23, weight: 1.9)
                    .foregroundStyle(Color.white.opacity(0.82))
                    .frame(width: 62, height: 62)
                    .glass(in: Circle())
            }
            .buttonStyle(.plain)
            .disabled(true)
            .opacity(0.55)
            .accessibilityLabel("Search")
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 22)
        // A fade under the dock, so a row scrolling behind it dims out rather
        // than showing through the glass mid-sentence. The glass blurs what is
        // behind it, which on a dark list is not enough on its own.
        .background {
            LinearGradient(
                colors: [Theme.bg.opacity(0), Theme.bg, Theme.bg],
                startPoint: .top,
                endPoint: .bottom
            )
            .padding(.top, -28)
            // Down to the screen's edge, not to the safe area's: the home
            // indicator sits over the list too, and a row ending half way
            // through the fade reads as a rendering fault.
            .ignoresSafeArea(edges: .bottom)
            .allowsHitTesting(false)
        }
    }
}
