import HatenaFeed
import SwiftUI

@MainActor
@Observable
final class HotEntryListModel {
    var category: HotEntryCategory = .all
    private(set) var items: [FeedItem] = []
    private(set) var error: Error?
    private(set) var isLoading = false

    func load() async {
        isLoading = true
        defer { isLoading = false }
        do {
            items = try await AppServices.feed.hotEntries(category)
            error = nil
        } catch {
            FailureLog.record(error)
            self.error = error
        }
    }
}

struct HotEntryListView: View {
    @State private var model = HotEntryListModel()
    @State private var isShowingSettings = false
    @State private var bookmarking: FeedItem?
    @Environment(ReadHistoryStore.self) private var readHistory
    @Environment(MuteStore.self) private var mutes

    var body: some View {
        VerticalBarReader { barsMayBeVertical in
            list(barsMayBeVertical: barsMayBeVertical)
        }
    }

    private func list(barsMayBeVertical: Bool) -> some View {
        let visible = model.items.filter { !mutes.isMuted($0) }
        let mutedCount = model.items.count - visible.count
        return NavigationStack {
            List {
                if let error = model.error {
                    ErrorBanner(error: error)
                }
                ForEach(visible) { item in
                    NavigationLink(value: item) {
                        EntryRow(item: item, isRead: readHistory.isRead(item.link))
                    }
                    .swipeActions(edge: .leading) {
                        Button("ブクマする", systemImage: "bookmark") { bookmarking = item }
                            .tint(.blue)
                    }
                }
                if mutedCount > 0 {
                    // Said at the foot, so that a short or empty list is not
                    // mistaken for a feed that failed to load.
                    Text("ミュートで \(mutedCount) 件を隠しています")
                        .font(.footnote)
                        .foregroundStyle(.secondary)
                        .frame(maxWidth: .infinity)
                        .listRowSeparator(.hidden)
                }
            }
            .listStyle(.plain)
            .navigationTitle(model.category.displayName)
            .navigationDestination(for: FeedItem.self) { item in
                ArticleBrowserView(entry: item)
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
                    Menu {
                        Picker("カテゴリー", selection: $model.category) {
                            ForEach(HotEntryCategory.allCases) { category in
                                Text(category.displayName).tag(category)
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
            .refreshable { await model.load() }
            .task(id: model.category) { await model.load() }
        }
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
                Text(item.title)
                    .font(.body)
                    .foregroundStyle(isRead ? .secondary : .primary)
                    .lineLimit(3)
                HStack(spacing: 8) {
                    Text("\(item.bookmarkCount) users")
                        .foregroundStyle(.red)
                        .bold()
                    if let host = item.link.host() {
                        Text(host).lineLimit(1)
                    }
                    if let date = item.date {
                        Text(date, style: .relative)
                    }
                }
                .font(.caption)
                .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
            if let imageURL = item.imageURL {
                AsyncImage(url: imageURL) { image in
                    image.resizable().scaledToFill()
                } placeholder: {
                    Color.secondary.opacity(0.1)
                }
                .frame(width: 64, height: 64)
                .clipShape(RoundedRectangle(cornerRadius: 6))
            }
        }
        .padding(.vertical, 4)
        .accessibilityElement(children: .combine)
        .accessibilityValue(isRead ? Text("既読") : Text(""))
    }
}
