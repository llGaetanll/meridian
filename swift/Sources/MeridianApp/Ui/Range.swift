/// The time range: one outline, the four options inside it, the live one
/// filled.
///
/// The web's 1D/1W/1M selector stood on its end. Four separately outlined rows
/// were four controls sitting near each other; this is one control with four
/// settings, and every option is the same height because in an enum no case is
/// bigger than another.

import SwiftUI

struct RangeControl: View {
    let range: ViewRange

    @EnvironmentObject private var runtime: RuntimeApp

    var body: some View {
        VStack(spacing: 0) {
            Option(on: range.kind == .this) {
                runtime.ui(.rangeSelected(kind: .this(unit: .month)))
            } content: {
                Text("This")
                Token(range.thisUnit, on: range.kind == .this)
            }

            Option(on: range.kind == .last) {
                // The tokens keep their last value even when another form is
                // live, so switching back does not reset the sentence.
                let count = UInt32(range.lastCount) ?? 3

                runtime.ui(.rangeSelected(kind: .last(count: count, unit: .month)))
            } content: {
                Text("Last")
                Token(range.lastCount, on: range.kind == .last, fixed: true)
                Token(range.lastUnit, on: range.kind == .last)
            }

            // Drawn and dead, as the sketch has it: picking a day needs a date
            // picker, and that is a screen of its own.
            Option(on: range.kind == .between, dead: true) {} content: {
                Text("Between")
                Token(range.betweenStart, on: false)
                Text("and")
                Token(range.betweenEnd, on: false)
            }

            Option(on: range.kind == .all) {
                runtime.ui(.rangeSelected(kind: .all))
            } content: {
                Text("All time")
            }
        }
        .padding(3)
        .background(Theme.bgPanel, in: RoundedRectangle(cornerRadius: Theme.radiusMd))
        .overlay(
            RoundedRectangle(cornerRadius: Theme.radiusMd).stroke(Theme.border, lineWidth: 1)
        )
    }
}

private struct Option<Content: View>: View {
    let on: Bool

    /// Part of the sentence, and not yet built. Dimmed rather than hidden, so
    /// the shape of the thing is honest.
    var dead = false

    let select: () -> Void

    @ViewBuilder let content: Content

    var body: some View {
        HStack(spacing: 7) {
            content
            Spacer(minLength: 0)
        }
        .font(.system(size: 13, weight: on ? .medium : .regular))
        .foregroundStyle(on ? Theme.accentBright : Theme.textFaint)
        .padding(.horizontal, 10)
        .frame(height: 38)
        .background(
            on ? Theme.accentTint : Color.clear,
            in: RoundedRectangle(cornerRadius: 7)
        )
        .opacity(dead ? 0.45 : 1)
        .contentShape(Rectangle())
        .onTapGesture {
            if !dead {
                select()
            }
        }
    }
}

/// One editable word inside the sentence — "This [month]".
private struct Token: View {
    let text: String
    let on: Bool

    /// A number sits in a fixed slot so the sentence does not jump from 3 to
    /// 12.
    var fixed = false

    init(_ text: String, on: Bool, fixed: Bool = false) {
        self.text = text
        self.on = on
        self.fixed = fixed
    }

    var body: some View {
        HStack(spacing: 5) {
            Text(text)
            Icon(name: "caret", size: 10).opacity(0.75)
        }
        .font(.system(size: 12.5, weight: .semibold))
        .foregroundStyle(on ? Theme.accentBright : Theme.textMuted)
        .padding(.horizontal, 8)
        .padding(.vertical, 3)
        .frame(minWidth: fixed ? 30 : 0)
        .background(
            on ? Theme.accentSoft.opacity(0.22) : Color.white.opacity(0.06),
            in: RoundedRectangle(cornerRadius: 6)
        )
    }
}
