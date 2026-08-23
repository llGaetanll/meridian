/// Just enough SVG to draw the icon set.
///
/// SwiftUI has no SVG, and the icons are defined as SVG in
/// `crates/phone-main/src/ui/icon.rs`. Retracing them by hand in `Path` calls
/// would have meant two drawings of every glyph that only look alike until
/// someone edits one, so this evaluates the path data itself and the two apps
/// stay literally the same shapes.
///
/// What it covers is what those icons use: `M L H V C S Q A Z`, upper and
/// lower case, plus the `line`, `circle`, `polyline` and `polygon` elements as
/// their own helpers. `T` is absent because nothing uses it, and malformed
/// data stops the path rather than failing — an icon that comes out short is
/// visible, and there is no input here that a user can get wrong.

import CoreGraphics
import Foundation
import SwiftUI

enum Svg {
    /// Appends an SVG `d` string, in its own coordinate space.
    static func append(_ d: String, to path: inout Path) {
        var cursor = Cursor(tokens: tokenize(d))

        // Where the pen is, where the current subpath began, and the previous
        // curve's second control point — which `S` and `Q` reflect.
        var point = CGPoint.zero
        var subpathStart = CGPoint.zero
        var lastCubicControl: CGPoint?
        var lastQuadraticControl: CGPoint?

        var command: Character = "M"

        while !cursor.done {
            if let next = cursor.command() {
                command = next
            }

            switch command {
            case "M", "m":
                guard let target = cursor.point(relativeTo: point, command == "m") else { return }

                path.move(to: target)

                point = target
                subpathStart = target

                // A second pair after a moveto is a lineto, per the spec.
                command = command == "M" ? "L" : "l"

            case "L", "l":
                guard let target = cursor.point(relativeTo: point, command == "l") else { return }

                path.addLine(to: target)
                point = target

            case "H", "h":
                guard let x = cursor.number() else { return }

                point = CGPoint(x: command == "h" ? point.x + x : x, y: point.y)
                path.addLine(to: point)

            case "V", "v":
                guard let y = cursor.number() else { return }

                point = CGPoint(x: point.x, y: command == "v" ? point.y + y : y)
                path.addLine(to: point)

            case "C", "c":
                let relative = command == "c"

                guard
                    let control1 = cursor.point(relativeTo: point, relative),
                    let control2 = cursor.point(relativeTo: point, relative),
                    let target = cursor.point(relativeTo: point, relative)
                else { return }

                path.addCurve(to: target, control1: control1, control2: control2)

                point = target
                lastCubicControl = control2
                lastQuadraticControl = nil

            case "S", "s":
                let relative = command == "s"

                guard
                    let control2 = cursor.point(relativeTo: point, relative),
                    let target = cursor.point(relativeTo: point, relative)
                else { return }

                // The first control point is the reflection of the last one,
                // or the current point when the previous segment was not a
                // curve.
                let control1 = reflect(lastCubicControl, about: point)

                path.addCurve(to: target, control1: control1, control2: control2)

                point = target
                lastCubicControl = control2
                lastQuadraticControl = nil

            case "Q", "q":
                let relative = command == "q"

                guard
                    let control = cursor.point(relativeTo: point, relative),
                    let target = cursor.point(relativeTo: point, relative)
                else { return }

                path.addQuadCurve(to: target, control: control)

                point = target
                lastQuadraticControl = control
                lastCubicControl = nil

            case "A", "a":
                let relative = command == "a"

                guard
                    let rx = cursor.number(),
                    let ry = cursor.number(),
                    let rotation = cursor.number(),
                    let largeArc = cursor.number(),
                    let sweep = cursor.number(),
                    let target = cursor.point(relativeTo: point, relative)
                else { return }

                arc(
                    from: point,
                    to: target,
                    rx: rx,
                    ry: ry,
                    rotation: rotation,
                    largeArc: largeArc != 0,
                    sweep: sweep != 0,
                    to: &path
                )

                point = target
                lastCubicControl = nil
                lastQuadraticControl = nil

            case "Z", "z":
                path.closeSubpath()
                point = subpathStart

            default:
                return
            }

            _ = lastQuadraticControl
        }
    }

    static func line(_ x1: Double, _ y1: Double, _ x2: Double, _ y2: Double, to path: inout Path) {
        path.move(to: CGPoint(x: x1, y: y1))
        path.addLine(to: CGPoint(x: x2, y: y2))
    }

    static func circle(_ cx: Double, _ cy: Double, _ r: Double, to path: inout Path) {
        path.addEllipse(in: CGRect(x: cx - r, y: cy - r, width: r * 2, height: r * 2))
    }

    /// `points="6 9 12 15 18 9"`, as the flat list the attribute is.
    static func polyline(_ points: [Double], to path: inout Path) {
        for (index, point) in pairs(points).enumerated() {
            if index == 0 {
                path.move(to: point)
            } else {
                path.addLine(to: point)
            }
        }
    }

    static func polygon(_ points: [Double], to path: inout Path) {
        polyline(points, to: &path)
        path.closeSubpath()
    }

    private static func pairs(_ values: [Double]) -> [CGPoint] {
        stride(from: 0, to: values.count - 1, by: 2)
            .map { CGPoint(x: values[$0], y: values[$0 + 1]) }
    }

    private static func reflect(_ control: CGPoint?, about point: CGPoint) -> CGPoint {
        guard let control else { return point }

        return CGPoint(x: 2 * point.x - control.x, y: 2 * point.y - control.y)
    }

    /// An elliptical arc, as cubics.
    ///
    /// SVG states an arc by where it ends; a curve needs where it is centred.
    /// This is the conversion from the SVG specification (F.6.5), then the arc
    /// split into quarter turns and each one approximated by a cubic — the
    /// standard construction, and accurate to well under a pixel at icon size.
    private static func arc(
        from start: CGPoint,
        to end: CGPoint,
        rx: Double,
        ry: Double,
        rotation: Double,
        largeArc: Bool,
        sweep: Bool,
        to path: inout Path
    ) {
        var rx = abs(rx)
        var ry = abs(ry)

        // A radius of zero is a straight line, and so is going nowhere.
        if rx == 0 || ry == 0 || (start.x == end.x && start.y == end.y) {
            path.addLine(to: end)
            return
        }

        let phi = rotation * .pi / 180
        let cosPhi = cos(phi)
        let sinPhi = sin(phi)

        let dx = (Double(start.x) - Double(end.x)) / 2
        let dy = (Double(start.y) - Double(end.y)) / 2

        let x1 = cosPhi * dx + sinPhi * dy
        let y1 = -sinPhi * dx + cosPhi * dy

        // Radii too small to reach the endpoint are scaled up until they do,
        // which the specification asks for rather than treating as an error.
        let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry)

        if lambda > 1 {
            rx *= lambda.squareRoot()
            ry *= lambda.squareRoot()
        }

        let numerator = max(
            0,
            rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1
        )
        let denominator = rx * rx * y1 * y1 + ry * ry * x1 * x1

        let coefficient = (largeArc == sweep ? -1.0 : 1.0)
            * (denominator == 0 ? 0 : (numerator / denominator).squareRoot())

        let cx1 = coefficient * rx * y1 / ry
        let cy1 = -coefficient * ry * x1 / rx

        let cx = cosPhi * cx1 - sinPhi * cy1 + (Double(start.x) + Double(end.x)) / 2
        let cy = sinPhi * cx1 + cosPhi * cy1 + (Double(start.y) + Double(end.y)) / 2

        let theta1 = atan2((y1 - cy1) / ry, (x1 - cx1) / rx)
        let theta2 = atan2((-y1 - cy1) / ry, (-x1 - cx1) / rx)

        var delta = theta2 - theta1

        if !sweep && delta > 0 {
            delta -= 2 * .pi
        } else if sweep && delta < 0 {
            delta += 2 * .pi
        }

        let segments = max(1, Int(ceil(abs(delta) / (.pi / 2))))
        let step = delta / Double(segments)

        // The tangent scale that makes a cubic match a circular arc of `step`.
        let alpha = sin(step) * ((4 + 3 * pow(tan(step / 2), 2)).squareRoot() - 1) / 3

        func onArc(_ theta: Double) -> CGPoint {
            CGPoint(
                x: cx + rx * cos(theta) * cosPhi - ry * sin(theta) * sinPhi,
                y: cy + rx * cos(theta) * sinPhi + ry * sin(theta) * cosPhi
            )
        }

        func tangent(_ theta: Double) -> CGPoint {
            CGPoint(
                x: -rx * sin(theta) * cosPhi - ry * cos(theta) * sinPhi,
                y: -rx * sin(theta) * sinPhi + ry * cos(theta) * cosPhi
            )
        }

        for segment in 0..<segments {
            let from = theta1 + Double(segment) * step
            let to = from + step

            let start = onArc(from)
            let startTangent = tangent(from)
            let finish = onArc(to)
            let finishTangent = tangent(to)

            path.addCurve(
                to: finish,
                control1: CGPoint(
                    x: start.x + alpha * startTangent.x,
                    y: start.y + alpha * startTangent.y
                ),
                control2: CGPoint(
                    x: finish.x - alpha * finishTangent.x,
                    y: finish.y - alpha * finishTangent.y
                )
            )
        }
    }

    private enum Token {
        case command(Character)
        case number(Double)
    }

    /// Reads tokens in order, and knows how a pair of them becomes a point.
    private struct Cursor {
        let tokens: [Token]

        var index = 0

        var done: Bool { index >= tokens.count }

        /// The next token if it is a command, leaving it alone if it is not —
        /// which is how a repeated parameter set continues the last command.
        mutating func command() -> Character? {
            guard index < tokens.count, case .command(let character) = tokens[index] else {
                return nil
            }

            index += 1

            return character
        }

        mutating func number() -> Double? {
            guard index < tokens.count, case .number(let value) = tokens[index] else {
                return nil
            }

            index += 1

            return value
        }

        mutating func point(relativeTo origin: CGPoint, _ relative: Bool) -> CGPoint? {
            guard let x = number(), let y = number() else { return nil }

            return relative
                ? CGPoint(x: origin.x + x, y: origin.y + y)
                : CGPoint(x: x, y: y)
        }
    }

    /// Splits path data into commands and numbers.
    ///
    /// SVG allows a sign or a second decimal point to start the next number
    /// with no separator at all — `15-6.7` is two numbers — which is why this
    /// is a scanner rather than a `split(separator:)`.
    private static func tokenize(_ d: String) -> [Token] {
        var tokens: [Token] = []
        var current = ""

        func flush() {
            if let value = Double(current) {
                tokens.append(.number(value))
            }

            current = ""
        }

        for character in d {
            switch character {
            case "0"..."9":
                current.append(character)

            case ".":
                if current.contains(".") {
                    flush()
                }

                current.append(character)

            case "-", "+":
                // A sign continues the number only as an exponent's sign.
                if current.hasSuffix("e") || current.hasSuffix("E") {
                    current.append(character)
                } else {
                    flush()
                    current.append(character)
                }

            case "e", "E":
                current.append(character)

            case " ", ",", "\n", "\t", "\r":
                flush()

            default:
                flush()
                tokens.append(.command(character))
            }
        }

        flush()

        return tokens
    }
}
