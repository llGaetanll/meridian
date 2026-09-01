/// Things you type into: the field, and the code.
///
/// The frame again, at the size of a thing you type into — the outer hairline,
/// three points of ground, and the well inset in them. Typing in it warms the
/// frame by exactly what selecting warms a chip's, so a live field and a chosen
/// chip are the same event happening on screen.
///
/// `fault` is what draws one wrong, and it only ever arrives from the core: a
/// value the server rejected, or an address that is not one. Never an empty
/// field — nothing has gone wrong yet, and a form that turns red as you arrive
/// at it is scolding you for showing up.

import SwiftUI
import UIKit

struct Field: View {
    let field: ViewField

    var keyboard: UIKeyboardType = .emailAddress

    var contentType: UITextContentType? = .emailAddress

    var submit: SubmitLabel = .continue

    /// The whole value, every time it changes. The core holds what the field
    /// holds, so a paste and a backspace need no handler of their own.
    let onChange: (String) -> Void

    var onSubmit: () -> Void = {}

    @FocusState private var focused: Bool

    var body: some View {
        HStack(spacing: 10) {
            Icon(name: field.icon, size: 17)
                .foregroundStyle(Theme.textFaint)

            TextField(
                "",
                text: Binding(get: { field.value }, set: onChange),
                prompt: Text(field.placeholder).foregroundStyle(Theme.textFaint)
            )
            .font(.system(size: 15))
            .foregroundStyle(field.fault == nil ? Theme.text : Theme.down)
            .tint(Theme.accentBright)
            .keyboardType(keyboard)
            .textContentType(contentType)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .submitLabel(submit)
            .focused($focused)
            .onSubmit(onSubmit)
        }
        .padding(.horizontal, 12)
        .frame(height: 46)
        .frame(maxWidth: .infinity)
        .background(Theme.bgInput, in: RoundedRectangle(cornerRadius: 7))
        .padding(3)
        .background(Theme.bgPanel, in: RoundedRectangle(cornerRadius: Theme.radiusMd))
        .overlay(
            RoundedRectangle(cornerRadius: Theme.radiusMd)
                .stroke(border, lineWidth: 1)
        )
        // The keyboard is up before the screen has settled: this is the only
        // thing on it, so making someone tap the field first would be asking
        // for a tap that has exactly one possible target.
        .onAppear { focused = true }
    }

    private var border: Color {
        if field.fault != nil {
            return Theme.down.opacity(0.40)
        }

        return focused ? Theme.accent.opacity(0.34) : Theme.border
    }
}

/// A code: as many boxes as there are digits, one glyph each.
///
/// Set in the money face, because a code is read a digit at a time and
/// monospace keeps each one in its own slot. A box per digit says how many are
/// left without a counter saying so.
///
/// One real field behind all six rather than six fields with focus moving
/// between them. Six would mean deciding where a backspace goes, where a paste
/// lands, and what happens when iOS fills the whole code from the notification
/// at once — and that last one is how most people will use this screen.
struct CodeField: View {
    let cells: [ViewCodeCell]

    let onChange: (String) -> Void

    /// The keyboard's own return key, so a complete code can be sent without
    /// reaching for the button. The button is still there — this is the
    /// shortcut, not the way.
    let onSubmit: () -> Void

    @FocusState private var focused: Bool

    /// What the boxes are showing, which is what the field behind them holds.
    /// Read off the cells rather than kept here, so there is one copy of it and
    /// it is the core's.
    private var digits: String {
        cells.map(\.digit).joined()
    }

    var body: some View {
        ZStack {
            // Invisible rather than hidden: a field of zero size cannot take
            // the focus, and the caret and selection are drawn by the boxes in
            // front of it instead.
            TextField("", text: Binding(get: { digits }, set: onChange))
                .font(.system(size: 22, weight: .semibold, design: .monospaced))
                .foregroundStyle(.clear)
                .tint(.clear)
                .keyboardType(.numberPad)
                .textContentType(.oneTimeCode)
                .focused($focused)
                .onSubmit(onSubmit)
                .frame(maxWidth: .infinity)

            HStack(spacing: 8) {
                ForEach(Array(cells.enumerated()), id: \.offset) { _, cell in
                    CodeCell(cell: cell)
                }
            }
            .allowsHitTesting(false)
        }
        .onAppear { focused = true }
    }
}

private struct CodeCell: View {
    let cell: ViewCodeCell

    var body: some View {
        Text(cell.digit)
            .font(.system(size: 22, weight: .semibold, design: .monospaced))
            .foregroundStyle(cell.bad ? Theme.down : Theme.text)
            .frame(maxWidth: .infinity)
            .frame(height: 54)
            .overlay { if cell.live { Caret() } }
            .background(Theme.bgInput, in: RoundedRectangle(cornerRadius: 7))
            .padding(3)
            .background(Theme.bgPanel, in: RoundedRectangle(cornerRadius: Theme.radiusMd))
            .overlay(
                RoundedRectangle(cornerRadius: Theme.radiusMd)
                    .stroke(border, lineWidth: 1)
            )
    }

    private var border: Color {
        if cell.bad {
            return Theme.down.opacity(0.40)
        }

        return cell.live ? Theme.accent.opacity(0.34) : Theme.border
    }
}

/// The insertion point, drawn rather than the system's.
///
/// The real caret is tinted clear, because it belongs to a field that spans all
/// six boxes and would sit wherever the text happens to end rather than in the
/// box that takes the next digit.
private struct Caret: View {
    var body: some View {
        RoundedRectangle(cornerRadius: 1)
            .fill(Theme.accentBright)
            .frame(width: 2, height: 20)
    }
}
