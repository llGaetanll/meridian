/// A row of chips that fills the measure.
///
/// Set like a justified paragraph, which is what it is. Where the order of a
/// row of chips carries no meaning, it is chosen so that every line but the
/// last is as full as it can be, and the chips on those lines share out what
/// little is left. The last line takes the remainder and sits left, because a
/// short line stretched across the screen is the ragged edge you can see from
/// across the room — the same reason the last line of a paragraph is not
/// justified either.
///
/// So the line that ends up with the most room to spare is the one put last,
/// and its slack costs nothing. Everything else is costed by the square of
/// what it wastes, which is what drives the chips onto the lines that need
/// them.
///
/// The sketch costs every way of cutting the chips into lines and takes the
/// cheapest outright, which it can afford with nine of them. The app draws all
/// seventeen categories, and every partition of seventeen is out of reach, so
/// this seeds each line count with a largest-first fill and then moves and
/// swaps chips between lines while that makes the row cheaper.

import SwiftUI

struct ChipRows: Layout {
    var spacing: CGFloat = 8

    /// The lines, kept between passes: a selection changing re-measures every
    /// chip, and re-packing a row that has not changed shape is work nobody
    /// asked for.
    struct Cache {
        var measure: CGFloat = -1
        var widths: [CGFloat] = []
        var lines: [[Int]] = []
    }

    func makeCache(subviews: Subviews) -> Cache {
        Cache()
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout Cache) -> CGSize {
        let measure = proposal.replacingUnspecifiedDimensions().width
        let lines = lines(of: subviews, in: measure, cache: &cache)

        var height: CGFloat = 0

        for line in lines {
            height += tallest(line, in: subviews) + spacing
        }

        return CGSize(width: measure, height: max(0, height - spacing))
    }

    func placeSubviews(
        in bounds: CGRect,
        proposal: ProposedViewSize,
        subviews: Subviews,
        cache: inout Cache
    ) {
        var y = bounds.minY

        let lines = lines(of: subviews, in: bounds.width, cache: &cache)

        for (index, line) in lines.enumerated() {
            let widths = line.map { cache.widths[$0] }
            let content = widths.reduce(0, +)

            // The chips keep their own widths; the space between them takes up
            // the remainder, so the line is flush at both edges. The last line
            // keeps its natural spacing and sits left, and so does a line with
            // one chip on it — a lone chip stretched across the screen is not a
            // row.
            let justified = index < lines.count - 1 && line.count > 1

            let gap = justified
                ? (bounds.width - content) / CGFloat(line.count - 1)
                : spacing

            var x = bounds.minX

            for index in line {
                let size = subviews[index].sizeThatFits(.unspecified)

                subviews[index].place(
                    at: CGPoint(x: x, y: y),
                    anchor: .topLeading,
                    proposal: ProposedViewSize(size)
                )

                x += size.width + gap
            }

            y += tallest(line, in: subviews) + spacing
        }
    }

    /// Which line has the most room to spare, and so goes last.
    private func loosest(_ lines: [[Int]], widths: [CGFloat]) -> Int? {
        lines.indices.min { first, second in
            fill(lines[first], widths: widths) < fill(lines[second], widths: widths)
        }
    }

    private func fill(_ line: [Int], widths: [CGFloat]) -> CGFloat {
        line.map { widths[$0] }.reduce(0, +) + gaps(line.count)
    }

    private func tallest(_ line: [Int], in subviews: Subviews) -> CGFloat {
        var height: CGFloat = 0

        for index in line {
            height = max(height, subviews[index].sizeThatFits(.unspecified).height)
        }

        return height
    }

    private func lines(of subviews: Subviews, in measure: CGFloat, cache: inout Cache) -> [[Int]] {
        let widths = subviews.map { $0.sizeThatFits(.unspecified).width }

        if cache.measure == measure, cache.widths == widths {
            return cache.lines
        }

        cache.measure = measure
        cache.widths = widths
        cache.lines = pack(widths, measure: measure)

        return cache.lines
    }

    /// The cheapest arrangement this can find, at whichever line count wins.
    ///
    /// Squaring the waste is what makes four lines wasting a little beat three
    /// wasting a lot on one of them, so the fewest lines that fit is a floor to
    /// start from rather than the answer.
    private func pack(_ widths: [CGFloat], measure: CGFloat) -> [[Int]] {
        guard !widths.isEmpty, measure > 0 else { return [] }

        let order = widths.indices.sorted { widths[$0] > widths[$1] }
        let content = widths.reduce(0, +) + spacing * CGFloat(widths.count - 1)
        let fewest = max(1, Int((content / measure).rounded(.up)))

        var best: [[Int]] = []
        var cheapest = CGFloat.infinity

        for count in fewest...(fewest + 2) {
            guard var lines = seed(order, widths: widths, count: count, measure: measure) else {
                continue
            }

            refine(&lines, widths: widths, measure: measure)

            let price = cost(lines, widths: widths, measure: measure)

            if price < cheapest {
                cheapest = price
                best = lines
            }
        }

        // Nothing fits: a chip wider than the screen. One per line, which at
        // least leaves each of them readable.
        if best.isEmpty {
            best = order.map { [$0] }
        }

        // Widest chip first within a line, and the fullest lines first: the
        // line with the most room to spare is the one that sits last and
        // ragged, so it is the one that has to be at the bottom.
        for index in best.indices {
            best[index].sort { widths[$0] > widths[$1] }
        }

        return best.sorted { first, second in
            fill(first, widths: widths) > fill(second, widths: widths)
        }
    }

    /// Largest first into the line it leaves least room on. Nil if the chips
    /// do not fit in this many lines at all.
    ///
    /// Tightest-fit and not most-room-first: spreading the wide chips evenly
    /// leaves every line with a hole too small for anything left, and the row
    /// then needs a line it should not have. Filling one line to the brim
    /// before starting the next is what makes seventeen categories sit on six
    /// lines rather than seven.
    private func seed(
        _ order: [Int],
        widths: [CGFloat],
        count: Int,
        measure: CGFloat
    ) -> [[Int]]? {
        var lines: [[Int]] = Array(repeating: [], count: count)
        var filled = [CGFloat](repeating: 0, count: count)

        for index in order {
            var target: Int?

            for line in lines.indices {
                let width = filled[line] + widths[index] + gaps(lines[line].count + 1)

                if width > measure { continue }

                if let best = target, filled[best] >= filled[line] { continue }

                target = line
            }

            guard let target else { return nil }

            lines[target].append(index)
            filled[target] += widths[index]
        }

        return lines.contains(where: \.isEmpty) ? nil : lines
    }

    /// Move and swap chips between lines for as long as that makes the row
    /// cheaper. It settles in a handful of passes at these sizes; the cap is
    /// there so a pathological measure cannot spin.
    private func refine(_ lines: inout [[Int]], widths: [CGFloat], measure: CGFloat) {
        var price = cost(lines, widths: widths, measure: measure)

        for _ in 0..<8 {
            var improved = false

            for from in lines.indices {
                for to in lines.indices where to != from {
                    for slot in lines[from].indices.reversed() {
                        var moved = lines
                        let chip = moved[from].remove(at: slot)
                        moved[to].append(chip)

                        // A line emptied is a line fewer, which is a different
                        // line count and not this pass's business.
                        if moved[from].isEmpty { continue }

                        let after = cost(moved, widths: widths, measure: measure)

                        if after < price {
                            lines = moved
                            price = after
                            improved = true
                        }
                    }

                    for here in lines[from].indices {
                        for there in lines[to].indices {
                            var swapped = lines
                            swapped[from][here] = lines[to][there]
                            swapped[to][there] = lines[from][here]

                            let after = cost(swapped, widths: widths, measure: measure)

                            if after < price {
                                lines = swapped
                                price = after
                                improved = true
                            }
                        }
                    }
                }
            }

            if !improved { return }
        }
    }

    /// What a row costs: the square of what each line wastes, and out of reach
    /// if any line overflows.
    ///
    /// The loosest line is left out of the sum. That line is the one set last,
    /// where the room left over is not stretched across the chips and so costs
    /// nothing — and leaving it out is what pushes the remainder onto it
    /// instead of spreading it over every line.
    private func cost(_ lines: [[Int]], widths: [CGFloat], measure: CGFloat) -> CGFloat {
        let last = loosest(lines, widths: widths)

        var total: CGFloat = 0

        for (index, line) in lines.enumerated() {
            let width = fill(line, widths: widths)

            if width > measure { return .infinity }

            if index == last { continue }

            total += (measure - width) * (measure - width)
        }

        return total
    }

    private func gaps(_ count: Int) -> CGFloat {
        spacing * CGFloat(max(0, count - 1))
    }
}
