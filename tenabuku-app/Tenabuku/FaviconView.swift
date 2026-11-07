import HatenaUnofficial
import SwiftUI

/// The small site icon at the start of an entry's details line. Decorative:
/// the host name next to it already says which site it is.
struct FaviconView: View {
    let pageURL: URL

    var body: some View {
        AsyncImage(url: Favicon.url(for: pageURL)) { phase in
            if let image = phase.image {
                image.resizable().scaledToFit()
            } else {
                Color.secondary.opacity(0.1)
            }
        }
        .frame(width: 13, height: 13)
        .clipShape(RoundedRectangle(cornerRadius: 2.5))
        .accessibilityHidden(true)
    }
}
