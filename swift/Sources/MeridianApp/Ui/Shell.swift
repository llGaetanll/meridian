/// The phone's frame: a header, a scrolling body, and the dock floating over
/// it.
///
/// Built once here rather than by each screen, so a change of tab moves a
/// title and a body and leaves the frame where it is.

import SwiftUI

struct PhoneShell<Band: View, Content: View>: View {
    var title: String

    /// Which screen this is, so a change of tab can be crossed rather than
    /// cut. The shell itself stays put — it is the title and the body that
    /// change, and only those are given a new identity.
    var screen: String

    /// Drawn between the header and the list, and only once the thing it
    /// summarises has scrolled past — see `bandAnchor()`. Handed the scroller,
    /// so tapping it can take you back to what it stands in for.
    @ViewBuilder var band: (ScrollViewProxy) -> Band

    @ViewBuilder var content: Content

    @EnvironmentObject private var runtime: RuntimeApp

    /// Whether the anchored view has gone by. A boolean and not an offset:
    /// this changes twice in a scroll rather than sixty times a second.
    @State private var banded = false

    var body: some View {
        VStack(spacing: 0) {
            // Over the band, not under it: the band slides in from above, and
            // without somewhere to slide out of sight it crosses the title on
            // its way down.
            Header(title: title)
                .background(Theme.bg)
                .zIndex(1)
                .animation(.easeInOut(duration: 0.2), value: screen)

            // A banner rather than a screen: whatever was loaded before is
            // still worth looking at, so the failure sits above it instead of
            // replacing it.
            if let message = runtime.error {
                Banner(message: message)
            }

            ScrollViewReader { scroller in
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        content
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    // Clears the floating dock, so the last row is reachable.
                    .padding(.bottom, 124)
                    // A new screen is a new body, which is what lets one cross
                    // into the next. Scoped to the body: the header, the dock
                    // and the scroller itself keep their identity, so a tab
                    // does not rebuild the app under the finger.
                    .id(screen)
                    .transition(.opacity)
                }
                .coordinateSpace(name: BandAnchor.space)
                .scrollIndicators(.hidden)
                .refreshable { runtime.ui(.refreshRequested) }
                .animation(.easeInOut(duration: 0.2), value: screen)
                // Over the list, not above it and not inset into it. Both of
                // those change how much room the scroller has the moment the
                // band arrives, which moves everything inside it — and on a
                // fast scroll the band leaving does it again in the other
                // direction, under a finger that is already moving. Floating,
                // the list never learns the band exists.
                .overlay(alignment: .top) {
                    if banded {
                        band(scroller)
                            .transition(.move(edge: .top))
                    }
                }
            }
        }
        .background(Theme.bg)
        .onPreferenceChange(BandAnchor.self) { bottom in
            // Two thresholds, not one, so a finger held still on the line
            // cannot flicker the band in and out. Both of them sit inside the
            // anchor rather than below it — see `leaves`.
            let wanted = banded ? bottom < BandAnchor.leaves : bottom < BandAnchor.arrives

            if wanted != banded {
                withAnimation(.easeOut(duration: 0.22)) { banded = wanted }
            }
        }
        .overlay(alignment: .bottom) { Dock() }
        // After the overlay, not before it. The sketch measures the dock 22
        // from the bottom of the phone, and the phone it is drawn on is 844
        // tall including the home indicator — so the dock has to be able to
        // reach the bottom of the screen. Applied to the shell alone this only
        // lets the shell draw into the safe area; the overlay still aligns to
        // the frame it was given, which is the one that stops short.
        .ignoresSafeArea(edges: .bottom)
    }
}

/// How far down the scroller the bottom of the band's anchor is.
///
/// A distance rather than a yes or no, because the two thresholds that decide
/// the band are the shell's business and not the anchor's.
struct BandAnchor: PreferenceKey {
    /// The scroller's own coordinate space, which the anchor measures itself
    /// against.
    static let space = "scroll"

    /// No anchor at all reads as "far below", which is what an unscrolled
    /// screen with no band looks like.
    static let defaultValue = CGFloat.infinity

    /// The band arrives as the last of the anchor goes: a headline still half
    /// on screen does not need standing in for, and the band over it says the
    /// same thing twice.
    static let arrives: CGFloat = 24

    /// And leaves later than it arrived, so the two cannot chase each other —
    /// but still short of the anchor's own height, which is where it rests. A
    /// threshold past that never clears: an empty page bounces far enough to
    /// bring the band in, and springs back to a resting anchor that is already
    /// below the line, leaving the band up over nothing.
    static let leaves: CGFloat = 48

    static func reduce(value: inout CGFloat, nextValue: () -> CGFloat) {
        value = min(value, nextValue())
    }
}

extension View {
    /// Marks the view whose scrolling past brings the band in.
    ///
    /// Whatever wears this has to be outside the lazy part of the list. A lazy
    /// stack throws away what has scrolled off, and the anchor going with it
    /// would report `false` — putting the band away at exactly the point it is
    /// there for.
    func bandAnchor() -> some View {
        overlay(alignment: .bottom) {
            GeometryReader { geometry in
                Color.clear.preference(
                    key: BandAnchor.self,
                    value: geometry.frame(in: .named(BandAnchor.space)).maxY
                )
            }
        }
    }
}

private struct Header: View {
    let title: String

    var body: some View {
        Text(title)
            .font(.system(size: 30, weight: .bold))
            .tracking(-0.9)
            .id(title)
            .transition(.opacity)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, pageInset)
            .padding(.top, 10)
            // The sketch sets 2 here and the page it is drawn on is 844 tall;
            // on a real phone the title and what follows it read as one block.
            .padding(.bottom, 16)
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
                .buttonStyle(.raised)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background(Theme.downTint, in: RoundedRectangle(cornerRadius: Theme.radiusSm))
        .overlay(
            RoundedRectangle(cornerRadius: Theme.radiusSm)
                .stroke(Theme.down.opacity(0.25), lineWidth: 1)
        )
        .padding(.horizontal, pageInset)
        .padding(.top, 8)
    }
}

/// The dock. Which tab is lit is the core's answer, not a comparison here.
private struct Dock: View {
    @EnvironmentObject private var runtime: RuntimeApp

    /// The lit tile is one tile that moves between tabs rather than three
    /// that appear and disappear. Each tab offers its centre under its own
    /// screen, and the tile takes whichever centre belongs to the lit one — so
    /// changing which one that is moves the tile there.
    @Namespace private var selection

    var body: some View {
        HStack(spacing: 10) {
            HStack(spacing: 2) {
                // Keyed on the tab, not on the whole item — `selected`
                // changes on every tab, and keying on it would make each
                // button a new button whenever the selection moved.
                ForEach(runtime.dock?.items ?? [], id: \.screen) { item in
                    Tab(item: item, selection: selection)
                }
            }
            .background {
                if let lit {
                    SelectionTile()
                        .matchedGeometryEffect(
                            id: lit,
                            in: selection,
                            properties: .position,
                            anchor: .center,
                            isSource: false
                        )
                }
            }
            .animation(.smooth(duration: 0.32), value: lit)
            .padding(.horizontal, 4)
            // The bar takes the row and the search button takes its square,
            // which is `flex: 1` in the stylesheet. Without this the bar is as
            // wide as the glyphs inside it happen to make it, and a screen
            // whose content is a little wider than another moves the dock.
            .frame(maxWidth: .infinity)
            .frame(height: 54)
            .background { ground }

            // The sketch draws this button but never the screen behind it, so
            // it stays inert rather than opening something invented. The glyph
            // is what dims, not the button: fading the whole thing made its
            // ground translucent and the list showed through it.
            Button {} label: {
                Icon(name: "search", size: 23, weight: 1.9)
                    .foregroundStyle(Theme.text)
                    .opacity(0.5)
                    .frame(width: 54, height: 54)
                    .background { ground }
            }
            .buttonStyle(.plain)
            .disabled(true)
            .accessibilityLabel("Search")
        }
        .padding(.horizontal, 16)
        .padding(.bottom, 22)
        .background {
            // The bar is opaque, but the list still runs through the gutter
            // either side of it, so the page fades out underneath rather than
            // showing a row half way through a sentence.
            LinearGradient(
                colors: [Theme.bg.opacity(0), Theme.bg, Theme.bg],
                startPoint: .top,
                endPoint: .bottom
            )
            .padding(.top, -38)
            // Down to the screen's edge, not to the safe area's: the home
            // indicator sits over the list too, and a row ending half way
            // through the fade reads as a rendering fault.
            .ignoresSafeArea(edges: .bottom)
            .allowsHitTesting(false)
        }
    }

    /// What the bar and the square beside it are made of: one step up from the
    /// page because they are above it, the chip's corner rather than a capsule,
    /// and a shadow that says so. No outline — at this size the shadow is
    /// enough, and a line round it only fought the tile inside.
    ///
    /// The shadow goes on the ground and not on the whole row, or the glyphs
    /// standing on it cast one too.
    private var ground: some View {
        RoundedRectangle(cornerRadius: Theme.radiusMd)
            .fill(Theme.bgRaised)
            .shadow(color: .black.opacity(0.5), radius: 14, y: 10)
    }

    /// Which tab is lit, so the tile has something to animate against.
    private var lit: Screen? {
        runtime.dock?.items.first(where: \.selected)?.screen
    }
}

private struct Tab: View {
    let item: ViewDockItem
    let selection: Namespace.ID

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        Button {
            runtime.ui(.screenOpened(screen: item.screen))
        } label: {
            Icon(name: item.icon, size: 23)
                .foregroundStyle(item.selected ? Theme.accentBright : Theme.textMuted)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                // Where the tile goes when this tab is the lit one.
                .matchedGeometryEffect(id: item.screen, in: selection, isSource: true)
                // The whole tab, not the ink in it. An `Icon` is a stroked
                // path, and SwiftUI hit-tests a stroke on the stroke — so
                // without this the target is a few 1.5pt lines, and only the
                // selected tab, which has a filled ground, is comfortably
                // tappable.
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        // The words under the glyphs are gone; the name is not.
        .accessibilityLabel(item.label)
    }
}

/// The lit tab's ground.
///
/// A square the height of the row: the words are gone, so what it marks is a
/// glyph, and a capsule cut for a label under one would sit round the icon
/// with nothing in the rest of it. 46pt is the bar's height less its padding.
private struct SelectionTile: View {
    var body: some View {
        RoundedRectangle(cornerRadius: 7)
            .fill(Theme.accentSoft.opacity(0.16))
            .frame(width: 46, height: 46)
    }
}
