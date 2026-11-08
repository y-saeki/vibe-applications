import HatenaFeed
import HomeShelves
import SwiftUI

/// The home screen: one shelf per category, stacked, each paging sideways
/// through its entries a few at a time, the way App Store shows its lists.
struct HomeView: View {
    @State private var feeds = ShelfFeedStore()
    @State private var isShowingSettings = false
    @State private var isEditingShelves = false
    @State private var bookmarking: FeedItem?
    @State private var muting: MuteSource?
    @State private var path = NavigationPath()
    @Environment(ShelfSettingsStore.self) private var shelfSettings
    @Environment(WebSessionStore.self) private var webSession

    var body: some View {
        VerticalBarReader { barsMayBeVertical in
            home(barsMayBeVertical: barsMayBeVertical)
        }
    }

    private func home(barsMayBeVertical: Bool) -> some View {
        NavigationStack(path: $path) {
            ScrollView {
                LazyVStack(alignment: .leading, spacing: 28) {
                    ForEach(shelfSettings.shownShelves, id: \.self) { shelf in
                        EntryShelf(
                            shelf: shelf,
                            feed: feeds[shelf],
                            retry: { Task { await feeds.load(shelf, webSession: webSession) } },
                            bookmark: { bookmarking = $0 },
                            mute: { muting = MuteSource(title: $0.title, url: $0.link) },
                            openFromPreview: openFromPreview
                        )
                        .task { await feeds.loadIfNeeded(shelf, webSession: webSession) }
                    }
                }
                .padding(.vertical, 12)
            }
            .overlay {
                if shelfSettings.shownShelves.isEmpty {
                    ContentUnavailableView {
                        Label("表示する棚がありません", systemImage: "square.stack")
                    } actions: {
                        Button("棚を編集") { isEditingShelves = true }
                    }
                }
            }
            .navigationTitle("ホットエントリー")
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
                        editShelvesButton
                        Spacer()
                    }
                } else {
                    ToolbarItem(placement: .topBarLeading) {
                        settingsButton
                    }
                    ToolbarItem(placement: .topBarTrailing) {
                        editShelvesButton
                    }
                }
                ToolbarItem(placement: .topBarTrailing) {
                    // A value, like the rows, so that the stack resolves every push the same way.
                    NavigationLink(value: MyBookmarkListRoute()) {
                        Label("マイブックマーク", systemImage: "books.vertical")
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
            .sheet(isPresented: $isEditingShelves) {
                NavigationStack {
                    ShelfEditorView()
                }
            }
            .refreshable { await feeds.reloadLoaded(webSession: webSession) }
            // Signing in from the prompt, or from Settings, brings My hot entries in.
            .onChange(of: webSession.isSignedIn) { _, isSignedIn in
                if isSignedIn, feeds[.myHotEntry].needsWebSignIn {
                    Task { await feeds.load(.myHotEntry, webSession: webSession) }
                }
            }
        }
        // On the stack, so that the screens it pushes, such as My bookmarks, have it too.
        .environment(\.openEntryFromPreview, OpenEntryFromPreviewAction(openFromPreview))
    }

    /// Opens the article beneath a preview that is already filling the screen.
    private func openFromPreview(_ item: FeedItem) {
        var transaction = Transaction()
        transaction.disablesAnimations = true
        withTransaction(transaction) { path.append(item) }
    }

    private var settingsButton: some View {
        Button("設定", systemImage: "gearshape") { isShowingSettings = true }
    }

    private var editShelvesButton: some View {
        Button("棚を編集", systemImage: "slider.horizontal.3") { isEditingShelves = true }
    }
}

/// One shelf: the category's name, then its entries in pages of
/// `rowsPerPage`, side by side. Each page is a little narrower than the
/// screen, so the edge of the next one shows and tells that the shelf goes
/// on sideways.
private struct EntryShelf: View {
    let shelf: HomeShelf
    let feed: ShelfFeed
    let retry: () -> Void
    let bookmark: (FeedItem) -> Void
    let mute: (FeedItem) -> Void
    let openFromPreview: (FeedItem) -> Void

    @Environment(ReadHistoryStore.self) private var readHistory
    @Environment(MuteStore.self) private var mutes
    @Environment(FaviconSettingsStore.self) private var faviconSettings

    static let rowsPerPage = 3
    private static let pageWidth = 0.88
    private static let margin: CGFloat = 16

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            header
            content
        }
    }

    private var header: some View {
        HStack(alignment: .firstTextBaseline) {
            Text(shelf.displayName)
                .font(.title3.bold())
                .accessibilityAddTraits(.isHeader)
            Spacer()
            // So that a shelf emptied by the user's own rules is not taken
            // for a feed that came back empty.
            if case let hidden = feed.items.count - visibleItems.count, hidden > 0 {
                Text("ミュート \(hidden) 件")
                    .font(.footnote)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(.horizontal, Self.margin)
    }

    @ViewBuilder
    private var content: some View {
        if feed.needsWebSignIn {
            WebSignInPrompt()
        } else if let error = feed.error, feed.items.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                ErrorBanner(error: error)
                Button("もう一度読み込む", action: retry)
                    .buttonStyle(.bordered)
            }
            .padding(.horizontal, Self.margin)
            .padding(.top, 8)
        } else if !feed.hasLoaded {
            ProgressView()
                .frame(maxWidth: .infinity, minHeight: 160)
        } else if visibleItems.isEmpty {
            Text(shelf == .myHotEntry
                ? "記事がありません。お気に入りのユーザーと関心ワードは、はてなの Web で設定します。"
                : "記事がありません")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .padding(.horizontal, Self.margin)
                .padding(.top, 8)
        } else {
            if let error = feed.error {
                // A reload that failed leaves the entries read before.
                ErrorBanner(error: error)
                    .padding(.horizontal, Self.margin)
            }
            pages
        }
    }

    private var pages: some View {
        let items = visibleItems
        let pageStarts = Array(stride(from: 0, to: items.count, by: Self.rowsPerPage))
        return ScrollView(.horizontal) {
            LazyHStack(alignment: .top, spacing: 12) {
                ForEach(pageStarts, id: \.self) { start in
                    page(items[start..<min(start + Self.rowsPerPage, items.count)])
                        // The length already leaves out the content margins.
                        .containerRelativeFrame(.horizontal) { length, _ in length * Self.pageWidth }
                        .onAppear { prefetchImages(after: min(start + Self.rowsPerPage, items.count) - 1) }
                }
            }
            .scrollTargetLayout()
        }
        .scrollTargetBehavior(.viewAligned)
        .contentMargins(.horizontal, Self.margin, for: .scrollContent)
        .scrollIndicators(.hidden)
    }

    private func page(_ items: ArraySlice<FeedItem>) -> some View {
        VStack(spacing: 0) {
            ForEach(Array(items.enumerated()), id: \.element.id) { offset, item in
                if offset > 0 {
                    Divider()
                }
                row(item)
            }
        }
    }

    private func row(_ item: FeedItem) -> some View {
        NavigationLink(value: item) {
            EntryRow(item: item, isRead: readHistory.isRead(item.link), reservesTitleSpace: true)
                .padding(.vertical, 6)
                .contentShape(Rectangle())
                .pagePreviewMenu(
                    url: item.link,
                    title: item.title,
                    actions: [
                        PagePreviewAction(title: "ブクマする", systemImage: "bookmark") { bookmark(item) },
                        PagePreviewAction(title: "ミュート", systemImage: "eye.slash") { mute(item) },
                    ],
                    open: { openFromPreview(item) }
                )
        }
        .buttonStyle(.plain)
    }

    private var visibleItems: [FeedItem] {
        feed.items.filter { !mutes.isMuted(title: $0.title, url: $0.link) }
    }

    /// Reads ahead the icons and images of the pages after the one that
    /// came into view.
    private func prefetchImages(after index: Int) {
        ImagePrefetch.rows(
            visibleItems, after: index,
            page: \.link, thumbnail: \.imageURL,
            faviconSources: faviconSettings.enabledSources
        )
    }
}
