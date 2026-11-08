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

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    if let host = item.link.host() {
                        HStack(spacing: 4) {
                            FaviconView(pageURL: item.link)
                            Text(host).lineLimit(1)
                        }
                    }
                    Spacer(minLength: 0)
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
            Spacer(minLength: 0)
            EntryThumbnail(url: item.imageURL)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityValue(isRead ? Text("既読") : Text(""))
    }
}

/// The square image at the end of an entry row. Every row keeps the slot, so
/// titles line up whether or not the page has an `og:image`: when there is
/// none, or it fails to load, the slot shows a globe on a faint gray, the same
/// glyph `FaviconView` falls back to.
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
        Group {
            switch phase {
            case .loading:
                Self.background
            case .loaded(let image):
                Image(uiImage: image).resizable().scaledToFill()
            case .failed:
                fallback
            }
        }
        .frame(width: ThumbnailCache.side, height: ThumbnailCache.side)
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

    private static var background: some View {
        Color.secondary.opacity(0.1)
    }

    private var fallback: some View {
        Self.background.overlay {
            Image(systemName: "globe")
                .font(.system(size: 24, weight: .light))
                .foregroundStyle(.quaternary)
        }
    }
}
