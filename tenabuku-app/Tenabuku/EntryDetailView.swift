import HatenaFeed
import HatenaOfficial
import HatenaShared
import HatenaUnofficial
import SwiftUI

/// One comment as the list shows it, whichever endpoint it came from.
struct CommentRowData: Identifiable {
    let user: String
    let iconURL: URL?
    let comment: String
    let tags: [String]
    let date: Date?
    /// What Hatena Star names the comment by; nil when it could not be told.
    let starURI: URL?

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
    /// The stars each comment has, by its star URI. A comment missing here
    /// has not been counted yet, or its count could not be read.
    private(set) var starCounts: [URL: StarCount] = [:]
    /// Comments whose star is on its way to Hatena, so that it is not sent twice.
    private(set) var addingStars: Set<URL> = []
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
            starCounts = [:]
            loadStars(for: comments, generation: generation)
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
            let added = page.comments.map(CommentRowData.init).filter { !skipped.contains($0.user) }
            comments += added
            nextCursor = page.nextCursor
            loadStars(for: added, generation: generation)
        } catch {
            guard generation == self.generation else { return }
            FailureLog.record(error)
            self.error = error
        }
    }

    /// Reads the star counts after the comments are on screen, so that the
    /// comments never wait for them. A failure only leaves the counts out.
    private func loadStars(for comments: [CommentRowData], generation: Int) {
        let uris = comments.compactMap(\.starURI)
        guard !uris.isEmpty else { return }
        Task {
            do {
                let counts = try await AppServices.stars.counts(for: uris)
                guard generation == self.generation else { return }
                starCounts.merge(counts) { _, new in new }
            } catch {
                FailureLog.record(error)
            }
        }
    }

    /// Adds one star to `comment` as the signed-in user. The count goes up
    /// once Hatena has taken it.
    func addStar(to comment: CommentRowData, webSession: WebSessionStore) async throws {
        guard let uri = comment.starURI, !addingStars.contains(uri) else { return }
        addingStars.insert(uri)
        defer { addingStars.remove(uri) }
        try await webSession.addStar(to: uri)
        starCounts[uri, default: .none].yellow += 1
    }

    // The ranking only exists in the website's entry page.
    private func popular() async throws -> [CommentRowData] {
        try await AppServices.popularComments.comments(for: url).map(CommentRowData.init)
    }
}

extension CommentRowData {
    init(_ comment: EntryComment) {
        self.init(
            user: comment.user, iconURL: comment.iconURL, comment: comment.comment, tags: comment.tags,
            date: comment.date, starURI: comment.starURI
        )
    }
}

struct EntryDetailView: View {
    let entry: FeedItem
    @State private var model: EntryCommentsModel
    @State private var isShowingBookmarkEditor = false
    /// The comment a star was meant for when Hatena asked for the website
    /// login; the star is sent once the login is done.
    @State private var pendingStar: CommentRowData?
    @State private var isShowingWebSignIn = false
    @State private var starError: Error?
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
                header
                    .listRowSeparator(.hidden)

                if let error = model.error {
                    ErrorBanner(error: error)
                } else if model.isLoading && model.comments.isEmpty {
                    ProgressView().frame(maxWidth: .infinity)
                } else if model.comments.isEmpty {
                    Text("コメントはまだありません").foregroundStyle(.secondary)
                }
                ForEach(model.comments) { comment in
                    CommentRow(
                        comment: comment,
                        stars: comment.starURI.flatMap { model.starCounts[$0] },
                        isAddingStar: comment.starURI.map(model.addingStars.contains) ?? false,
                        addStar: comment.starURI == nil ? nil : { Task { await addStar(to: comment) } }
                    )
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
        // No title in the bar: the count sits in the list beside the tabs, so
        // that no row of the bar is spent on it, and on iPhone Duo's outer
        // display, where the buttons go to the side, the comments start at the top.
        .navigationBarTitleDisplayMode(.inline)
        .simultaneousGesture(tabSwipe)
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
        .webSignInSheet(isPresented: $isShowingWebSignIn) {
            if let comment = pendingStar {
                Task { await addStar(to: comment) }
            }
        }
        .onChange(of: isShowingWebSignIn) { _, isShowing in
            // Cancelling the login drops the star it was for.
            if !isShowing { pendingStar = nil }
        }
        .alert(
            "スターを付けられませんでした",
            isPresented: Binding(get: { starError != nil }, set: { if !$0 { starError = nil } }),
            presenting: starError
        ) { _ in
            Button("OK") {}
        } message: { error in
            Text(Self.starErrorMessage(error))
        }
        .refreshable { await model.load(webSession: webSession, refreshIgnored: true) }
        .task(id: model.tab) { await model.load(webSession: webSession) }
    }

    /// The bookmark count, and the tabs beside it.
    private var header: some View {
        HStack {
            Text("\(entry.bookmarkCount) users")
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
            Spacer()
            Picker("並び順", selection: $model.tab) {
                ForEach(EntryCommentsModel.Tab.allCases) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .pickerStyle(.segmented)
            .fixedSize()
        }
    }

    /// Swiping sideways moves to the tab on that side, as the tabs are laid
    /// out: to the left for the one on the right.
    private var tabSwipe: some Gesture {
        DragGesture(minimumDistance: 30)
            .onEnded { value in
                let dx = value.translation.width
                // Clearly sideways, so that scrolling the list never switches.
                guard abs(dx) > 60, abs(dx) > abs(value.translation.height) * 2 else { return }
                let tabs = EntryCommentsModel.Tab.allCases
                guard let index = tabs.firstIndex(of: model.tab) else { return }
                let next = dx < 0 ? tabs.index(after: index) : index - 1
                guard tabs.indices.contains(next) else { return }
                withAnimation { model.tab = tabs[next] }
            }
    }

    private func addStar(to comment: CommentRowData) async {
        do {
            try await model.addStar(to: comment, webSession: webSession)
        } catch HatenaError.unauthorized {
            pendingStar = comment
            isShowingWebSignIn = true
        } catch {
            FailureLog.record(error)
            starError = error
        }
    }

    private static func starErrorMessage(_ error: Error) -> String {
        if case let .rejected(reason)? = error as? HatenaError {
            return "はてなが受け付けませんでした（\(reason)）"
        }
        return ErrorBanner.title(for: error) + "\n" + ErrorBanner.detail(for: error)
    }
}

struct CommentRow: View {
    let comment: CommentRowData
    /// Nil until the count is read, or when it could not be.
    var stars: StarCount?
    var isAddingStar = false
    /// Nil when the comment cannot take a star.
    var addStar: (() -> Void)?

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            AsyncImage(url: comment.iconURL) { image in
                image.resizable().scaledToFill()
            } placeholder: {
                Image(systemName: "person.crop.circle.fill").resizable().foregroundStyle(.tertiary)
            }
            .frame(width: 32, height: 32)
            .clipShape(Circle())

            VStack(alignment: .leading, spacing: 2) {
                // The user, the comment and the tags run on as one paragraph,
                // so that a short comment takes a single line.
                commentText.font(.subheadline)
                HStack(spacing: 12) {
                    if let date = comment.date {
                        Text(date, format: .dateTime.month().day().hour().minute())
                            .font(.caption2)
                            .foregroundStyle(.secondary)
                    }
                    if addStar != nil || (stars?.total ?? 0) > 0 {
                        starRow
                    }
                }
            }
        }
        .padding(.vertical, 2)
    }

    private var commentText: Text {
        let user = Text(comment.user).bold()
        let body = Text(comment.comment)
        guard !comment.tags.isEmpty else { return Text("\(user)  \(body)") }
        let tags = Text(comment.tags.map { "#\($0)" }.joined(separator: " "))
            .font(.caption)
            .foregroundStyle(.secondary)
        return Text("\(user)  \(body)  \(tags)")
    }

    /// The coloured stars, then the yellow ones as the button that adds one,
    /// as Hatena shows them.
    private var starRow: some View {
        HStack(spacing: 10) {
            ForEach(stars?.colored ?? [], id: \.color) { colored in
                starCount(colored.count, color: Self.color(named: colored.color))
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("\(Self.colorName(colored.color))のスター \(colored.count) 個")
            }
            if let addStar {
                Button(action: addStar) {
                    starCount(stars?.yellow, color: .yellow)
                        .opacity(isAddingStar ? 0.4 : 1)
                        // A target large enough to hit without hitting the row.
                        .frame(minWidth: 44, minHeight: 28, alignment: .leading)
                        .contentShape(Rectangle())
                }
                // Only the star reacts to a tap, not the whole row.
                .buttonStyle(.borderless)
                .disabled(isAddingStar)
                .accessibilityLabel("スターを付ける")
                .accessibilityValue(stars.map { "\($0.yellow) 個" } ?? "")
            }
        }
        .font(.caption)
        .frame(minHeight: 28)
    }

    /// A star and how many there are; an empty star while nil or none.
    private func starCount(_ count: Int?, color: Color) -> some View {
        HStack(spacing: 3) {
            Image(systemName: (count ?? 0) > 0 ? "star.fill" : "star")
                .foregroundStyle(color)
            if let count {
                Text(count, format: .number)
                    .monospacedDigit()
                    // Plain grey: `.secondary` would take the button's tint.
                    .foregroundStyle(Color.secondary)
            }
        }
    }

    private static func color(named name: String) -> Color {
        switch name {
        case "green": .green
        case "red": .red
        case "blue": .blue
        case "purple": .purple
        // A colour Hatena adds later still shows, in a neutral tone.
        default: .gray
        }
    }

    private static func colorName(_ name: String) -> String {
        switch name {
        case "green": "グリーン"
        case "red": "レッド"
        case "blue": "ブルー"
        case "purple": "パープル"
        default: name
        }
    }
}
