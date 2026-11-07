import HatenaFeed
import HatenaShared
import HatenaUnofficial
import SwiftUI

/// What the main list shows: one of Hatena's hot entry categories, or the
/// user's own My hot entries.
enum EntryListSource: Hashable, Identifiable {
    case hotEntry(HotEntryCategory)
    case myHotEntry

    static let allCases: [EntryListSource] = HotEntryCategory.allCases.map(Self.hotEntry) + [.myHotEntry]

    var id: Self { self }

    var displayName: String {
        switch self {
        case let .hotEntry(category): category.displayName
        case .myHotEntry: "マイホットエントリー"
        }
    }
}

@MainActor
@Observable
final class HotEntryListModel {
    var source: EntryListSource = .hotEntry(.all)
    private(set) var items: [FeedItem] = []
    private(set) var error: Error?
    private(set) var isLoading = false
    /// Whether `items` came back for the current source, which tells an empty
    /// My hot entries apart from one still on its way.
    private(set) var hasLoaded = false
    /// Set when My hot entries needs the website login.
    private(set) var needsWebSignIn = false

    func load(webSession: WebSessionStore) async {
        let source = source
        isLoading = true
        defer { isLoading = false }
        do {
            let items = try await Self.fetch(source, webSession: webSession)
            // A slow answer for a source the user has since left is dropped.
            guard source == self.source else { return }
            self.items = items
            error = nil
            needsWebSignIn = false
            hasLoaded = true
        } catch HatenaError.unauthorized where source == .myHotEntry {
            guard source == self.source else { return }
            items = []
            error = nil
            needsWebSignIn = true
            hasLoaded = true
        } catch {
            guard source == self.source else { return }
            FailureLog.record(error)
            self.error = error
            needsWebSignIn = false
            hasLoaded = true
        }
    }

    /// Clears what the previous source showed, so that it is not taken for
    /// the new one while that loads.
    func didChangeSource() {
        items = []
        error = nil
        needsWebSignIn = false
        hasLoaded = false
    }

    private static func fetch(_ source: EntryListSource, webSession: WebSessionStore) async throws -> [FeedItem] {
        switch source {
        case let .hotEntry(category):
            return try await AppServices.feed.hotEntries(category)
        case .myHotEntry:
            return try await webSession.myHotEntries().map { entry in
                FeedItem(
                    title: entry.title,
                    link: entry.url,
                    summary: entry.summary,
                    date: entry.date,
                    bookmarkCount: entry.bookmarkCount,
                    imageURL: entry.imageURL
                )
            }
        }
    }
}

struct HotEntryListView: View {
    @State private var model = HotEntryListModel()
    @State private var isShowingSettings = false
    @State private var bookmarking: FeedItem?
    @State private var muting: MuteSource?
    @Environment(ReadHistoryStore.self) private var readHistory
    @Environment(MuteStore.self) private var mutes
    @Environment(WebSessionStore.self) private var webSession

    var body: some View {
        VerticalBarReader { barsMayBeVertical in
            list(barsMayBeVertical: barsMayBeVertical)
        }
    }

    private func list(barsMayBeVertical: Bool) -> some View {
        NavigationStack {
            List {
                if model.needsWebSignIn {
                    WebSignInPrompt()
                        .listRowSeparator(.hidden)
                } else if model.source == .myHotEntry, model.hasLoaded, model.error == nil, model.items.isEmpty {
                    ContentUnavailableView(
                        "記事がありません",
                        systemImage: "tray",
                        description: Text("お気に入りのユーザーと関心ワードは、はてなの Web で設定します。")
                    )
                    .listRowSeparator(.hidden)
                }
                if let error = model.error {
                    ErrorBanner(error: error)
                }
                ForEach(visibleItems) { item in
                    NavigationLink(value: item) {
                        EntryRow(item: item, isRead: readHistory.isRead(item.link))
                    }
                    .swipeActions(edge: .leading) {
                        Button("ブクマする", systemImage: "bookmark") { bookmarking = item }
                            .tint(.blue)
                    }
                    .swipeActions(edge: .trailing) {
                        Button("ミュート", systemImage: "eye.slash") {
                            muting = MuteSource(title: item.title, url: item.link)
                        }
                        .tint(.gray)
                    }
                }
                // So that a list emptied by the user's own rules is not taken
                // for a feed that came back empty.
                if case let hidden = model.items.count - visibleItems.count, hidden > 0 {
                    Text("ミュートで \(hidden) 件を隠しています")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .listRowSeparator(.hidden)
                }
            }
            .listStyle(.plain)
            .navigationTitle(model.source.displayName)
            .navigationDestination(for: FeedItem.self) { item in
                ArticleBrowserView(entry: item)
            }
            .navigationDestination(for: MyBookmarkListRoute.self) { _ in
                MyBookmarkListView()
            }
            .toolbar {
                // Apple asks for the top of a vertical bar to hold navigation and
                // prominent actions, so where bars can stand vertically, as on
                // iPhone Duo's outer display, the rarely used settings go to the
                // foot of it. Elsewhere they take the usual top leading corner.
                if barsMayBeVertical {
                    ToolbarItemGroup(placement: .bottomBar) {
                        settingsButton
                        Spacer()
                    }
                } else {
                    ToolbarItem(placement: .topBarLeading) {
                        settingsButton
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    // A value, like the rows, so that the stack resolves every push the same way.
                    NavigationLink(value: MyBookmarkListRoute()) {
                        Label("マイブックマーク", systemImage: "books.vertical")
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    Menu {
                        Picker("カテゴリー", selection: $model.source) {
                            ForEach(EntryListSource.allCases) { source in
                                Text(source.displayName).tag(source)
                            }
                        }
                    } label: {
                        Label("カテゴリー", systemImage: "square.grid.2x2")
                    }
                }
            }
            .sheet(item: $bookmarking) { item in
                NavigationStack {
                    BookmarkEditorView(url: item.link, title: item.title)
                }
            }
            .sheet(item: $muting) { source in
                NavigationStack {
                    MuteRuleEditorView(source: source)
                }
            }
            .sheet(isPresented: $isShowingSettings) {
                NavigationStack {
                    SettingsView()
                }
            }
            .overlay {
                if model.isLoading && model.items.isEmpty {
                    ProgressView()
                }
            }
            .refreshable { await model.load(webSession: webSession) }
            .task(id: model.source) { await model.load(webSession: webSession) }
            .onChange(of: model.source) { model.didChangeSource() }
            // Signing in from the prompt, or from Settings, brings the list in.
            .onChange(of: webSession.isSignedIn) { _, isSignedIn in
                if isSignedIn, model.needsWebSignIn {
                    Task { await model.load(webSession: webSession) }
                }
            }
        }
    }

    private var visibleItems: [FeedItem] {
        model.items.filter { !mutes.isMuted(title: $0.title, url: $0.link) }
    }

    private var settingsButton: some View {
        Button("設定", systemImage: "gearshape") { isShowingSettings = true }
    }
}

/// Tells its content whether the system may lay the bars out vertically here,
/// by reading `toolbarVerticalEdge`: a trait of the context rather than the
/// device model, as Apple's iPhone Duo guidance asks. The value only exists
/// from iOS 27.1; earlier systems count as never placing a vertical bar.
private struct VerticalBarReader<Content: View>: View {
    @ViewBuilder let content: (Bool) -> Content

    var body: some View {
        if #available(iOS 27.1, *) {
            Reading(content: content)
        } else {
            content(false)
        }
    }

    @available(iOS 27.1, *)
    private struct Reading: View {
        @Environment(\.toolbarVerticalEdge) private var verticalEdge
        let content: (Bool) -> Content

        var body: some View {
            content(verticalEdge != nil)
        }
    }
}

struct EntryRow: View {
    let item: FeedItem
    /// Dims the title, the way the official app tells opened entries apart.
    let isRead: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 8) {
                    if let host = item.link.host() {
                        HStack(spacing: 4) {
                            FaviconView(pageURL: item.link)
                            Text(host).lineLimit(1)
                        }
                    }
                    Spacer(minLength: 0)
                    Text("\(item.bookmarkCount) users")
                        .foregroundStyle(.red)
                        .bold()
                        .fixedSize()
                }
                .font(.caption)
                .foregroundStyle(.secondary)
                Text(item.title)
                    .font(.body)
                    .foregroundStyle(isRead ? .secondary : .primary)
                    .lineLimit(3)
                    // Drawn below the site line but read out first.
                    .accessibilitySortPriority(1)
            }
            Spacer(minLength: 0)
            EntryThumbnail(url: item.imageURL)
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityValue(isRead ? Text("既読") : Text(""))
    }
}

/// The square image at the end of an entry row. Every row keeps the slot, so
/// titles line up whether or not the page has an `og:image`: when there is
/// none, or it fails to load, the slot shows a globe on a faint gray, the same
/// glyph `FaviconView` falls back to.
struct EntryThumbnail: View {
    let url: URL?

    private static let size: CGFloat = 64
    private static let shape = RoundedRectangle(cornerRadius: 6)

    var body: some View {
        Group {
            if let url {
                AsyncImage(url: url) { phase in
                    switch phase {
                    case .success(let image):
                        image.resizable().scaledToFill()
                    case .failure:
                        fallback
                    case .empty:
                        Self.background
                    @unknown default:
                        fallback
                    }
                }
            } else {
                fallback
            }
        }
        .frame(width: Self.size, height: Self.size)
        .clipShape(Self.shape)
        .accessibilityHidden(true)
    }

    private static var background: some View {
        Color.secondary.opacity(0.1)
    }

    private var fallback: some View {
        Self.background.overlay {
            Image(systemName: "globe")
                .font(.system(size: 24, weight: .light))
                .foregroundStyle(.quaternary)
        }
    }
}
