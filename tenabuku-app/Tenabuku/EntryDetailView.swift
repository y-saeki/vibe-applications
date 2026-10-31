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
        case popular = "人気"
        case recent = "新着"

        var id: Self { self }
    }

    let url: URL
    var tab: Tab = .popular
    private(set) var comments: [CommentRowData] = []
    private(set) var error: Error?
    private(set) var isLoading = false

    init(url: URL) {
        self.url = url
    }

    func load() async {
        isLoading = true
        defer { isLoading = false }
        do {
            switch tab {
            case .popular:
                comments = try await popular()
            case .recent:
                comments = try await recent()
            }
            error = nil
        } catch {
            FailureLog.record(error)
            comments = []
            self.error = error
        }
    }

    // The ranking only exists in the website's undocumented JSON.
    private func popular() async throws -> [CommentRowData] {
        try await AppServices.entryComments.comments(for: url, sort: .popular).comments.map {
            CommentRowData(user: $0.user, iconURL: $0.iconURL, comment: $0.comment, tags: $0.tags, date: $0.date)
        }
    }

    // Newest first through the documented API, leaving out bookmarks with no comment.
    private func recent() async throws -> [CommentRowData] {
        let entry = try await AppServices.entryInfo.entry(for: url)
        return (entry?.bookmarks ?? []).filter { !$0.comment.isEmpty }.map {
            CommentRowData(user: $0.user, iconURL: nil, comment: $0.comment, tags: $0.tags, date: $0.date)
        }
    }
}

struct EntryDetailView: View {
    let entry: FeedItem
    @State private var model: EntryCommentsModel
    @Environment(\.openURL) private var openURL

    init(entry: FeedItem) {
        self.entry = entry
        _model = State(initialValue: EntryCommentsModel(url: entry.link))
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
                }
            }
        }
        .listStyle(.plain)
        .navigationTitle("\(entry.bookmarkCount) users")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("Safari で開く", systemImage: "safari") {
                    openURL(entry.link)
                }
            }
        }
        .refreshable { await model.load() }
        .task(id: model.tab) { await model.load() }
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
