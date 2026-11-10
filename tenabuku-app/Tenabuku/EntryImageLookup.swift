import Foundation
import HatenaUnofficial

/// Finds the thumbnail of entries that came without one, such as the user's
/// own bookmarks, and keeps what it found this session. Rows that scroll back
/// into view show their image without asking again.
@MainActor
final class EntryImageLookup {
    static let shared = EntryImageLookup()

    private let api = EntryImageAPI(http: AppServices.http)
    /// The image of each page asked about, or nil for a page that has none.
    private var found: [URL: URL?] = [:]
    private var pending: [URL: Task<URL?, Never>] = [:]

    /// What is already known about `page` without waiting: `.some(nil)` for a
    /// page without an image, nil when it has not been asked about.
    func cached(_ page: URL) -> URL?? {
        found[page]
    }

    func imageURL(for page: URL) async -> URL? {
        if let known = found[page] {
            return known
        }
        return await lookUp(page).value
    }

    /// Looks up the pages not already known or on their way, and fetches the
    /// thumbnails they turn out to have.
    func prefetch(_ pages: some Sequence<URL>) {
        for page in pages where found[page] == nil && pending[page] == nil {
            let task = lookUp(page)
            Task {
                if let image = await task.value {
                    ThumbnailCache.shared.prefetch([image])
                }
            }
        }
    }

    private func lookUp(_ page: URL) -> Task<URL?, Never> {
        if let task = pending[page] {
            return task
        }
        let api = api
        let task = Task<URL?, Never> {
            defer { self.pending[page] = nil }
            do {
                let image = try await api.imageURL(for: page)
                self.found[page] = .some(image)
                return image
            } catch {
                // Not kept: a later appearance of the row asks again.
                FailureLog.record(error)
                return nil
            }
        }
        pending[page] = task
        return task
    }
}
