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
    private(set) var error: Error?
    private(set) var isLoading = false
    /// Bumped by every reload, so that a page still on its way for the
    /// previous query or account is dropped when it arrives.
    @ObservationIgnored private var generation = 0

    static let pageSize = 50

    /// Starts over from the newest bookmark matching `query`.
    func reload(_ query: String, api: BookmarkAPI, accounts: AccountStore) async {
        generation += 1
        self.query = query
        listing = query.isEmpty ? MyBookmarkTimeline() : MyBookmarkPages(query: query)
        hasLoaded = false
        error = nil
        await fetch(api: api, accounts: accounts)
    }

    func loadMore(api: BookmarkAPI, accounts: AccountStore) async {
        guard listing.hasMore, !isLoading else { return }
        await fetch(api: api, accounts: accounts)
    }

    func didChange(_ url: URL, to bookmark: MyBookmark?) {
        if let bookmark {
            listing.update(url, with: bookmark)
        } else {
            listing.remove(url)
        }
    }

    /// Reads pages until the list grows or has nothing more. The timeline
    /// shows nothing until both of its searches have answered, so one call
    /// can take more than one page.
    private func fetch(api: BookmarkAPI, accounts: AccountStore) async {
        let generation = generation
        isLoading = true
        defer {
            if generation == self.generation { isLoading = false }
        }
        let shown = listing.hits.count
        do {
            while let request = listing.nextRequest {
                let page = try await api.search(request.query, offset: request.offset, limit: Self.pageSize)
                guard generation == self.generation else { return }
                listing.receive(page, for: request)
                if listing.hits.count > shown { break }
            }
            hasLoaded = true
            error = nil
        } catch {
            guard generation == self.generation else { return }
            accounts.record(error)
            self.error = error
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
            ForEach(model.listing.hits) { hit in
                // A value the hot entry list's stack already knows how to open.
                NavigationLink(value: FeedItem(title: hit.title, link: hit.url, bookmarkCount: hit.bookmarkCount)) {
                    MyBookmarkRow(hit: hit, isRead: readHistory.isRead(hit.url))
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
                    model.didChange(hit.url, to: bookmark)
                }
            }
        }
        .refreshable { await model.reload(query, api: api, accounts: accounts) }
        .task(id: LoadKey(query: query, account: accounts.account)) {
            await model.reload(query, api: api, accounts: accounts)
        }
    }

    /// Reads the next page as it scrolls into view. After a failure it waits
    /// for a tap, so that a page that keeps failing is not asked for in a loop.
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
            Text(hit.title.isEmpty ? hit.url.absoluteString : hit.title)
                .font(.body)
                .foregroundStyle(isRead ? .secondary : .primary)
                .lineLimit(3)
            if !hit.comment.isEmpty {
                Text(hit.comment)
                    .font(.subheadline)
                    .lineLimit(3)
            }
            HStack(spacing: 8) {
                if hit.isPrivate {
                    Image(systemName: "lock.fill")
                        .accessibilityLabel("非公開")
                }
                Text("\(hit.bookmarkCount) users")
                    .foregroundStyle(.red)
                    .bold()
                if let host = hit.url.host() {
                    Text(host).lineLimit(1)
                }
                if let date = hit.bookmarkedAt {
                    Text(date, format: .dateTime.year().month().day())
                }
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityValue(isRead ? Text("既読") : Text(""))
    }
}
