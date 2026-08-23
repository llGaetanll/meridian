/// The logo slot.
///
/// Reserved whether or not there is a logo, so every row's text starts on the
/// same line — the sketch is explicit that there is no stand-in glyph. A logo
/// that fails to load leaves the slot empty for the same reason: Plaid's logo
/// host is reachable from a phone but not from every network.

import SwiftUI

struct Logo: View {
    let url: String?

    var size: CGFloat = 40

    var body: some View {
        RoundedRectangle(cornerRadius: Theme.radiusMd)
            .fill(url == nil ? Color.clear : Theme.bgInput)
            .frame(width: size, height: size)
            .overlay {
                AsyncImage(url: url.flatMap(URL.init(string:))) { image in
                    image.resizable().scaledToFill()
                } placeholder: {
                    Color.clear
                }
                .frame(width: size, height: size)
                .clipShape(RoundedRectangle(cornerRadius: Theme.radiusMd))
            }
    }
}
