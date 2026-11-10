import HatenaFeed
import SwiftUI

/// Tells its content whether the system may lay the bars out vertically here,
/// by reading `toolbarVerticalEdge`: a trait of the context rather than the
/// device model, as Apple's iPhone Duo guidance asks. The value only exists
/// from iOS 27.1; earlier systems count as never placing a vertical bar.
struct VerticalBarReader<Content: View>: View {
    @ViewBuilder let content: (Bool) -> Content

    var body: some View {
        if #available(iOS 27.1, *) {
            Reading(content: content)
        } else {
            content(false)
        }
    }

    @available(iOS 27.1, *)
    private struct Reading: View {
        @Environment(\.toolbarVerticalEdge) private var verticalEdge
        let content: (Bool) -> Content

        var body: some View {
            content(verticalEdge != nil)
        }
    }
}

struct EntryRow: View {
    let item: FeedItem
    /// Dims the title, the way the official app tells opened entries apart.
    let isRead: Bool
    /// Keeps room for the longest title, so that rows side by side on a
    /// shelf line up whatever their titles' length.
    var reservesTitleSpace = false
    /// Asks Hatena for the thumbnail, for entries that came without one.
    var looksUpImage = false
    /// Marks a bookmark the user keeps private.
    var isPrivate = false

    private static let lineSpacing: CGFloat = 6

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            SquareAsTallAs {
                if looksUpImage {
                    LookedUpThumbnail(page: item.link)
                } else {
                    EntryThumbnail(url: item.imageURL)
                }
                // The site line and three title lines, in the fonts below,
                // so the image reaches the bottom of the third line at any
                // Dynamic Type size.
                VStack(alignment: .leading, spacing: Self.lineSpacing) {
                    Text(verbatim: " ").font(.caption).bold()
                    Text(verbatim: " \n \n ").font(.body)
                }
                .hidden()
            }
            VStack(alignment: .leading, spacing: Self.lineSpacing) {
                HStack(spacing: 8) {
                    if let host = item.link.host() {
                        HStack(spacing: 4) {
                            FaviconView(pageURL: item.link)
                            Text(host).lineLimit(1)
                        }
                    }
                    Spacer(minLength: 0)
                    if isPrivate {
                        Image(systemName: "lock.fill")
                            .accessibilityLabel("非公開")
                    }
                    Text("\(item.bookmarkCount) users")
                        .foregroundStyle(.red)
                        .bold()
                        .fixedSize()
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                Text(item.title)
                    .font(.body)
                    .foregroundStyle(isRead ? .secondary : .primary)
                    .lineLimit(3, reservesSpace: reservesTitleSpace)
                    // Drawn below the site line but read out first.
                    .accessibilitySortPriority(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityValue(isRead ? Text("既読") : Text(""))
    }
}

/// Draws its first subview as a square as tall as its second, which it
/// measures but leaves to the caller to hide.
private struct SquareAsTallAs: Layout {
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard subviews.count == 2 else { return .zero }
        let side = subviews[1].sizeThatFits(.unspecified).height
        return CGSize(width: side, height: side)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        guard subviews.count == 2 else { return }
        let side = min(bounds.width, bounds.height)
        subviews[0].place(at: bounds.origin, proposal: ProposedViewSize(width: side, height: side))
        subviews[1].place(at: bounds.origin, proposal: .unspecified)
    }
}

/// The square image at the start of an entry row, as large as the row offers.
/// Every row keeps the slot, so titles line up whether or not the page has an
/// `og:image`: when there is none, or it fails to load, the slot shows a globe
/// on a faint gray, the same glyph `FaviconView` falls back to.
struct EntryThumbnail: View {
    let url: URL?

    @State private var phase: Phase

    private enum Phase {
        case loading
        case loaded(UIImage)
        case failed
    }

    private static let shape = RoundedRectangle(cornerRadius: 6)

    init(url: URL?) {
        self.url = url
        // A thumbnail the list fetched ahead of time shows on the first frame.
        _phase = State(initialValue: Self.phase(for: url) ?? .loading)
    }

    private static func phase(for url: URL?) -> Phase? {
        guard let url else { return .failed }
        return ThumbnailCache.shared.cached(url).map { $0.map(Phase.loaded) ?? .failed }
    }

    var body: some View {
        // The color takes the square the row offers, and the image fills it
        // without growing it.
        Self.background
            .overlay {
                switch phase {
                case .loading:
                    EmptyView()
                case .loaded(let image):
                    Image(uiImage: image).resizable().scaledToFill()
                case .failed:
                    fallback
                }
            }
            .clipShape(Self.shape)
            .accessibilityHidden(true)
            .task(id: url) {
                if let known = Self.phase(for: url) {
                    phase = known
                    return
                }
                guard let url else { return }
                phase = .loading
                phase = await ThumbnailCache.shared.image(for: url).map(Phase.loaded) ?? .failed
            }
    }

    /// The empty slot, as a thumbnail shows it while its image loads.
    static var placeholder: some View {
        background
            .clipShape(shape)
            .accessibilityHidden(true)
    }

    private static var background: some View {
        Color.secondary.opacity(0.1)
    }

    private var fallback: some View {
        Image(systemName: "globe")
            .font(.system(size: 24, weight: .light))
            .foregroundStyle(.quaternary)
    }
}

/// The thumbnail of a page whose image has to be looked up first. The slot
/// stays empty while it is, as it does while an image loads.
private struct LookedUpThumbnail: View {
    let page: URL

    /// Nil while looking up; `.some(nil)` when the page has no image.
    @State private var image: URL??

    init(page: URL) {
        self.page = page
        // A page the list looked up ahead of time shows on the first frame.
        _image = State(initialValue: EntryImageLookup.shared.cached(page))
    }

    var body: some View {
        Group {
            if let image {
                EntryThumbnail(url: image)
            } else {
                EntryThumbnail.placeholder
            }
        }
        .task(id: page) {
            if let known = EntryImageLookup.shared.cached(page) {
                image = known
                return
            }
            image = nil
            image = .some(await EntryImageLookup.shared.imageURL(for: page))
        }
    }
}
