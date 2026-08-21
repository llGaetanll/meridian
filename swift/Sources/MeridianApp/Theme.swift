/// The palette, as Swift values.
///
/// A transcription of `crates/web-main/css/theme.css`, which is where the
/// colours are decided for every platform. The names are the CSS custom
/// properties with the dashes taken out, so a token in the stylesheet can be
/// found here by reading it aloud.
///
/// Category tokens are the exception worth knowing about: the core sends a
/// token like `"c-food"` rather than a colour, precisely so that each platform
/// resolves it in its own palette. `Theme.category(_:)` is that lookup, and it
/// is the only place a token becomes a colour — the same table
/// `crates/phone-main/css/app.css` holds for the web.

import SwiftUI

enum Theme {
    static let bg = Color(hex: 0x01030A)
    static let bgPanel = Color(hex: 0x050A0F)
    static let bgRaised = Color(hex: 0x0B1118)
    static let bgInput = Color(hex: 0x111921)

    static let border = Color(hex: 0x131A22)
    static let divider = Color(hex: 0x161E26)
    static let borderStrong = Color(hex: 0x1D2731)

    static let text = Color(hex: 0xF1F5F9)
    static let textMuted = Color(hex: 0x94A3B8)
    static let textFaint = Color(hex: 0x64748B)

    static let accent = Color(hex: 0x8B5CF6)
    static let accentSoft = Color(hex: 0xA78BFA)
    static let accentBright = Color(hex: 0xC4B5FD)
    static let accentTint = Color(hex: 0x8B5CF6).opacity(0.14)

    static let up = Color(hex: 0xA3E635)
    static let down = Color(hex: 0xFB7185)
    static let downTint = Color(hex: 0xFB7185).opacity(0.14)

    static let radiusSm: CGFloat = 8
    static let radiusMd: CGFloat = 10
    static let radiusLg: CGFloat = 16

    /// The colour a category token stands for.
    ///
    /// An unknown token falls back to muted text rather than to a placeholder
    /// colour — a category we have not styled should read as unremarkable, not
    /// as an error.
    static func category(_ token: String) -> Color {
        switch token {
        case "c-food", "c-fees": return Color(hex: 0xFCD34D)
        case "c-transport", "c-home": return Color(hex: 0x38BDF8)
        case "c-travel", "c-entertain": return Color(hex: 0xA78BFA)
        case "c-merch", "c-services": return Color(hex: 0x5EEAD4)
        case "c-loans", "c-medical": return Color(hex: 0xFB7185)
        case "c-personal", "c-income": return Color(hex: 0xA3E635)
        case "c-transfer": return Color(hex: 0xCBD5E1)
        case "c-utilities", "c-govt": return Color(hex: 0x94A3B8)
        default: return textMuted
        }
    }

    /// The same colour behind text, for a chip or a bar. The stylesheet's
    /// `--cat-tint`, which is the colour at roughly an eighth.
    static func categoryTint(_ token: String) -> Color {
        category(token).opacity(0.13)
    }
}

extension Color {
    /// `0x8B5CF6` reads as the hex in the stylesheet does; the components do
    /// not.
    init(hex: UInt32) {
        self.init(
            .sRGB,
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255,
            opacity: 1
        )
    }
}

extension Theme {
    /// Temporary: the dock's icons as SF Symbols, until the app draws the same
    /// icons the web does.
    static func icon(_ name: String) -> String {
        switch name {
        case "home": return "house"
        case "ledger": return "list.bullet"
        case "recurring": return "arrow.trianglehead.2.clockwise"
        case "balances": return "building.columns"
        default: return "circle"
        }
    }
}
