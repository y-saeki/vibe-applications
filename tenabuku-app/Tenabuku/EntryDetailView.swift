import HatenaFeed
import HatenaOfficial
import HatenaUnofficial
import SwiftUI

/// One comment as the list shows it, whichever endpoint it came from.
struct CommentRowData: Identifiable {
    let user: String
    let iconURL: URL?
    let comment: String
    let tags: [String]
    let date: Date?

    var id: String { user }
}

@MainActor
@Observable
final class EntryCommentsModel {
    enum Tab: String, CaseIterable, Identifiable {
        case popular = "注目"
        case recent = "新着"

        var id: Self { self }
    }

    let url: URL
    var tab: Tab = .popular
    private(set) var comments: [CommentRowData] = []
    private(set) var error: Error?
    private(set) var isLoading = false
    /// Where the recent comments continue; nil when all are shown.
    private var nextCursor: String?
    /// The users hidden on Hatena's website, whose comments are left out as
    /// the website does.
    private var ignoredUsers: Set<String> = []
    /// Bumped by every `load`, so that a page arriving for an earlier tab or
    /// an earlier pull is dropped.
    private var generation = 0

    var hasMore: Bool { nextCursor != nil }

    init(url: URL) {
        self.url = url
    }

    /// `refreshIgnored` asks Hatena for the hidden users again rather than
    /// reusing the ones read a little earlier.
    func load(webSession: WebSessionStore, refreshIgnored: Bool = false) async {
        generation += 1
        let generation = generation
        nextCursor = nil
        isLoading = true
        defer {
            if generation == self.generation { isLoading = false }
        }
        // Read alongside the comments, so that hiding costs no extra wait.
        async let ignored = webSession.ignoredUsers(refresh: refreshIgnored)
        do {
            let loaded: [CommentRowData]
            switch tab {
            case .popular:
                loaded = try await popular()
            case .recent:
                let page = try await AppServices.recentComments.comments(for: url)
                guard generation == self.generation else { return }
                nextCursor = page.nextCursor
                loaded = page.comments.map(CommentRowData.init)
            }
            let ignoredUsers = await ignored
            guard generation == self.generation else { return }
            self.ignoredUsers = ignoredUsers
            comments = loaded.filter { !ignoredUsers.contains($0.user) }
            error = nil
        } catch {
            _ = await ignored
            guard generation == self.generation else { return }
            FailureLog.record(error)
            comments = []
            self.error = error
        }
    }

    /// Appends the next page of recent comments.
    func loadMore() async {
        guard tab == .recent, let cursor = nextCursor, !isLoading, error == nil else { return }
        let generation = generation
        isLoading = true
        defer {
            if generation == self.generation { isLoading = false }
        }
        do {
            let page = try await AppServices.recentComments.comments(for: url, cursor: cursor)
            guard generation == self.generation else { return }
            // A comment can move to a later page when someone edits theirs
            // while the list is being read; the list keys rows by user.
            let skipped = Set(comments.map(\.user)).union(ignoredUsers)
            comments += page.comments.map(CommentRowData.init).filter { !skipped.contains($0.user) }
            nextCursor = page.nextCursor
        } catch {
            guard generation == self.generation else { return }
            FailureLog.record(error)
            self.error = error
        }
    }

    // The ranking only exists in the website's entry page.
    private func popular() async throws -> [CommentRowData] {
        try await AppServices.popularComments.comments(for: url).map(CommentRowData.init)
    }
}

extension CommentRowData {
    init(_ comment: EntryComment) {
        self.init(user: comment.user, iconURL: comment.iconURL, comment: comment.comment, tags: comment.tags, date: comment.date)
    }
}

struct EntryDetailView: View {
    let entry: FeedItem
    @State private var model: EntryCommentsModel
    @State private var isShowingBookmarkEditor = false
    @Environment(\.openURL) private var openURL
    @Environment(WebSessionStore.self) private var webSession

    init(entry: FeedItem) {
        self.entry = entry
        _model = State(initialValue: EntryCommentsModel(url: entry.link))
    }

    /// The row that starts loading the next page: some way before the end.
    private var prefetchMark: String? {
        let comments = model.comments
        return comments.isEmpty ? nil : comments[max(0, comments.count - 20)].id
    }

    var body: some View {
        List {
            Section {
                Picker("並び順", selection: $model.tab) {
                    ForEach(EntryCommentsModel.Tab.allCases) { tab in
                        Text(tab.rawValue).tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .listRowSeparator(.hidden)

                if let error = model.error {
                    ErrorBanner(error: error)
                } else if model.isLoading && model.comments.isEmpty {
                    ProgressView().frame(maxWidth: .infinity)
                } else if model.comments.isEmpty {
                    Text("コメントはまだありません").foregroundStyle(.secondary)
                }
                ForEach(model.comments) { comment in
                    CommentRow(comment: comment)
                        .onAppear {
                            // Starts on the next page before the end comes into view.
                            if comment.id == prefetchMark {
                                Task { await model.loadMore() }
                            }
                        }
                }
                if model.hasMore && model.error == nil {
                    // Also catches a short list, whose mark is already on screen.
                    ProgressView().frame(maxWidth: .infinity)
                        .task(id: model.comments.count) { await model.loadMore() }
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("\(entry.bookmarkCount) users")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItemGroup(placement: .primaryAction) {
                Button("ブクマする", systemImage: "bookmark") {
                    isShowingBookmarkEditor = true
                }
                Button("Safari で開く", systemImage: "safari") {
                    openURL(entry.link)
                }
            }
        }
        .sheet(isPresented: $isShowingBookmarkEditor) {
            NavigationStack {
                BookmarkEditorView(url: entry.link, title: entry.title)
            }
        }
        .refreshable { await model.load(webSession: webSession, refreshIgnored: true) }
        .task(id: model.tab) { await model.load(webSession: webSession) }
    }
}

struct CommentRow: View {
    let comment: CommentRowData

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            AsyncImage(url: comment.iconURL) { image in
                image.resizable().scaledToFill()
            } placeholder: {
                Image(systemName: "person.crop.circle.fill").resizable().foregroundStyle(.tertiary)
            }
            .frame(width: 32, height: 32)
            .clipShape(Circle())

            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text(comment.user).font(.caption.bold())
                    Spacer()
                    if let date = comment.date {
                        Text(date, format: .dateTime.month().day().hour().minute())
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                }
                Text(comment.comment).font(.subheadline)
                if !comment.tags.isEmpty {
                    Text(comment.tags.map { "#\($0)" }.joined(separator: " "))
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                }
            }
        }
        .padding(.vertical, 2)
    }
}
