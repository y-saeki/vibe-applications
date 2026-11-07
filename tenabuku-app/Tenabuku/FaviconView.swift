import HatenaUnofficial
import ImageIO
import SwiftUI
import UIKit

/// The small site icon at the start of an entry's site line. Decorative: the
/// host name next to it already says which site it is.
struct FaviconView: View {
    let pageURL: URL

    @Environment(FaviconSettingsStore.self) private var settings
    @State private var phase: Phase

    init(pageURL: URL) {
        self.pageURL = pageURL
        // An icon the list fetched ahead of time shows on the first frame.
        _phase = State(initialValue: Self.phase(cached: FaviconCache.shared.cached(pageURL)) ?? .loading)
    }

    private static func phase(cached: UIImage??) -> Phase? {
        cached.map { $0.map(Phase.loaded) ?? .missing }
    }

    private enum Phase {
        case loading
        case loaded(UIImage)
        case missing
    }

    private struct TaskID: Equatable {
        let pageURL: URL
        let sources: [FaviconSource]
    }

    private static let size: CGFloat = 13
    private static let shape = RoundedRectangle(cornerRadius: 3, style: .continuous)

    var body: some View {
        Group {
            switch phase {
            case .loading:
                Color.clear
            case .loaded(let image):
                Image(uiImage: image)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .clipShape(Self.shape)
            case .missing:
                // Neither Hatena nor DuckDuckGo had an icon.
                Image(systemName: "globe")
                    .font(.system(size: 12, weight: .regular))
                    .foregroundStyle(.secondary)
            }
        }
        .frame(width: Self.size, height: Self.size)
        .accessibilityHidden(true)
        .task(id: TaskID(pageURL: pageURL, sources: settings.enabledSources)) {
            if let cached = Self.phase(cached: FaviconCache.shared.cached(pageURL)) {
                phase = cached
                return
            }
            phase = .loading
            let image = await FaviconCache.shared.image(for: pageURL, from: settings.enabledSources)
            phase = image.map(Phase.loaded) ?? .missing
        }
    }
}

/// Keeps the icons fetched this session, so that scrolling back up or
/// reopening a list does not ask again, and asks only once for a URL that
/// several rows are waiting on, or that `ImagePrefetch` already asked for.
@MainActor
final class FaviconCache {
    static let shared = FaviconCache()

    private final class Entry {
        let image: UIImage?
        init(_ image: UIImage?) { self.image = image }
    }

    private let done = NSCache<NSURL, Entry>()
    private var pending: [URL: Task<UIImage?, Never>] = [:]
    /// Bumped by `removeAll`, so that a fetch started under the old sources
    /// does not put its result back afterwards.
    private var generation = 0

    /// Forgets every icon, for when the sources change.
    func removeAll() {
        done.removeAllObjects()
        pending.removeAll()
        generation += 1
    }

    /// What is already known about `page` without waiting: `.some(nil)` for
    /// a page with no icon, nil when it has not been fetched.
    func cached(_ page: URL) -> UIImage?? {
        done.object(forKey: page as NSURL).map(\.image)
    }

    func image(for page: URL, from sources: [FaviconSource]) async -> UIImage? {
        if let entry = done.object(forKey: page as NSURL) {
            return entry.image
        }
        return await load(page, from: sources).value
    }

    /// Starts fetching the icons not already fetched or on their way.
    func prefetch(_ pages: some Sequence<URL>, from sources: [FaviconSource]) {
        for page in pages where done.object(forKey: page as NSURL) == nil {
            _ = load(page, from: sources)
        }
    }

    private func load(_ page: URL, from sources: [FaviconSource]) -> Task<UIImage?, Never> {
        if let task = pending[page] {
            return task
        }
        let started = generation
        let task = Task<UIImage?, Never> {
            let image = await Task.detached(priority: .utility) {
                await AppServices.favicons.icon(for: page, from: sources).flatMap(Self.decode)
            }.value
            if self.generation == started {
                self.pending[page] = nil
                self.done.setObject(Entry(image), forKey: page as NSURL)
            }
            return image
        }
        pending[page] = task
        return task
    }

    /// Decodes PNG, JPEG or ICO. An ICO holds several sizes; the largest is
    /// the one that stays sharp at 13 points on a 3x screen.
    nonisolated private static func decode(_ data: Data) -> UIImage? {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
        let count = CGImageSourceGetCount(source)
        guard count > 0 else { return nil }
        let largest = (0..<count).max { a, b in
            width(source, a) < width(source, b)
        } ?? 0
        guard let image = CGImageSourceCreateImageAtIndex(source, largest, nil) else { return nil }
        return UIImage(cgImage: image)
    }

    nonisolated private static func width(_ source: CGImageSource, _ index: Int) -> Int {
        let properties = CGImageSourceCopyPropertiesAtIndex(source, index, nil) as? [CFString: Any]
        return properties?[kCGImagePropertyPixelWidth] as? Int ?? 0
    }
}
