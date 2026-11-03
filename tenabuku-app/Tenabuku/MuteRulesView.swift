import Muting
import SwiftUI

/// The mute rules, reached from the settings. Tapping a rule edits it.
struct MuteRulesView: View {
    @Environment(MuteStore.self) private var mutes
    @State private var editing: MuteRule?

    var body: some View {
        List {
            ForEach(mutes.rules) { rule in
                Button { editing = rule } label: {
                    MuteRuleRow(rule: rule)
                }
                .tint(.primary)
            }
            .onDelete { offsets in
                for id in offsets.map({ mutes.rules[$0].id }) {
                    mutes.remove(id: id)
                }
            }
        }
        .overlay {
            if mutes.rules.isEmpty {
                ContentUnavailableView(
                    "ミュートの条件はありません",
                    systemImage: "speaker.slash",
                    description: Text("条件に合う記事は、一覧に表示されなくなります。")
                )
            }
        }
        .navigationTitle("ミュート")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .primaryAction) {
                Button("追加", systemImage: "plus") {
                    editing = MuteRule(target: .url, syntax: .wildcard, pattern: "")
                }
            }
        }
        .sheet(item: $editing) { rule in
            NavigationStack {
                MuteRuleEditorView(rule: rule, isNew: !mutes.rules.contains { $0.id == rule.id })
            }
        }
    }
}

private struct MuteRuleRow: View {
    let rule: MuteRule

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(rule.pattern)
                .font(.body.monospaced())
                .lineLimit(2)
            Text("\(rule.target.displayName)・\(rule.syntax.displayName)")
                .font(.caption)
                .foregroundStyle(.secondary)
        }
        .padding(.vertical, 2)
        .frame(maxWidth: .infinity, alignment: .leading)
        .contentShape(Rectangle())
    }
}

struct MuteRuleEditorView: View {
    @Environment(MuteStore.self) private var mutes
    @Environment(\.dismiss) private var dismiss
    @State private var rule: MuteRule
    private let isNew: Bool

    init(rule: MuteRule, isNew: Bool) {
        _rule = State(initialValue: rule)
        self.isNew = isNew
    }

    private var problem: MuteRuleError? {
        do {
            try rule.validate()
            return nil
        } catch {
            return error as? MuteRuleError
        }
    }

    var body: some View {
        Form {
            Section {
                Picker("対象", selection: $rule.target) {
                    ForEach(MuteRule.Target.allCases, id: \.self) { Text($0.displayName).tag($0) }
                }
                Picker("書き方", selection: $rule.syntax) {
                    ForEach(MuteRule.Syntax.allCases, id: \.self) { Text($0.displayName).tag($0) }
                }
            }

            Section {
                TextField("条件", text: $rule.pattern, axis: .vertical)
                    .font(.body.monospaced())
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(rule.target == .url ? .URL : .default)
                if problem == .invalidRegex {
                    Text("正規表現として読み取れません。")
                        .font(.footnote)
                        .foregroundStyle(.red)
                }
            } footer: {
                Text(hint)
            }

            if !isNew {
                Section {
                    Button("この条件を削除", role: .destructive) {
                        mutes.remove(id: rule.id)
                        dismiss()
                    }
                }
            }
        }
        .navigationTitle(isNew ? "条件を追加" : "条件を編集")
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
                .disabled(problem != nil)
            }
        }
    }

    private var hint: String {
        switch (rule.target, rule.syntax) {
        case (.url, .wildcard):
            "「example.com」と書くと、そのドメインとサブドメインの記事を隠します。「example.com/blog/」のように / を含めると、URL がそこから始まる記事を隠します。* は任意の文字列に合います。大文字と小文字は区別しません。"
        case (.url, .regex):
            "URL 全体のどこかに合えば隠します。例: ^https://[^/]+\\.example\\.com/ 。大文字と小文字は区別しません。"
        case (.title, .wildcard):
            "この言葉をタイトルに含む記事を隠します。* は任意の文字列に合います。大文字と小文字、全角と半角は区別しません。"
        case (.title, .regex):
            "タイトルのどこかに合えば隠します。例: ^【PR】 。大文字と小文字は区別しません。"
        }
    }
}

extension MuteRule.Target {
    var displayName: String {
        switch self {
        case .url: "URL・ドメイン"
        case .title: "タイトル"
        }
    }
}

extension MuteRule.Syntax {
    var displayName: String {
        switch self {
        case .wildcard: "ワイルドカード"
        case .regex: "正規表現"
        }
    }
}
