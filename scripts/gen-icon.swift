// Draws the app icon: the Meridian mark, colours inverted against the app.
//
// In the app the mark is violet on near-black. On the home screen it is the
// other way round — a dark mark on violet — because an icon that is mostly
// black is a hole in a row of icons, and the mark is the thing to recognise.
//
// The gradient is slight on purpose: enough that the tile is not flat, not so
// much that it reads as a button. Run by `scripts/ios-run.sh`; the output is
// checked in, so a build without Swift on the path still has an icon.

import AppKit
import CoreGraphics
import Foundation

let size = 1024.0

/// The mark's paths, in the 200-unit box `sketches/brand/meridian-mark.svg`
/// draws them in. Kept in step with `Ui/Icon.swift`, which draws the same five.
let meridians = [
    "M 181.753,73.308 C 171.596,42.2,126.76,28.932,81.61,43.674 C 36.459,58.415,8.09,95.584,18.247,126.692",
    "M 118.172,15.942 C 108.574,13.867,92.658,49.819,82.622,96.243 C 72.586,142.667,72.23,181.983,81.828,184.058",
    "M 14,100 C 14,133.585,52.504,160.811,100,160.811 C 147.496,160.811,186,133.585,186,100",
    "M 100,14 C 66.415,14,39.189,33.252,39.189,57 C 39.189,80.748,66.415,100,100,100 C 133.585,100,160.811,80.748,160.811,57 C 160.811,33.252,133.585,14,100,14",
    "M 147.342,171.797 C 166.464,159.188,160.77,116.822,134.624,77.17 C 108.477,37.517,71.78,15.595,52.658,28.203",
]

/// Just the two commands these paths use, absolute, which is all the generator
/// that wrote them emits.
func parse(_ d: String) -> CGPath {
    let path = CGMutablePath()
    var numbers: [Double] = []
    var command: Character = " "

    func flush() {
        guard !numbers.isEmpty else { return }

        switch command {
        case "M":
            path.move(to: CGPoint(x: numbers[0], y: numbers[1]))
        case "C":
            for start in stride(from: 0, to: numbers.count, by: 6) where start + 5 < numbers.count {
                path.addCurve(
                    to: CGPoint(x: numbers[start + 4], y: numbers[start + 5]),
                    control1: CGPoint(x: numbers[start], y: numbers[start + 1]),
                    control2: CGPoint(x: numbers[start + 2], y: numbers[start + 3])
                )
            }
        default:
            break
        }

        numbers = []
    }

    var token = ""

    for character in d {
        if character == "M" || character == "C" {
            if !token.isEmpty { numbers.append(Double(token) ?? 0); token = "" }
            flush()
            command = character
        } else if character == "," || character == " " {
            if !token.isEmpty { numbers.append(Double(token) ?? 0); token = "" }
        } else {
            token.append(character)
        }
    }

    if !token.isEmpty { numbers.append(Double(token) ?? 0) }
    flush()

    return path
}

let space = CGColorSpaceCreateDeviceRGB()

guard let context = CGContext(
    data: nil,
    width: Int(size),
    height: Int(size),
    bitsPerComponent: 8,
    bytesPerRow: 0,
    space: space,
    bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
) else {
    fatalError("no context")
}

func rgb(_ hex: UInt32) -> CGColor {
    CGColor(
        srgbRed: Double((hex >> 16) & 0xFF) / 255,
        green: Double((hex >> 8) & 0xFF) / 255,
        blue: Double(hex & 0xFF) / 255,
        alpha: 1
    )
}

// Theme.accentSoft down to Theme.accent: the two violets the app already uses.
let gradient = CGGradient(
    colorsSpace: space,
    colors: [rgb(0xA78BFA), rgb(0x8B5CF6)] as CFArray,
    locations: [0, 1]
)!

context.drawLinearGradient(
    gradient,
    start: CGPoint(x: 0, y: size),
    end: CGPoint(x: 0, y: 0),
    options: []
)

// The mark sits at 62% of the tile, which is about where Apple's own icons put
// their glyph inside the rounded square iOS masks this into.
let inset = size * 0.19
let scale = (size - inset * 2) / 200

context.translateBy(x: inset, y: size - inset)
context.scaleBy(x: scale, y: -scale)

context.setStrokeColor(rgb(0x0A0616))
// Lighter than the 14 the app draws at. Dark-on-light reads heavier than
// light-on-dark at the same weight, and at icon size the meridians merge into
// a blob before they read as a globe.
context.setLineWidth(10)
context.setLineCap(.round)
context.setLineJoin(.round)

// Clipped to a slightly smaller circle so the strokes stop at the rim rather
// than crossing it, which is what the SVG's clipPath does.
context.saveGState()
context.addEllipse(in: CGRect(x: 7, y: 7, width: 186, height: 186))
context.clip()

for d in meridians {
    context.addPath(parse(d))
    context.strokePath()
}

context.restoreGState()

// Inset 14, the same rim `Ui/Icon.swift` draws, inside the clip at 7.
context.addEllipse(in: CGRect(x: 14, y: 14, width: 172, height: 172))
context.strokePath()

guard let image = context.makeImage() else { fatalError("no image") }

let out = URL(fileURLWithPath: CommandLine.arguments[1])
let rep = NSBitmapImageRep(cgImage: image)

guard let png = rep.representation(using: .png, properties: [:]) else {
    fatalError("no png")
}

try png.write(to: out)
print("wrote \(out.path)")
