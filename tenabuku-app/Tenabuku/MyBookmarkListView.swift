import HatenaFeed
import HatenaOfficial
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
    /// Bumped by `show` and `refresh`, so that a page still on its way for
    /// the previous list is dropped when it arrives.
    @ObservationIgnored private var generation = 0

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
        Group {
            if let api = accounts.bookmarkAPI {
                MyBookmarkList(api: api)
            } else {
                SignInPrompt()
            }
        }
        .navigationTitle("マイブックマーク")
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
                NavigationLink(value: FeedItem(title: hit.title, link: hit.url, bookmarkCount: hit.bookmarkCount)) {
                    MyBookmarkRow(hit: hit, isRead: readHistory.isRead(hit.url))
                }
                .onAppear {
                    // Starts on the next page well before the end comes
                    // into view, since one takes seconds to arrive.
                    if hit.id == prefetchMark && model.error == nil {
                        Task { await model.loadMore(api: api, accounts: accounts) }
                    }
                }
                .swipeActions(edge: .leading) {
                    Button("編集", systemImage: "pencil") { editing = hit }
                        .tint(.blue)
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

private struct MyBookmarkRow: View {
    let hit: MyBookmarkSearchHit
    /// Dims the title, as in the hot entry list.
    let isRead: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 8) {
                if let host = hit.url.host() {
                    HStack(spacing: 4) {
                        FaviconView(pageURL: hit.url)
                        Text(host).lineLimit(1)
                    }
                }
                Spacer(minLength: 0)
                if hit.isPrivate {
                    Image(systemName: "lock.fill")
                        .accessibilityLabel("非公開")
                }
                Text("\(hit.bookmarkCount) users")
                    .foregroundStyle(.red)
                    .bold()
                    .fixedSize()
            }
            .font(.caption)
            .foregroundStyle(.secondary)
            Text(hit.title.isEmpty ? hit.url.absoluteString : hit.title)
                .font(.body)
                .foregroundStyle(isRead ? .secondary : .primary)
                .lineLimit(3)
                // Drawn below the site line but read out first.
                .accessibilitySortPriority(1)
            if !hit.comment.isEmpty {
                Text(hit.comment)
                    .font(.subheadline)
                    .lineLimit(3)
            }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityValue(isRead ? Text("既読") : Text(""))
    }
}
