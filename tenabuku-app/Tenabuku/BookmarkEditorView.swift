import HatenaOfficial
import SwiftUI

@MainActor
@Observable
final class BookmarkEditorModel {
    enum Phase {
        case loading
        case ready
        case failed(Error)
    }

    let url: URL
    var draft: BookmarkDraft
    var tagInput = ""
    private(set) var phase = Phase.loading
    /// The bookmark already saved for `url`, which can then be deleted.
    private(set) var saved: MyBookmark?
    private(set) var userTags: [UserTag] = []
    private(set) var isWorking = false
    private(set) var actionError: Error?

    init(url: URL) {
        self.url = url
        draft = BookmarkDraft(url: url)
    }

    var suggestions: [UserTag] {
        UserTag.suggestions(from: userTags, matching: tagInput, excluding: draft.tags)
    }

    func load(api: BookmarkAPI, accounts: AccountStore) async {
        phase = .loading
        do {
            // The tags only help with typing, so their failure does not stop editing.
            async let tags = try? api.tags()
            saved = try await api.bookmark(for: url)
            if let saved {
                draft = BookmarkDraft(url: url, editing: saved)
            }
            userTags = await tags ?? []
            phase = .ready
        } catch {
            accounts.record(error)
            phase = .failed(error)
        }
    }

    func addTag(_ tag: String) {
        if draft.addTag(tag) {
            tagInput = ""
        }
    }

    /// Returns whether it saved, so that the editor can close.
    func save(api: BookmarkAPI, accounts: AccountStore) async -> Bool {
        // Text left in the field is a tag the user meant to add.
        addTag(tagInput)
        return await perform(accounts) { [draft] in try await api.save(draft) }
    }

    func delete(api: BookmarkAPI, accounts: AccountStore) async -> Bool {
        await perform(accounts) { [url] in try await api.delete(url) }
    }

    private func perform(_ accounts: AccountStore, _ action: () async throws -> Void) async -> Bool {
        isWorking = true
        defer { isWorking = false }
        do {
            try await action()
            actionError = nil
            return true
        } catch {
            accounts.record(error)
            actionError = error
            return false
        }
    }
}

/// Adds, edits or deletes the signed-in user's bookmark of one page.
struct BookmarkEditorView: View {
    let title: String
    @State private var model: BookmarkEditorModel
    @Environment(AccountStore.self) private var accounts
    @Environment(\.dismiss) private var dismiss

    init(url: URL, title: String) {
        self.title = title
        _model = State(initialValue: BookmarkEditorModel(url: url))
    }

    var body: some View {
        Group {
            if let api = accounts.bookmarkAPI {
                editor(api: api)
                    // Signing in again gives a new token, which starts the load over.
                    .task(id: accounts.account) { await model.load(api: api, accounts: accounts) }
            } else {
                SignInPrompt()
            }
        }
        .navigationTitle(model.saved == nil ? "ブックマークを追加" : "ブックマークを編集")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("キャンセル") { dismiss() }
            }
        }
    }

    @ViewBuilder
    private func editor(api: BookmarkAPI) -> some View {
        switch model.phase {
        case .loading:
            ProgressView()
        case let .failed(error):
            List {
                ErrorBanner(error: error)
                Button("再読み込み") { Task { await model.load(api: api, accounts: accounts) } }
            }
        case .ready:
            BookmarkForm(model: model, title: title, api: api, dismiss: dismiss)
        }
    }
}

private struct BookmarkForm: View {
    @Bindable var model: BookmarkEditorModel
    let title: String
    let api: BookmarkAPI
    let dismiss: DismissAction
    @Environment(AccountStore.self) private var accounts
    @State private var isConfirmingDelete = false
    @FocusState private var isTagFieldFocused: Bool

    var body: some View {
        Form {
            Section {
                Text(title).font(.subheadline).foregroundStyle(.secondary).lineLimit(2)
            }

            if let error = model.actionError {
                Section { ErrorBanner(error: error) }
            }

            Section("コメント") {
                TextField("コメント", text: $model.draft.comment, axis: .vertical)
                    .lineLimit(3...8)
            }

            Section {
                if !model.draft.tags.isEmpty {
                    TagChips(tags: model.draft.tags) { model.draft.removeTag($0) }
                }
                TextField("タグを追加", text: $model.tagInput)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .submitLabel(.done)
                    .focused($isTagFieldFocused)
                    .onSubmit {
                        model.addTag(model.tagInput)
                        isTagFieldFocused = true
                    }
                    .disabled(!model.draft.canAddTag)
                ForEach(model.suggestions) { tag in
                    Button {
                        model.addTag(tag.name)
                    } label: {
                        LabeledContent(tag.name, value: "\(tag.count)")
                    }
                    .tint(.primary)
                    .disabled(!model.draft.canAddTag)
                }
            } header: {
                Text("タグ")
            } footer: {
                Text("\(model.draft.tags.count) / \(BookmarkDraft.maxTags) 個")
            }

            Section {
                Toggle("非公開にする", isOn: $model.draft.isPrivate)
            }

            if model.saved != nil {
                Section {
                    Button("ブックマークを削除", role: .destructive) { isConfirmingDelete = true }
                        .confirmationDialog("ブックマークを削除しますか？", isPresented: $isConfirmingDelete, titleVisibility: .visible) {
                            Button("削除", role: .destructive) {
                                Task {
                                    if await model.delete(api: api, accounts: accounts) { dismiss() }
                                }
                            }
                        }
                }
            }
        }
        .disabled(model.isWorking)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("保存") {
                    Task {
                        if await model.save(api: api, accounts: accounts) { dismiss() }
                    }
                }
                .disabled(model.isWorking)
            }
        }
    }
}

/// The chosen tags, each removed by tapping it.
private struct TagChips: View {
    let tags: [String]
    let remove: (String) -> Void

    var body: some View {
        FlowLayout(spacing: 6) {
            ForEach(tags, id: \.self) { tag in
                Button {
                    remove(tag)
                } label: {
                    Label(tag, systemImage: "xmark.circle.fill")
                        .labelStyle(TrailingIconLabelStyle())
                        .font(.subheadline)
                }
                .buttonStyle(.bordered)
                .buttonBorderShape(.capsule)
                .accessibilityHint("タグを外します")
            }
        }
        .padding(.vertical, 4)
    }
}

private struct TrailingIconLabelStyle: LabelStyle {
    func makeBody(configuration: Configuration) -> some View {
        HStack(spacing: 4) {
            configuration.title
            configuration.icon.foregroundStyle(.secondary)
        }
    }
}

/// Lays its subviews out in rows, wrapping to the next row when one is full.
private struct FlowLayout: Layout {
    var spacing: CGFloat

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let rows = rows(width: proposal.width ?? .infinity, subviews: subviews)
        let height = rows.map(\.height).reduce(0, +) + spacing * CGFloat(max(rows.count - 1, 0))
        let width = rows.map(\.width).max() ?? 0
        return CGSize(width: proposal.width ?? width, height: height)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var y = bounds.minY
        for row in rows(width: bounds.width, subviews: subviews) {
            var x = bounds.minX
            for index in row.indices {
                let size = subviews[index].sizeThatFits(.unspecified)
                subviews[index].place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
                x += size.width + spacing
            }
            y += row.height + spacing
        }
    }

    private struct Row {
        var indices: [Int] = []
        var width: CGFloat = 0
        var height: CGFloat = 0
    }

    private func rows(width: CGFloat, subviews: Subviews) -> [Row] {
        var rows: [Row] = []
        var row = Row()
        for index in subviews.indices {
            let size = subviews[index].sizeThatFits(.unspecified)
            let needed = row.indices.isEmpty ? size.width : row.width + spacing + size.width
            if needed > width, !row.indices.isEmpty {
                rows.append(row)
                row = Row()
            }
            row.width = row.indices.isEmpty ? size.width : row.width + spacing + size.width
            row.height = max(row.height, size.height)
            row.indices.append(index)
        }
        if !row.indices.isEmpty {
            rows.append(row)
        }
        return rows
    }
}
