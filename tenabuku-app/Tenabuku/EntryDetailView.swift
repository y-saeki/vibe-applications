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
    let tab: Tab
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
    /// Bumped by every `load`, so that a page arriving for an earlier pull is
    /// dropped.
    private var generation = 0

    var hasMore: Bool { nextCursor != nil }

    init(url: URL, tab: Tab) {
        self.url = url
        self.tab = tab
    }

    /// Loads the comments the first time the tab shows, and not again when
    /// it comes back into view.
    func loadIfNeeded(webSession: WebSessionStore) async {
        guard generation == 0 else { return }
        await load(webSession: webSession)
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
    @State private var tab: EntryCommentsModel.Tab = .popular
    /// The page in view, which follows `tab` and sets it when swiped.
    @State private var page: EntryCommentsModel.Tab? = .popular
    /// One per tab, so that each keeps its comments and where it was
    /// scrolled to while the other shows.
    @State private var popular: EntryCommentsModel
    @State private var recent: EntryCommentsModel
    @State private var isShowingBookmarkEditor = false
    /// The comment a star was meant for when Hatena asked for the website
    /// login; the star is sent once the login is done.
    @State private var pendingStar: PendingStar?
    @State private var isShowingWebSignIn = false
    @State private var starError: Error?
    /// Whether the sheet is wider than it is tall, which with a vertical bar
    /// tells iPhone Duo's inner display held sideways from its outer display.
    @State private var isWide = false
    /// How far the bars reach over the comments, the vertical bar included.
    @State private var barInsets = EdgeInsets()
    @Environment(\.openURL) private var openURL
    @Environment(\.dismiss) private var dismiss
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    /// How far below the tabs' centre the middle of the count's figures sits,
    /// so that its baseline meets the foot of the labels' ideographs.
    @ScaledMetric(relativeTo: .headline) private var countDrop: CGFloat = 1
    @Environment(WebSessionStore.self) private var webSession

    private struct PendingStar {
        let comment: CommentRowData
        let model: EntryCommentsModel
    }

    init(entry: FeedItem) {
        self.entry = entry
        _popular = State(initialValue: EntryCommentsModel(url: entry.link, tab: .popular))
        _recent = State(initialValue: EntryCommentsModel(url: entry.link, tab: .recent))
    }

    private func model(for tab: EntryCommentsModel.Tab) -> EntryCommentsModel {
        switch tab {
        case .popular: popular
        case .recent: recent
        }
    }

    var body: some View {
        // Pages side by side, so that a sideways swipe slides the other tab
        // in under the finger and choosing a tab slides it in the same way.
        ScrollView(.horizontal) {
            HStack(spacing: 0) {
                ForEach(EntryCommentsModel.Tab.allCases) { tab in
                    let model = model(for: tab)
                    CommentList(model: model, barInsets: barInsets) { comment in
                        Task { await addStar(to: comment, in: model) }
                    }
                    .containerRelativeFrame(.horizontal)
                    .id(tab)
                }
            }
            .scrollTargetLayout()
        }
        .scrollTargetBehavior(.paging)
        // The pages' own edge, which the header sits on, softened as well.
        .softTopEdge()
        .scrollPosition(id: $page)
        .scrollIndicators(.hidden)
        .scrollBounceBehavior(.basedOnSize, axes: .horizontal)
        // Under the bars, so that the comments run on behind their glass
        // rather than stopping short of it, and each page is as wide as the
        // sheet, so that none of the next one shows beside the vertical bar.
        // The pages do not hand the bars' room on to the lists, so the lists
        // are told it themselves.
        .ignoresSafeArea(.container)
        .background {
            Color.clear.onGeometryChange(for: EdgeInsets.self, of: \.safeAreaInsets) { barInsets = $0 }
        }
        .onChange(of: tab) { _, tab in
            guard page != tab else { return }
            withAnimation { page = tab }
        }
        .onChange(of: page) { _, page in
            if let page, page != tab { tab = page }
        }
        .onGeometryChange(for: Bool.self) { $0.size.width > $0.size.height } action: { isWide = $0 }
        // The count and the tabs sit in a row of their own at the top, in
        // the text size of a bar title and the plain segmented control, rather
        // than in the navigation bar, which draws its items larger and in
        // glass. The navigation bar would only leave an empty row above them.
        .topBar { header }
        .toolbar(.hidden, for: .navigationBar)
        // The buttons stay at the bottom even on iPhone Duo's inner display
        // held sideways, where the system would put them in a vertical bar.
        // On its outer display they go to the vertical bar.
        .verticalBarDisabled(isWide)
        .toolbar {
            ToolbarItem(placement: .bottomBar) {
                Button("閉じる", systemImage: "xmark") { dismiss() }
            }
            if #available(iOS 26.0, *) {
                ToolbarSpacer(.flexible, placement: .bottomBar)
            } else {
                ToolbarItem(placement: .bottomBar) { Spacer() }
            }
            ToolbarItemGroup(placement: .bottomBar) {
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
            if let pendingStar {
                Task { await addStar(to: pendingStar.comment, in: pendingStar.model) }
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
    }

    /// The cap height of the headline font at the given text size, which
    /// the figures of the count reach.
    private static func headlineCapHeight(for size: DynamicTypeSize) -> CGFloat {
        let traits = UITraitCollection(preferredContentSizeCategory: UIContentSizeCategory(size))
        return UIFont.preferredFont(forTextStyle: .headline, compatibleWith: traits).capHeight
    }

    /// Two segments of 88 points, on the 8-point grid.
    private static let tabsWidth: CGFloat = 176

    /// The bookmark count, and the tabs beside it.
    private var header: some View {
        HStack(alignment: .opticalCenter) {
            Text("\(entry.bookmarkCount) users")
                .font(.headline)
                .accessibilityAddTraits(.isHeader)
                // Placed by the glyphs it shows rather than by its line box,
                // which keeps room below for descenders the count never has.
                // Its baseline then sits on the foot of the tab labels'
                // ideographs, which reach a little below a Latin baseline:
                // centred on the box, the count's foot stood about a point
                // above theirs and the count looked high.
                .alignmentGuide(.opticalCenter) { d in
                    d[.firstTextBaseline] - Self.headlineCapHeight(for: dynamicTypeSize) / 2 - countDrop
                }
            Spacer()
            Picker("並び順", selection: $tab) {
                ForEach(EntryCommentsModel.Tab.allCases) { tab in
                    Text(tab.rawValue).tag(tab)
                }
            }
            .pickerStyle(.segmented)
            // Wider than the labels need, at the control's own height, so
            // that each segment is twice the 44-point minimum target Apple
            // asks for and easy to hit.
            .frame(width: Self.tabsWidth)
        }
        // On the 8-point grid, with the same inset as the comments, and
        // room enough under the sheet's grabber that the row does not crowd it.
        .padding(.horizontal, 16)
        .padding(.top, 20)
        .padding(.bottom, 12)
    }

    private func addStar(to comment: CommentRowData, in model: EntryCommentsModel) async {
        do {
            try await model.addStar(to: comment, webSession: webSession)
        } catch HatenaError.unauthorized {
            pendingStar = PendingStar(comment: comment, model: model)
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

/// The comments of one tab.
private struct CommentList: View {
    let model: EntryCommentsModel
    /// How far the bars reach over the list, which runs on under them.
    let barInsets: EdgeInsets
    let addStar: (CommentRowData) -> Void
    @Environment(WebSessionStore.self) private var webSession

    /// Room below the last comment, so that it ends clear of the bottom bar.
    private static let endPadding: CGFloat = 16

    /// The same space above and below each comment, kept small so that more
    /// of them fit on the screen.
    private static let rowInsets = EdgeInsets(top: 16, leading: 16, bottom: 16, trailing: 16)

    /// The row that starts loading the next page: some way before the end.
    private var prefetchMark: String? {
        let comments = model.comments
        return comments.isEmpty ? nil : comments[max(0, comments.count - 20)].id
    }

    var body: some View {
        List {
            Section {
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
                    addStar: comment.starURI == nil ? nil : { addStar(comment) }
                )
                .listRowInsets(Self.rowInsets)
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
          // No rule above the first comment: the list starts under the header,
          // which needs no line to set it apart.
          .listSectionSeparator(.hidden, edges: .top)
          .listRowBackground(Color.clear)
        }
        .listStyle(.plain)
        // Clear, so that the sheet's glass shows through.
        .scrollContentBackground(.hidden)
        // A soft edge under the header, like the system's own bars, rather
        // than a rule across the sheet.
        .softTopEdge()
        .contentMargins(.bottom, Self.endPadding, for: .scrollContent)
        // As safe area rather than margins, so that the list also blurs what
        // runs under the bars, as it would where the bars reach it themselves.
        .safeAreaPadding(barInsets)
        .refreshable { await model.load(webSession: webSession, refreshIgnored: true) }
        .task { await model.loadIfNeeded(webSession: webSession) }
    }
}

struct CommentRow: View {
    let comment: CommentRowData
    /// Nil until the count is read, or when it could not be.
    var stars: StarCount?
    var isAddingStar = false
    /// Nil when the comment cannot take a star.
    var addStar: (() -> Void)?
    /// Leaves the ID out and puts a small icon, the date and the stars on a
    /// line above the comment, for a list of the user's own comments, where
    /// the ID says nothing and the comment can take the row's full width.
    var isOwn = false

    /// As tall as a line of the comment at any text size.
    @ScaledMetric(relativeTo: .subheadline) private var ownIconSide: CGFloat = 20

    var body: some View {
        if isOwn {
            ownBody
        } else {
            sheetBody
        }
    }

    /// The icon, the date and the stars on one line, and the comment under
    /// them at the date's indent, running the full width of the row.
    private var ownBody: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                icon(side: ownIconSide)
                // On the text baseline, so that the stars sit on the line
                // with the date instead of hanging below it.
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    if let date = comment.date {
                        Text(HatenaDate.timestamp(date))
                            .font(.caption2.monospaced())
                            .foregroundStyle(.secondary)
                    }
                    if (stars?.total ?? 0) > 0 {
                        starRow
                    }
                }
            }
            if !comment.comment.isEmpty || !comment.tags.isEmpty {
                commentText
                    .font(.subheadline)
                    .padding(.leading, ownIconSide + 8)
            }
        }
    }

    private func icon(side: CGFloat) -> some View {
        AsyncImage(url: comment.iconURL) { image in
            image.resizable().scaledToFill()
        } placeholder: {
            Image(systemName: "person.crop.circle.fill").resizable().foregroundStyle(.tertiary)
        }
        .frame(width: side, height: side)
        .clipShape(Circle())
    }

    private var sheetBody: some View {
        HStack(alignment: .top, spacing: 10) {
            icon(side: 32)

            VStack(alignment: .leading, spacing: 8) {
                // The user, the comment and the tags run on as one paragraph,
                // so that a short comment takes a single line.
                commentText.font(.subheadline)
                // On the text baseline, so that the stars sit on the line
                // with the date instead of hanging below it.
                HStack(alignment: .firstTextBaseline, spacing: 12) {
                    if let date = comment.date {
                        Text(HatenaDate.timestamp(date))
                            .font(.caption2.monospaced())
                            .foregroundStyle(.secondary)
                    }
                    if addStar != nil || (stars?.total ?? 0) > 0 {
                        starRow
                    }
                }
            }
        }
    }

    private var commentText: Text {
        var parts: [Text] = []
        if !isOwn { parts.append(Text(comment.user).bold()) }
        if !comment.comment.isEmpty { parts.append(Text(comment.comment)) }
        if !comment.tags.isEmpty {
            parts.append(
                Text(comment.tags.joined(separator: " "))
                    .font(.caption)
                    .foregroundStyle(.secondary)
            )
        }
        return parts.dropFirst().reduce(parts.first ?? Text(verbatim: "")) { Text("\($0)  \($1)") }
    }

    /// The coloured stars, then the yellow ones as the button that adds one,
    /// as Hatena shows them. Without the button, the yellow ones are only
    /// counted.
    private var starRow: some View {
        HStack(alignment: .firstTextBaseline, spacing: 10) {
            ForEach(stars?.colored ?? [], id: \.color) { colored in
                starCount(colored.count, color: Self.color(named: colored.color))
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("\(Self.colorName(colored.color))のスター \(colored.count) 個")
            }
            if let addStar {
                Button(action: addStar) {
                    starCount(stars?.yellow, color: Self.yellowStarColor)
                        .opacity(isAddingStar ? 0.4 : 1)
                        // A target large enough to hit without hitting the row,
                        // reaching past the line so that it adds no height.
                        .frame(minWidth: 44, alignment: .leading)
                        .padding(.vertical, 8)
                        .contentShape(Rectangle())
                        .padding(.vertical, -8)
                }
                // Only the star reacts to a tap, not the whole row.
                .buttonStyle(.borderless)
                .disabled(isAddingStar)
                .accessibilityLabel("スターを付ける")
                .accessibilityValue(stars.map { "\($0.yellow) 個" } ?? "")
            } else if let yellow = stars?.yellow, yellow > 0 {
                // Only the count where the row cannot add a star.
                starCount(yellow, color: Self.yellowStarColor)
                    .accessibilityElement(children: .ignore)
                    .accessibilityLabel("スター \(yellow) 個")
            }
        }
        .font(.caption)
    }

    /// A star and how many there are, both in the star's colour; grey while
    /// nil or none, so that the first star added turns them to its colour.
    private func starCount(_ count: Int?, color: Color) -> some View {
        let tint = (count ?? 0) > 0 ? color : Self.noStarColor
        return HStack(alignment: .firstTextBaseline, spacing: 3) {
            Image(systemName: "star.fill")
            if let count {
                Text(count, format: .number)
                    .monospaced()
                    .bold()
                    .contentTransition(.numericText(value: Double(count)))
            }
        }
        // Explicit, so that the button's tint does not take over.
        .foregroundStyle(tint)
        .animation(.snappy, value: count)
    }

    /// The star of a comment with none yet: filled like the others, in a grey
    /// that reads as not yet given.
    private static let noStarColor = Color(uiColor: .systemGray)

    // The colours of Hatena's own star artwork (the Hatena Star logo on
    // https://www.hatena.ne.jp/shop/star), so the stars look as they do there.
    private static let yellowStarColor = hatenaColor(0xFFB000)

    private static func hatenaColor(_ rgb: UInt32) -> Color {
        Color(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }

    private static func color(named name: String) -> Color {
        switch name {
        case "green": hatenaColor(0x00D700)
        case "red": hatenaColor(0xFF0033)
        case "blue": hatenaColor(0x00A2E3)
        case "purple": hatenaColor(0xC500C6)
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

private extension View {
    /// Puts `content` in a bar along the top of the view. From iOS 26 it is a
    /// safe area bar, so that what scrolls under it fades the way it does
    /// under the system's own bars, with no fill of its own; before that, it
    /// sits on the bar material.
    @ViewBuilder
    func topBar(@ViewBuilder _ content: () -> some View) -> some View {
        if #available(iOS 26.0, *) {
            safeAreaBar(edge: .top, spacing: 0, content: content)
        } else {
            safeAreaInset(edge: .top, spacing: 0) { content().background(.bar) }
        }
    }
}

private extension View {
    @ViewBuilder
    func softTopEdge() -> some View {
        if #available(iOS 26.0, *) {
            scrollEdgeEffectStyle(.soft, for: .top)
        } else {
            self
        }
    }
}

private extension VerticalAlignment {
    /// The middle of what a view shows: its frame's centre unless the view
    /// says otherwise.
    private enum OpticalCenter: AlignmentID {
        static func defaultValue(in d: ViewDimensions) -> CGFloat { d[VerticalAlignment.center] }
    }

    static let opticalCenter = VerticalAlignment(OpticalCenter.self)
}
