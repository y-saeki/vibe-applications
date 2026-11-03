import EntryMute
import SwiftUI

/// The entry a new rule is being made from: its domain or title fills the
/// pattern, and the form says whether the rule as typed would hide it.
struct MuteSource: Identifiable {
    var id: URL { url }
    let title: String
    let url: URL

    func suggestedPattern(for target: MuteRule.Target) -> String {
        switch target {
        case .url: url.host() ?? url.absoluteString
        case .title: title
        }
    }
}

/// Adds a mute rule, or edits one already saved.
struct MuteRuleEditorView: View {
    private let source: MuteSource?
    private let isNew: Bool
    @State private var rule: MuteRule
    @Environment(MuteStore.self) private var mutes
    @Environment(\.dismiss) private var dismiss

    /// A new rule, filled in from `source` when there is one.
    init(source: MuteSource?) {
        self.source = source
        isNew = true
        _rule = State(initialValue: MuteRule(
            target: .url,
            syntax: .wildcard,
            pattern: source?.suggestedPattern(for: .url) ?? ""
        ))
    }

    init(editing rule: MuteRule) {
        source = nil
        isNew = false
        _rule = State(initialValue: rule)
    }

    var body: some View {
        Form {
            Section {
                Picker("対象", selection: $rule.target) {
                    Text("ドメイン・URL").tag(MuteRule.Target.url)
                    Text("タイトル").tag(MuteRule.Target.title)
                }
                .pickerStyle(.segmented)
                .onChange(of: rule.target) { old, new in
                    // Follows the target only while the pattern is still the
                    // one this form filled in, never over what the user typed.
                    if let source, rule.pattern == source.suggestedPattern(for: old) {
                        rule.pattern = source.suggestedPattern(for: new)
                    }
                }
                Picker("書き方", selection: $rule.syntax) {
                    Text("ワイルドカード").tag(MuteRule.Syntax.wildcard)
                    Text("正規表現").tag(MuteRule.Syntax.regex)
                }
                .pickerStyle(.segmented)
            }

            Section {
                TextField(placeholder, text: $rule.pattern, axis: .vertical)
                    .lineLimit(1...4)
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .font(rule.syntax == .regex ? .body.monospaced() : .body)
                if case let .invalidRegex(detail)? = rule.problem {
                    Text("正規表現として読めません。\(detail)")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
            } footer: {
                Text(explanation)
            }

            if let source {
                Section("この記事") {
                    Text(source.title).font(.subheadline).lineLimit(2)
                    Text(source.url.absoluteString).font(.caption).foregroundStyle(.secondary).lineLimit(2)
                    if rule.problem == nil {
                        if willMute(source) {
                            Label("この記事は一覧に表示されなくなります", systemImage: "eye.slash")
                                .foregroundStyle(.secondary)
                        } else {
                            Label("この条件では、この記事は隠れません", systemImage: "exclamationmark.triangle")
                                .foregroundStyle(.orange)
                        }
                    }
                }
            }
        }
        .navigationTitle(isNew ? "ミュートを追加" : "ミュートを編集")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("キャンセル") { dismiss() }
            }
            ToolbarItem(placement: .confirmationAction) {
                Button("保存") {
                    rule.pattern = rule.pattern.trimmingCharacters(in: .whitespacesAndNewlines)
                    mutes.save(rule)
                    dismiss()
                }
                .disabled(rule.problem != nil)
            }
        }
    }

    private func willMute(_ source: MuteSource) -> Bool {
        (try? MuteMatcher(rule))?.matches(title: source.title, url: source.url) ?? false
    }

    private var placeholder: String {
        switch (rule.target, rule.syntax) {
        case (.url, .wildcard): "example.com"
        case (.url, .regex): #"^https://[^/]+\.example\.com/"#
        case (.title, .wildcard): "キーワード"
        case (.title, .regex): #"^\d+選"#
        }
    }

    private var explanation: String {
        switch (rule.target, rule.syntax) {
        case (.url, .wildcard):
            "ドメインだけを書くと、そのサブドメインも隠れます。example.com/blog/ のように / を含めると、その URL で始まる記事が隠れます。* は任意の文字列、? は任意の 1 文字です。"
        case (.url, .regex):
            "https:// から始まる URL 全体のどこかに一致すると隠れます。大文字と小文字は区別しません。"
        case (.title, .wildcard):
            "タイトルのどこかに含まれると隠れます。* は任意の文字列、? は任意の 1 文字です。大文字と小文字、全角と半角は区別しません。"
        case (.title, .regex):
            "タイトルのどこかに一致すると隠れます。大文字と小文字は区別しません。"
        }
    }
}

extension MuteRule {
    /// How the rule reads in the list of rules.
    var summary: String {
        switch (target, syntax) {
        case (.url, .wildcard): "ドメイン・URL"
        case (.url, .regex): "ドメイン・URL（正規表現）"
        case (.title, .wildcard): "タイトル"
        case (.title, .regex): "タイトル（正規表現）"
        }
    }
}

/// Every saved rule, each opened for editing by tapping it.
struct MuteRuleListView: View {
    @Environment(MuteStore.self) private var mutes
    @State private var isAdding = false
    @State private var editing: MuteRule?

    var body: some View {
        List {
            if mutes.rules.isEmpty {
                Text("ミュートの条件はまだありません。一覧の記事を左にスワイプしても追加できます。")
                    .foregroundStyle(.secondary)
            }
            ForEach(mutes.rules) { rule in
                Button {
                    editing = rule
                } label: {
                    VStack(alignment: .leading, spacing: 4) {
                        Text(rule.pattern)
                            .font(rule.syntax == .regex ? .body.monospaced() : .body)
                            .lineLimit(2)
                        Text(rule.summary)
                            .font(.caption)
                            .foregroundStyle(.secondary)
                    }
                }
                .tint(.primary)
            }
            .onDelete { offsets in
                for id in offsets.map({ mutes.rules[$0].id }) {
                    mutes.remove(id: id)
                }
            }
        }
        .navigationTitle("ミュート")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("追加", systemImage: "plus") { isAdding = true }
            }
        }
        .sheet(isPresented: $isAdding) {
            NavigationStack { MuteRuleEditorView(source: nil) }
        }
        .sheet(item: $editing) { rule in
            NavigationStack { MuteRuleEditorView(editing: rule) }
        }
    }
}
