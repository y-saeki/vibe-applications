import HatenaFeed
import HomeShelves
import SwiftUI

/// The home screen: one shelf per category, stacked, each paging sideways
/// through its entries a few at a time, the way App Store shows its lists.
struct HomeView: View {
    @State private var feeds = ShelfFeedStore()
    @State private var isShowingSettings = false
    @State private var bookmarking: FeedItem?
    @State private var muting: MuteSource?
    @State private var path = NavigationPath()
    @Environment(ShelfSettingsStore.self) private var shelfSettings
    @Environment(WebSessionStore.self) private var webSession
    @Environment(AccountStore.self) private var accounts
    @AppStorage(AccountIconSetting.key) private var showsAccountIcon = AccountIconSetting.defaultValue

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
                        Button("設定を開く") { isShowingSettings = true }
                    }
                }
            }
            .navigationTitle("ホットエントリー")
            // In the bar's own row, so that no empty row sits above the title
            // and the settings stand level with it.
            .toolbarTitleDisplayMode(.inlineLarge)
            .navigationDestination(for: FeedItem.self) { item in
                ArticleBrowserView(entry: item)
            }
            .navigationDestination(for: MyBookmarkListRoute.self) { _ in
                MyBookmarkListView()
            }
            .toolbar {
                // The settings stay at the top on the right, even where the
                // other buttons move to a vertical bar at the side.
                ToolbarItem(placement: .topBarTrailing) {
                    settingsButton
                }
                .horizontalBarOnly()
                myBookmarksItem(barsMayBeVertical: barsMayBeVertical)
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

    /// At the top of a vertical bar, and otherwise in the middle of the
    /// bottom bar, where a thumb reaches it.
    @ToolbarContentBuilder
    private func myBookmarksItem(barsMayBeVertical: Bool) -> some ToolbarContent {
        if barsMayBeVertical {
            ToolbarItem(placement: .topBarTrailing) {
                myBookmarksLink
            }
        } else {
            ToolbarItemGroup(placement: .bottomBar) {
                Spacer()
                myBookmarksLink
                Spacer()
            }
        }
    }

    private var myBookmarksLink: some View {
        // A value, like the rows, so that the stack resolves every push the same way.
        NavigationLink(value: MyBookmarkListRoute()) {
            // Not a Label: the toolbar takes a label's icon out
            // as a bare image, dropping the circle it is cut to.
            AccountIcon(url: showsAccountIcon ? accounts.account?.iconURL : nil)
                .accessibilityLabel("マイブックマーク")
        }
    }

    private var settingsButton: some View {
        Button("設定", systemImage: "gearshape") { isShowingSettings = true }
    }
}

/// Whether the My bookmarks button, and the user's own comments in My
/// bookmarks, show the signed-in user's own picture, set in Settings for
/// those who would rather not see it on every screen.
enum AccountIconSetting {
    static let key = "showsAccountIconOnMyBookmarks"
    static let defaultValue = true
}

/// The signed-in user's profile image, or a figure of a person when nobody
/// is signed in, the image cannot be read or Settings hides it.
private struct AccountIcon: View {
    let url: URL?

    /// The picture sits inside the bar's glass with a rim of it showing, as
    /// the App Store's account button does.
    private static let diameter: CGFloat = 36
    /// The bar pads a custom view more at the sides than above and below,
    /// which stretches its glass into a capsule around a picture this size.
    /// Taking that much back off the sides keeps the glass round.
    private static let sideInset: CGFloat = 6

    var body: some View {
        if let url {
            AsyncImage(url: url) { phase in
                if let image = phase.image {
                    // Drawn over a sized shape rather than sized itself, so
                    // that the bar cannot treat it as a symbol and lay it out
                    // at the image's own size.
                    Circle()
                        .fill(.clear)
                        .overlay { image.resizable().scaledToFill() }
                        .clipShape(Circle())
                } else {
                    placeholder
                }
            }
            .frame(width: Self.diameter, height: Self.diameter)
            // A new account loads its own image rather than showing the last one's.
            .id(url)
            .padding(.horizontal, -Self.sideInset)
        } else {
            placeholder
        }
    }

    private var placeholder: some View {
        Image(systemName: "person.crop.circle")
    }
}

/// One shelf: the category's name, then its entries in pages of
/// `rowsPerPage`, side by side. The pages on screen leave a little of the
/// width over, so the edge of the next one shows and tells that the shelf
/// goes on sideways.
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
    @Environment(\.horizontalSizeClass) private var horizontalSizeClass

    static let rowsPerPage = 3
    /// The share of the width the pages on screen take together, gaps
    /// between them included; the rest shows the next page's edge.
    private static let pagesShare = 0.88
    private static let pageSpacing: CGFloat = 12
    private static let margin: CGFloat = 16

    /// One page on a compact width, as App Store shows its lists on iPhone.
    /// On a regular one, such as iPhone Duo's inner display, two, rather
    /// than one stretched across: an even count, so that a fold down the
    /// middle falls between them.
    private var pagesOnScreen: Int {
        horizontalSizeClass == .regular ? 2 : 1
    }

    private func pageWidth(in length: CGFloat) -> CGFloat {
        let count = CGFloat(pagesOnScreen)
        return (length * Self.pagesShare + Self.pageSpacing) / count - Self.pageSpacing
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            header
            content
        }
    }

    private var header: some View {
        Text(shelf.displayName)
            .font(.title3.bold())
            .accessibilityAddTraits(.isHeader)
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
            LazyHStack(alignment: .top, spacing: Self.pageSpacing) {
                ForEach(pageStarts, id: \.self) { start in
                    page(items[start..<min(start + Self.rowsPerPage, items.count)])
                        // The length already leaves out the content margins.
                        .containerRelativeFrame(.horizontal) { length, _ in pageWidth(in: length) }
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
        let items = visibleItems
        ImagePrefetch.rows(
            items, after: index,
            page: \.link, thumbnail: \.imageURL,
            faviconSources: faviconSettings.enabledSources
        )
        // Entries the feed had no image for read it from the page.
        let start = index + 1
        guard start < items.count else { return }
        let ahead = items[start..<min(start + ImagePrefetch.lookahead, items.count)]
        EntryImageLookup.shared.prefetch(ahead.filter { $0.imageURL == nil }.map(\.link), asksHatena: false)
    }
}
