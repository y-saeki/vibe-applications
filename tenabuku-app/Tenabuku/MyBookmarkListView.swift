import HatenaFeed
import HatenaOfficial
import HatenaUnofficial
import SwiftUI

@MainActor
@Observable
final class MyBookmarkListModel {
    /// The search the list shows, or an empty string for every bookmark.
    private(set) var query = ""
    private(set) var listing: any MyBookmarkListing = MyBookmarkTimeline()
    /// Whether a page has come back for `query`, which tells an empty list
    /// apart from one still on its way.
    private(set) var hasLoaded = false
    /// When the newest bookmarks of the timeline were read from Hatena. It
    /// can be long ago, since the timeline is shown from the saved copy.
    private(set) var readAt: Date?
    private(set) var error: Error?
    private(set) var isLoading = false
    /// The stars each bookmark has, by its star URI. A bookmark missing here
    /// has not been read yet.
    private(set) var starCounts: [URL: StarCount] = [:]
    /// Star URIs read or on their way, so that each is asked for once.
    @ObservationIgnored private var askedStars: Set<URL> = []
    /// Bumped by `show` and `refresh`, so that a page still on its way for
    /// the previous list is dropped when it arrives.
    @ObservationIgnored private var generation = 0

    /// Bookmarks whose stars are read in one go: a stretch of the list rather
    /// than each row on its own, which would send a request per row.
    static let starBatch = 40

    static let pageSize = 50

    /// Shows `query`. Every bookmark comes from the saved timeline when there
    /// is one, without asking Hatena: each search takes seconds, and pulling
    /// the list down reads it again.
    func show(_ query: String, api: BookmarkAPI, accounts: AccountStore) async {
        let generation = startOver()
        self.query = query
        error = nil
        if query.isEmpty, let user = accounts.account?.urlName,
           let saved = await AppServices.myBookmarks.load(user: user) {
            guard generation == self.generation else { return }
            listing = saved.timeline
            readAt = saved.savedAt
            hasLoaded = true
            return
        }
        listing = Self.start(query)
        readAt = nil
        hasLoaded = false
        await loadMore(api: api, accounts: accounts)
    }

    /// Reads `query` from the newest bookmark again. What is shown stays until
    /// the first page has arrived, and stays if reading fails.
    func refresh(api: BookmarkAPI, accounts: AccountStore) async {
        let generation = startOver()
        var fresh = Self.start(query)
        do {
            let target = Self.pageSize
            while fresh.hits.count < target, case let requests = fresh.nextRequests, !requests.isEmpty {
                for (request, page) in try await Self.read(requests, api: api) {
                    fresh.receive(page, for: request)
                }
                guard generation == self.generation else { return }
            }
            listing = fresh
            readAt = .now
            hasLoaded = true
            error = nil
            save(accounts)
            // The rows on screen do not appear again, so the stars they show
            // are read here; the rest as they come into view.
            askedStars = []
            loadStars(around: 0, user: accounts.account?.urlName)
        } catch {
            guard generation == self.generation else { return }
            accounts.record(error)
            self.error = error
        }
    }

    /// Reads pages until the list has grown by a page or has nothing more.
    func loadMore(api: BookmarkAPI, accounts: AccountStore) async {
        guard listing.hasMore, !isLoading else { return }
        let generation = generation
        isLoading = true
        defer {
            if generation == self.generation { isLoading = false }
        }
        let target = listing.hits.count + Self.pageSize
        do {
            while listing.hits.count < target, case let requests = listing.nextRequests, !requests.isEmpty {
                let pages = try await Self.read(requests, api: api)
                guard generation == self.generation else { return }
                for (request, page) in pages {
                    listing.receive(page, for: request)
                }
            }
            if readAt == nil { readAt = .now }
            hasLoaded = true
            error = nil
            save(accounts)
        } catch {
            guard generation == self.generation else { return }
            accounts.record(error)
            self.error = error
        }
    }

    /// Reads the stars of the stretch of the list that holds `index`, and of
    /// the next one once the row is close to it.
    func loadStars(around index: Int, user: String?) {
        guard let user else { return }
        let hits = listing.hits
        let batches = Set([index / Self.starBatch, (index + Self.starBatch / 2) / Self.starBatch])
        for batch in batches.sorted() {
            let start = batch * Self.starBatch
            guard start < hits.count else { continue }
            let uris = hits[start..<min(start + Self.starBatch, hits.count)]
                .compactMap { Self.starURI(of: $0, user: user) }
                .filter { !askedStars.contains($0) }
            guard !uris.isEmpty else { continue }
            askedStars.formUnion(uris)
            Task {
                do {
                    let counts = try await AppServices.stars.counts(for: uris)
                    starCounts.merge(counts) { _, new in new }
                } catch {
                    // Asked again when the rows next come into view.
                    askedStars.subtract(uris)
                    FailureLog.record(error)
                }
            }
        }
    }

    /// What Hatena Star names the bookmark of `user` by. A private bookmark
    /// can take no stars.
    static func starURI(of hit: MyBookmarkSearchHit, user: String) -> URL? {
        guard !hit.isPrivate, let eid = hit.eid, let date = hit.bookmarkedAt else { return nil }
        return HatenaStarAPI.uri(user: user, eid: eid, date: date)
    }

    func didChange(_ url: URL, to bookmark: MyBookmark?, accounts: AccountStore) {
        if let bookmark {
            listing.update(url, with: bookmark)
        } else {
            listing.remove(url)
        }
        save(accounts)
    }

    /// Drops whatever is still being read. A dropped read no longer clears
    /// `isLoading`, so it is cleared here.
    private func startOver() -> Int {
        generation += 1
        isLoading = false
        return generation
    }

    private static func start(_ query: String) -> any MyBookmarkListing {
        query.isEmpty ? MyBookmarkTimeline() : MyBookmarkPages(query: query)
    }

    /// Each request takes seconds whatever its size, so the pages a listing
    /// asks for together, such as the timeline's two searches, are read at
    /// the same time.
    private static func read(
        _ requests: [MyBookmarkSearchRequest], api: BookmarkAPI
    ) async throws -> [(MyBookmarkSearchRequest, MyBookmarkSearchPage)] {
        let limit = pageSize
        return try await withThrowingTaskGroup(of: (MyBookmarkSearchRequest, MyBookmarkSearchPage).self) { group in
            for request in requests {
                group.addTask {
                    (request, try await api.search(request.query, offset: request.offset, limit: limit))
                }
            }
            return try await group.reduce(into: []) { $0.append($1) }
        }
    }

    /// Keeps the timeline, so that the list opens with it next time. Search
    /// results are not kept.
    private func save(_ accounts: AccountStore) {
        guard let timeline = listing as? MyBookmarkTimeline,
              let user = accounts.account?.urlName,
              let readAt
        else { return }
        let saved = MyBookmarkTimelineStore.Saved(user: user, savedAt: readAt, timeline: timeline)
        Task {
            do {
                try await AppServices.myBookmarks.save(saved)
            } catch {
                FailureLog.record(error)
            }
        }
    }
}

/// Opens `MyBookmarkListView` from the hot entry list's navigation stack.
struct MyBookmarkListRoute: Hashable {}

/// The signed-in user's bookmarks, private ones included, and a search over
/// their titles, bodies, URLs and comments.
struct MyBookmarkListView: View {
    @Environment(AccountStore.self) private var accounts

    var body: some View {
        VerticalBarReader { barsMayBeVertical in
            let movesBack = Self.movesBackDown(barsMayBeVertical: barsMayBeVertical)
            // The same views whichever way the bars stand, so that the list
            // keeps what it has read when iPhone Duo opens or closes.
            Group {
                if let api = accounts.bookmarkAPI {
                    MyBookmarkList(api: api)
                } else {
                    SignInPrompt()
                }
            }
            .navigationTitle("マイブックマーク")
            // In the bar's own row: with going back at the bottom, the row
            // above the title would otherwise stand empty.
            .toolbarTitleDisplayMode(.inlineLarge)
            .backButtonMoved(movesBack)
            .toolbar {
                if movesBack {
                    ToolbarItem(placement: .bottomBar) {
                        BackButton()
                    }
                    if #available(iOS 26.0, *) {
                        // Apart, so that going back is a glass of its own
                        // beside the field rather than part of it.
                        ToolbarSpacer(.fixed, placement: .bottomBar)
                        DefaultToolbarItem(kind: .search, placement: .bottomBar)
                    }
                }
            }
        }
    }

    /// With bars at the top and bottom, going back sits at the bottom next to
    /// the search field. The search field only goes to the bottom from iOS 26,
    /// so before that the back button stays at the top too.
    private static func movesBackDown(barsMayBeVertical: Bool) -> Bool {
        guard #available(iOS 26.0, *) else { return false }
        return !barsMayBeVertical
    }
}

private struct MyBookmarkList: View {
    let api: BookmarkAPI
    @State private var model = MyBookmarkListModel()
    @State private var searchText = ""
    /// What was last submitted, as opposed to what is being typed: every
    /// search is a request, so it waits for the return key. Empty lists
    /// every bookmark.
    @State private var query = ""
    @State private var editing: MyBookmarkSearchHit?
    @Environment(AccountStore.self) private var accounts
    @Environment(ReadHistoryStore.self) private var readHistory
    @Environment(FaviconSettingsStore.self) private var faviconSettings
    @Environment(\.openEntryFromPreview) private var openFromPreview

    /// Signing in again gives a new token, which starts the list over.
    private struct LoadKey: Equatable {
        let query: String
        let account: AccountStore.Account?
    }

    var body: some View {
        List {
            if let error = model.error {
                ErrorBanner(error: error)
            }
            if model.query.isEmpty, let readAt = model.readAt {
                Text("\(Text(readAt, style: .relative))前に読み込みました。引っ張ると更新します")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .listRowSeparator(.hidden)
            }
            ForEach(model.listing.hits) { hit in
                // A value the hot entry list's stack already knows how to open.
                let entry = FeedItem(title: hit.title, link: hit.url, bookmarkCount: hit.bookmarkCount)
                let account = accounts.account
                NavigationLink(value: entry) {
                    MyBookmarkRow(
                        hit: hit,
                        user: account?.urlName ?? "",
                        iconURL: account?.iconURL,
                        stars: account
                            .flatMap { MyBookmarkListModel.starURI(of: hit, user: $0.urlName) }
                            .flatMap { model.starCounts[$0] },
                        isRead: readHistory.isRead(hit.url)
                    )
                        .pagePreviewMenu(
                            url: hit.url,
                            title: hit.title.isEmpty ? hit.url.absoluteString : hit.title,
                            actions: [
                                PagePreviewAction(title: "編集", systemImage: "pencil") { editing = hit },
                            ],
                            open: { openFromPreview(entry) }
                        )
                }
                // As on the home screen: the row is an entry, plainly a link,
                // and a chevron would float between the entry and the comment.
                .navigationLinkIndicatorVisibility(.hidden)
                .onAppear {
                    // Starts on the next page well before the end comes
                    // into view, since one takes seconds to arrive.
                    if hit.id == prefetchMark && model.error == nil {
                        Task { await model.loadMore(api: api, accounts: accounts) }
                    }
                    prefetchImages(after: hit)
                }
            }
            if !model.listing.hits.isEmpty && model.listing.hasMore {
                footer
            }
            if let timeline = model.listing as? MyBookmarkTimeline, timeline.isCutShort {
                Text("これより前のブックマークは、検索すると見つかります")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity)
                    .listRowSeparator(.hidden)
            }
        }
        .listStyle(.plain)
        .searchable(text: $searchText, prompt: "タイトル、本文、URL、コメント")
        .onSubmit(of: .search) {
            query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        }
        .onChange(of: searchText) { _, text in
            // Clearing the field, or its cancel button, goes back to the whole list.
            if text.isEmpty { query = "" }
        }
        .overlay {
            if model.listing.hits.isEmpty {
                emptyState
            }
        }
        .sheet(item: $editing) { hit in
            NavigationStack {
                BookmarkEditorView(url: hit.url, title: hit.title) { bookmark in
                    model.didChange(hit.url, to: bookmark, accounts: accounts)
                }
            }
        }
        .refreshable { await model.refresh(api: api, accounts: accounts) }
        .task(id: LoadKey(query: query, account: accounts.account)) {
            await model.show(query, api: api, accounts: accounts)
        }
    }

    /// Fetches the favicons, thumbnails and stars of the rows coming up.
    private func prefetchImages(after hit: MyBookmarkSearchHit) {
        let hits = model.listing.hits
        guard let index = hits.firstIndex(where: { $0.id == hit.id }) else { return }
        ImagePrefetch.rows(hits, after: index, page: \.url, faviconSources: faviconSettings.enabledSources)
        let ahead = hits[index..<min(index + 1 + ImagePrefetch.lookahead, hits.count)]
        EntryImageLookup.shared.prefetch(ahead.map(\.url))
        model.loadStars(around: index, user: accounts.account?.urlName)
    }

    /// The row whose appearance starts reading the next page.
    private var prefetchMark: String? {
        let hits = model.listing.hits
        return hits.isEmpty ? nil : hits[max(hits.count - Self.prefetchDistance, 0)].id
    }

    private static let prefetchDistance = 25

    /// Reads the next page as it scrolls into view, if it is not already on
    /// its way. After a failure it waits for a tap, so that a page that keeps
    /// failing is not asked for in a loop.
    @ViewBuilder
    private var footer: some View {
        if model.error == nil {
            ProgressView()
                .frame(maxWidth: .infinity)
                .listRowSeparator(.hidden)
                .task(id: model.listing.hits.count) { await model.loadMore(api: api, accounts: accounts) }
        } else {
            Button("続きを読み込む") { Task { await model.loadMore(api: api, accounts: accounts) } }
                .frame(maxWidth: .infinity)
                .listRowSeparator(.hidden)
        }
    }

    @ViewBuilder
    private var emptyState: some View {
        if model.isLoading {
            ProgressView()
        } else if model.error == nil && model.hasLoaded {
            if model.query.isEmpty {
                ContentUnavailableView("ブックマークはまだありません", systemImage: "bookmark")
            } else {
                ContentUnavailableView.search(text: model.query)
            }
        }
    }
}

/// A bookmark laid out as the hot entry list shows its entry, with the
/// user's own comment below it as the comment sheet shows comments.
private struct MyBookmarkRow: View {
    let hit: MyBookmarkSearchHit
    let user: String
    let iconURL: URL?
    /// Nil until the count is read, or when the bookmark can take no stars.
    let stars: StarCount?
    /// Dims the title, as in the hot entry list.
    let isRead: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            EntryRow(
                item: FeedItem(
                    title: hit.title.isEmpty ? hit.url.absoluteString : hit.title,
                    link: hit.url,
                    bookmarkCount: hit.bookmarkCount
                ),
                isRead: isRead,
                looksUpImage: true,
                isPrivate: hit.isPrivate
            )
            if showsComment {
                // Only counts the stars: adding one is left to the comment
                // sheet, where everyone's comments are.
                CommentRow(
                    comment: CommentRowData(
                        user: user,
                        iconURL: iconURL,
                        comment: hit.commentBody,
                        tags: hit.tags,
                        date: hit.bookmarkedAt,
                        starURI: nil
                    ),
                    stars: stars
                )
                .padding(.bottom, 4)
            }
        }
        .accessibilityElement(children: .combine)
    }

    /// A bookmark with nothing but the date to say keeps to the entry.
    private var showsComment: Bool {
        !hit.commentBody.isEmpty || !hit.tags.isEmpty || (stars?.total ?? 0) > 0
    }
}
