import HatenaUnofficial
import ImageIO
import SwiftUI
import UIKit

/// The small site icon at the start of an entry's site line. Decorative: the
/// host name next to it already says which site it is.
struct FaviconView: View {
    let pageURL: URL

    @State private var phase: Phase = .loading

    private enum Phase {
        case loading
        case loaded(UIImage)
        case missing
    }

    private static let size: CGFloat = 13
    private static let shape = RoundedRectangle(cornerRadius: 3, style: .continuous)

    var body: some View {
        Group {
            switch phase {
            case .loading:
                Self.shape.fill(.quaternary)
            case .loaded(let image):
                Image(uiImage: image)
                    .resizable()
                    .interpolation(.high)
                    .scaledToFit()
                    .clipShape(Self.shape)
            case .missing:
                // Neither Hatena nor DuckDuckGo had an icon.
                Image(systemName: "globe")
                    .font(.system(size: 9, weight: .semibold))
                    .foregroundStyle(.secondary)
                    .frame(width: Self.size, height: Self.size)
                    .background(.quaternary, in: Self.shape)
            }
        }
        .frame(width: Self.size, height: Self.size)
        .accessibilityHidden(true)
        .task(id: pageURL) {
            phase = .loading
            let image = await FaviconCache.shared.image(for: pageURL)
            phase = image.map(Phase.loaded) ?? .missing
        }
    }
}

/// Keeps the icons fetched this session, so that scrolling back up or
/// reopening a list does not ask again, and asks only once for a URL that
/// several rows are waiting on.
@MainActor
final class FaviconCache {
    static let shared = FaviconCache()

    private final class Entry {
        let image: UIImage?
        init(_ image: UIImage?) { self.image = image }
    }

    private let done = NSCache<NSURL, Entry>()
    private var pending: [URL: Task<UIImage?, Never>] = [:]

    func image(for page: URL) async -> UIImage? {
        if let entry = done.object(forKey: page as NSURL) {
            return entry.image
        }
        if let task = pending[page] {
            return await task.value
        }
        let task = Task.detached(priority: .utility) {
            await AppServices.favicons.icon(for: page).flatMap(Self.decode)
        }
        pending[page] = task
        let image = await task.value
        pending[page] = nil
        done.setObject(Entry(image), forKey: page as NSURL)
        return image
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
