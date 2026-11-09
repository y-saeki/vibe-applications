import Foundation
import HatenaUnofficial
import ImageIO
import UIKit

/// Keeps the entry thumbnails fetched this session, already shrunk to the
/// size a row draws them at, so that a row can show its image on its first
/// frame when the list fetched it ahead of time (see `ImagePrefetch`).
@MainActor
final class ThumbnailCache {
    static let shared = ThumbnailCache()

    /// The side in points the thumbnail is decoded for, and the screen scale.
    /// A row draws it as tall as its site line and three title lines, about
    /// 90pt at the default text size. Larger text draws it a little soft
    /// rather than making every cached image heavier.
    nonisolated private static let side: CGFloat = 96
    nonisolated private static let scale: CGFloat = 3

    private final class Entry {
        /// Nil when the server answered with something that is not an image.
        let image: UIImage?
        init(_ image: UIImage?) { self.image = image }
    }

    private enum Outcome: Sendable {
        case image(UIImage)
        /// The server answered, but not with an image: not worth asking again.
        case notAnImage
        /// Not reached: a later appearance of the row tries again.
        case unreachable
    }

    private let done: NSCache<NSURL, Entry> = {
        let cache = NSCache<NSURL, Entry>()
        // About 200 thumbnails at 192 by 192 pixels.
        cache.totalCostLimit = 32 * 1024 * 1024
        return cache
    }()
    private var pending: [URL: Task<UIImage?, Never>] = [:]

    /// What is already known about `url` without waiting: `.some(nil)` for a
    /// URL that is not an image, nil when it has not been fetched.
    func cached(_ url: URL) -> UIImage?? {
        done.object(forKey: url as NSURL).map(\.image)
    }

    func image(for url: URL) async -> UIImage? {
        if let entry = done.object(forKey: url as NSURL) {
            return entry.image
        }
        return await load(url).value
    }

    /// Starts fetching the URLs not already fetched or on their way.
    func prefetch(_ urls: some Sequence<URL>) {
        for url in urls where done.object(forKey: url as NSURL) == nil {
            _ = load(url)
        }
    }

    private func load(_ url: URL) -> Task<UIImage?, Never> {
        if let task = pending[url] {
            return task
        }
        let task = Task<UIImage?, Never> {
            let outcome = await Task.detached(priority: .utility) { await Self.fetch(url) }.value
            self.pending[url] = nil
            switch outcome {
            case let .image(image):
                let cost = image.cgImage.map { $0.bytesPerRow * $0.height } ?? 0
                self.done.setObject(Entry(image), forKey: url as NSURL, cost: cost)
                return image
            case .notAnImage:
                self.done.setObject(Entry(nil), forKey: url as NSURL)
                return nil
            case .unreachable:
                return nil
            }
        }
        pending[url] = task
        return task
    }

    nonisolated private static func fetch(_ url: URL) async -> Outcome {
        guard let (data, _) = try? await URLSession.shared.data(from: url) else {
            return .unreachable
        }
        return downsample(data).map(Outcome.image) ?? .notAnImage
    }

    /// Decodes only as many pixels as the slot needs: the short side fills
    /// it, as `scaledToFill` draws it, so a large `og:image` costs neither
    /// the memory nor the main thread time of its full size.
    nonisolated private static func downsample(_ data: Data) -> UIImage? {
        let options = [kCGImageSourceShouldCache: false] as CFDictionary
        guard let source = CGImageSourceCreateWithData(data as CFData, options),
              CGImageSourceGetCount(source) > 0,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? CGFloat,
              let height = properties[kCGImagePropertyPixelHeight] as? CGFloat,
              width > 0, height > 0
        else { return nil }
        let target = side * scale
        // A very wide or tall image would otherwise come out huge.
        let aspect = min(max(width, height) / min(width, height), 4)
        let thumbnailOptions = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: ceil(target * aspect),
        ] as CFDictionary
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, thumbnailOptions) else { return nil }
        return UIImage(cgImage: image)
    }
}

/// Fetches the favicons and thumbnails of the rows just below the one that
/// came into view, so that scrolling finds them already loaded instead of
/// showing them pop in one by one. Only a short stretch ahead, so that a long
/// list, such as the user's own bookmarks, does not fetch everything at once.
@MainActor
enum ImagePrefetch {
    static let lookahead = 15

    /// - Parameters:
    ///   - rows: every row of the list, in order.
    ///   - index: the row that came into view.
    static func rows<Row>(
        _ rows: [Row],
        after index: Int,
        page: (Row) -> URL,
        thumbnail: (Row) -> URL? = { _ in nil },
        faviconSources: [FaviconSource]
    ) {
        let start = index + 1
        guard start < rows.count else { return }
        let ahead = rows[start..<min(start + lookahead, rows.count)]
        FaviconCache.shared.prefetch(ahead.map(page), from: faviconSources)
        ThumbnailCache.shared.prefetch(ahead.compactMap(thumbnail))
    }
}
