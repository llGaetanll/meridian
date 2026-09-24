/// The icon set, drawn from the same path data the web draws.
///
/// The geometry below is the phone's icon set, and this file is where it
/// lives: a 24-unit box, a 1.8 stroke, and the same `d` strings the sketches
/// under `sketches/mobile` draw. The core sends an icon *name*
/// (`ViewDockItem.icon`) and each platform resolves it, so the only way two
/// platforms can show the same glyph is to share the drawing rather than each
/// pick something that looks close — which makes these strings the thing
/// Kotlin copies. SF Symbols would have been less code and a different icon
/// set.
///
/// Which means a little SVG path evaluation, since SwiftUI has none. It
/// handles the subset these icons use; `Svg.append` says what that is.

import SwiftUI

/// One icon, sized in points. Stroke width scales with it, so a 14pt icon is
/// the 20pt one shrunk rather than a heavier line.
struct Icon: View {
    let name: String

    var size: CGFloat = 20

    /// The stylesheet's `.icon { stroke-width: 1.8 }`, in the 24-unit box.
    var weight: CGFloat = 1.8

    var body: some View {
        let glyph = Glyph(name: name)

        Group {
            if Glyph.filled.contains(name) {
                glyph.fill(.foreground)
            } else {
                glyph.stroke(
                    .foreground,
                    style: StrokeStyle(
                        lineWidth: weight * size / 24,
                        lineCap: .round,
                        lineJoin: .round
                    )
                )
            }
        }
        .frame(width: size, height: size)
    }
}

/// The paths themselves, in the 24-unit box the web draws them in.
struct Glyph: Shape {
    let name: String

    /// Drawn filled rather than stroked, as `fill="currentColor"` in the SVG.
    ///
    /// `warn` is not among them: its dot is the one filled part of an
    /// otherwise stroked glyph, and at 18px a 0.6-radius ring reads as a dot
    /// anyway.
    static let filled: Set<String> = ["caret"]

    func path(in rect: CGRect) -> Path {
        var path = Path()

        switch name {
        case "home":
            Svg.append("M3 10.2 12 3l9 7.2", to: &path)
            Svg.append("M5.5 9.2V20a1 1 0 0 0 1 1H10v-6h4v6h3.5a1 1 0 0 0 1-1V9.2", to: &path)

        case "ledger":
            for y in [6.0, 12.0, 18.0] {
                Svg.line(9, y, 21, y, to: &path)
                Svg.circle(4, y, 1, to: &path)
            }

        case "recurring":
            Svg.append("M3 11a9 9 0 0 1 15-6.7L21 7", to: &path)
            Svg.polyline([21, 3, 21, 7, 17, 7], to: &path)
            Svg.append("M21 13a9 9 0 0 1-15 6.7L3 17", to: &path)
            Svg.polyline([3, 21, 3, 17, 7, 17], to: &path)

        case "search":
            Svg.circle(11, 11, 7, to: &path)
            Svg.line(16.5, 16.5, 21, 21, to: &path)

        case "filter":
            Svg.line(4, 7, 20, 7, to: &path)
            Svg.line(7, 12, 17, 12, to: &path)
            Svg.line(10, 17, 14, 17, to: &path)

        case "chevron":
            Svg.polyline([6, 9, 12, 15, 18, 9], to: &path)

        case "caret":
            Svg.polygon([6, 9, 18, 9, 12, 16], to: &path)

        case "eye":
            Svg.append("M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z", to: &path)
            Svg.circle(12, 12, 3, to: &path)

        case "eye-off":
            Svg.append("M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7-10-7-10-7Z", to: &path)
            Svg.circle(12, 12, 3, to: &path)
            Svg.line(3, 21, 21, 3, to: &path)

        case "tag":
            Svg.append(
                "M20.6 13.4 12 4.8H4.8V12l8.6 8.6a1.7 1.7 0 0 0 2.4 0l4.8-4.8a1.7 1.7 0 0 0 0-2.4Z",
                to: &path
            )
            Svg.circle(8.4, 8.4, 1.2, to: &path)

        case "clock":
            Svg.circle(12, 12, 9, to: &path)
            Svg.polyline([12, 7, 12, 12, 15.5, 14], to: &path)

        // Open in new, and it is telling the truth: Plaid Link in update mode
        // is a hosted page, so the tap leaves the app.
        case "ext":
            Svg.append("M14 4h6v6", to: &path)
            Svg.append("M20 4 12.5 11.5", to: &path)
            Svg.append(
                "M18.5 14v5.5a1 1 0 0 1-1 1h-13a1 1 0 0 1-1-1v-13a1 1 0 0 1 1-1H10",
                to: &path
            )

        case "refresh":
            Svg.append("M3 12a9 9 0 0 1 15-6.7L21 8", to: &path)
            Svg.polyline([21, 3, 21, 8, 16, 8], to: &path)

        case "mail":
            Svg.rect(2.5, 5, 19, 14, 2.5, to: &path)
            Svg.append("m3 7 9 6 9-6", to: &path)

        // Back, and only back. The dock's `chevron` points down; this one is
        // the same stroke turned, so the two read as one family.
        case "back":
            Svg.polyline([15, 5, 8, 12, 15, 19], to: &path)

        case "tick-circle":
            Svg.circle(12, 12, 9, to: &path)
            Svg.polyline([16, 9.5, 10.8, 15, 8, 12.4], to: &path)

        case "warn":
            Svg.circle(12, 12, 9, to: &path)
            Svg.line(12, 7.5, 12, 13, to: &path)
            Svg.circle(12, 16.5, 0.6, to: &path)

        case "lock":
            Svg.rect(4.5, 10.5, 15, 10, 2.5, to: &path)
            Svg.append("M8 10.5V7.8a4 4 0 0 1 8 0v2.7", to: &path)

        case "bank":
            Svg.append("m3 9.5 9-5.5 9 5.5", to: &path)
            Svg.line(4, 20, 20, 20, to: &path)
            Svg.line(6.5, 12, 6.5, 17, to: &path)
            Svg.line(12, 12, 12, 17, to: &path)
            Svg.line(17.5, 12, 17.5, 17, to: &path)

        // The account tab's. A head and shoulders and nothing else: the app
        // never asked for a name, so anything more would be a face invented
        // for somebody who did not give us one.
        case "user":
            Svg.circle(12, 8.5, 3.8, to: &path)
            Svg.append("M4.5 20.5a7.5 7.5 0 0 1 15 0", to: &path)

        case "trash":
            Svg.append("M4.5 6.5h15", to: &path)
            Svg.append("M9 6.5V4.8h6v1.7", to: &path)
            Svg.append("M6.5 6.5 7.4 20h9.2l.9-13.5", to: &path)

        // One three-quarter arc, and the turning is what says "working".
        case "spin":
            Svg.append("M12 3.5a8.5 8.5 0 1 1-8.5 8.5", to: &path)

        // A step not reached yet: the tick's circle with nothing in it.
        case "dot":
            Svg.circle(12, 12, 8, to: &path)

        // An unknown name draws nothing rather than a "missing" glyph: a gap is
        // easier to spot in review than a box that looks deliberate.
        default:
            break
        }

        let scale = min(rect.width, rect.height) / 24

        return path.applying(CGAffineTransform(scaleX: scale, y: scale))
    }
}

/// The mark, from `sketches/brand/meridian-mark.svg`.
///
/// A globe of five strokes inside a circle, with the strokes clipped to a
/// slightly smaller circle so they stop at the rim rather than crossing it —
/// which is what the SVG's `clipPath` does.
struct BrandMark: View {
    var size: CGFloat = 30

    var body: some View {
        let weight = 14 * size / 200

        ZStack {
            Meridians()
                .stroke(
                    .foreground,
                    style: StrokeStyle(lineWidth: weight, lineCap: .round, lineJoin: .round)
                )
                .clipShape(Circle().inset(by: size * 7 / 200))

            Circle()
                .inset(by: size * 14 / 200)
                .stroke(.foreground, lineWidth: weight)
        }
        .frame(width: size, height: size)
    }

    private struct Meridians: Shape {
        func path(in rect: CGRect) -> Path {
            var path = Path()

            for d in [
                "M 181.753,73.308 C 171.596,42.2,126.76,28.932,81.61,43.674 C 36.459,58.415,8.09,95.584,18.247,126.692",
                "M 118.172,15.942 C 108.574,13.867,92.658,49.819,82.622,96.243 C 72.586,142.667,72.23,181.983,81.828,184.058",
                "M 14,100 C 14,133.585,52.504,160.811,100,160.811 C 147.496,160.811,186,133.585,186,100",
                "M 100,14 C 66.415,14,39.189,33.252,39.189,57 C 39.189,80.748,66.415,100,100,100 C 133.585,100,160.811,80.748,160.811,57 C 160.811,33.252,133.585,14,100,14",
                "M 147.342,171.797 C 166.464,159.188,160.77,116.822,134.624,77.17 C 108.477,37.517,71.78,15.595,52.658,28.203",
            ] {
                Svg.append(d, to: &path)
            }

            let scale = min(rect.width, rect.height) / 200

            return path.applying(CGAffineTransform(scaleX: scale, y: scale))
        }
    }
}
