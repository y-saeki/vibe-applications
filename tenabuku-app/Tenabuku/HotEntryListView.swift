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

    var body: some View {
        NavigationStack {
            List {
                if let error = model.error {
                    ErrorBanner(error: error)
                }
                ForEach(model.items) { item in
                    NavigationLink(value: item) {
                        EntryRow(item: item)
                    }
                }
            }
            .listStyle(.plain)
            .navigationTitle(model.category.displayName)
            .navigationDestination(for: FeedItem.self) { item in
                ArticleBrowserView(entry: item)
            }
            .toolbar {
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
            .overlay {
                if model.isLoading && model.items.isEmpty {
                    ProgressView()
                }
            }
            .refreshable { await model.load() }
            .task(id: model.category) { await model.load() }
        }
    }
}

struct EntryRow: View {
    let item: FeedItem

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            VStack(alignment: .leading, spacing: 6) {
                Text(item.title)
                    .font(.body)
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
    }
}
