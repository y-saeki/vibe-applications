import Foundation
import HatenaUnofficial
import PageImage

/// Finds the thumbnail of entries that came without one, such as the user's
/// own bookmarks, and keeps what it found this session. Rows that scroll back
/// into view show their image without asking again.
///
/// Hatena is asked first, where it has not already been through a feed. When
/// it has no image, the page's own `og:image` is read: Hatena leaves out
/// pages whose image is a WebP or AVIF file, for one.
@MainActor
final class EntryImageLookup {
    static let shared = EntryImageLookup()

    private let api = EntryImageAPI(http: AppServices.http)
    private let pageAPI = PageImageAPI(http: AppServices.http)
    /// The image of each page asked about, or nil for a page that has none.
    private var found: [URL: URL?] = [:]
    private var pending: [URL: Task<URL?, Never>] = [:]

    /// What is already known about `page` without waiting: `.some(nil)` for a
    /// page without an image, nil when it has not been asked about.
    func cached(_ page: URL) -> URL?? {
        found[page]
    }

    /// - Parameter asksHatena: false for a page whose feed already said
    ///   Hatena has no image for it.
    func imageURL(for page: URL, asksHatena: Bool = true) async -> URL? {
        if let known = found[page] {
            return known
        }
        return await lookUp(page, asksHatena: asksHatena).value
    }

    /// Looks up the pages not already known or on their way, and fetches the
    /// thumbnails they turn out to have.
    func prefetch(_ pages: some Sequence<URL>, asksHatena: Bool = true) {
        for page in pages where found[page] == nil && pending[page] == nil {
            let task = lookUp(page, asksHatena: asksHatena)
            Task {
                if let image = await task.value {
                    ThumbnailCache.shared.prefetch([image])
                }
            }
        }
    }

    private func lookUp(_ page: URL, asksHatena: Bool) -> Task<URL?, Never> {
        if let task = pending[page] {
            return task
        }
        let api = api
        let pageAPI = pageAPI
        let task = Task<URL?, Never> {
            defer { self.pending[page] = nil }
            if asksHatena {
                do {
                    if let image = try await api.imageURL(for: page) {
                        self.found[page] = .some(image)
                        return image
                    }
                } catch {
                    // Not kept: a later appearance of the row asks again.
                    FailureLog.record(error)
                    return nil
                }
            }
            do {
                let image = try await pageAPI.imageURL(for: page)
                self.found[page] = .some(image)
                return image
            } catch {
                // A site that is down or refuses the app is not Hatena
                // breaking, so it is not logged. Not kept either: a later
                // appearance of the row asks again.
                return nil
            }
        }
        pending[page] = task
        return task
    }
}
