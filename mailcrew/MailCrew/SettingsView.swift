import SwiftUI

/// The settings window: the accounts Mail Crew reads, the custom views, the
/// senders whose images are shown without asking, and whether BIMI is looked
/// up. The words are the Tauri app's (`src/index.html`).
struct SettingsView: View {
    let model: AppModel
    @State private var clientId = ""
    @State private var clientSecret = ""
    @State private var otherClient = false
    /// The view in the form: nil while it is closed.
    @State private var editing: ViewDraft?

    var body: some View {
        Form {
            accounts
            views
            imageSenders
            brand
        }
        .formStyle(.grouped)
        .frame(width: 560)
        .frame(minHeight: 480)
        .sheet(item: $editing) { draft in
            ViewForm(model: model, draft: draft) { editing = nil }
        }
    }

    // MARK: Accounts

    private var accounts: some View {
        Section("アカウント") {
            ForEach(model.accounts, id: \.self) { account in
                HStack {
                    Text(account)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    Button("削除") { Task { await model.removeAccount(account) } }
                        .disabled(model.removingAccount != nil || model.addingAccount)
                }
            }

            DisclosureGroup("別の OAuth クライアントでログインする", isExpanded: $otherClient) {
                Text("空欄のままなら、最初のアカウントと同じクライアントを使います。")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                TextField("OAuth クライアント ID", text: $clientId)
                SecureField("クライアント シークレット", text: $clientSecret)
            }
            .autocorrectionDisabled()
            .disabled(model.addingAccount)

            if let error = model.accountError {
                Text(error)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }

            if model.addingAccount {
                HStack(spacing: 12) {
                    ProgressView().controlSize(.small)
                    Text("ブラウザで、追加する Google アカウントにログインしてください。")
                    Spacer()
                    Button("中止") { model.cancelSignIn() }
                }
            } else {
                Button("Google でログインして追加") {
                    Task {
                        await model.addAccount(clientId: clientId, clientSecret: clientSecret)
                        if model.accountError == nil {
                            clientId = ""
                            clientSecret = ""
                            otherClient = false
                        }
                    }
                }
                .disabled(model.removingAccount != nil)
            }
        }
    }

    // MARK: Views

    private var views: some View {
        Section {
            if model.views.isEmpty {
                Text("ビューはまだありません")
                    .foregroundStyle(.secondary)
            }
            ForEach(model.views, id: \.id) { view in
                HStack {
                    VStack(alignment: .leading, spacing: 2) {
                        Text(view.name)
                        Text(AppModel.describe(view))
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .lineLimit(1)
                            .truncationMode(.tail)
                    }
                    Spacer()
                    Button("編集") { open(view) }
                    Button("削除") { Task { await model.deleteView(view.id) } }
                }
            }
            if let error = model.viewError, editing == nil {
                Text(error)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }
            Button("ビューを追加") { open(nil) }
        } header: {
            Text("ビュー")
        } footer: {
            Text("受信トレイの中を絞り込んだ一覧です。サイドバーの「受信トレイ」「あとで読む」の下に並びます。")
                .foregroundStyle(.secondary)
        }
    }

    private func open(_ view: SavedView?) {
        model.clearViewError()
        editing = ViewDraft(
            id: view?.id,
            name: view?.name ?? "",
            accounts: Set(view?.accounts ?? []),
            query: view?.query ?? ""
        )
    }

    // MARK: Images and BIMI

    private var imageSenders: some View {
        Section {
            if model.imageSenders.isEmpty {
                Text("画像を常に表示する差出人はいません")
                    .foregroundStyle(.secondary)
            }
            ForEach(model.imageSenders, id: \.self) { sender in
                HStack {
                    Text(sender)
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Spacer()
                    Button("削除") { Task { await model.removeImageSender(sender) } }
                }
            }
            if let error = model.imageSenderError {
                Text(error)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }
        } header: {
            Text("画像を常に表示する差出人")
        } footer: {
            Text("ここにある差出人からのメールは、外部の画像を確認せずに読み込みます。削除すると、次からはメールごとに確認します。")
                .foregroundStyle(.secondary)
        }
    }

    private var brand: some View {
        Section {
            Toggle("BIMI のロゴを表示する", isOn: Binding(
                get: { model.looksUpBrands },
                set: { on in Task { await model.setLooksUpBrands(on) } }
            ))
            .disabled(model.savingBrandSetting)
            Text("差出人のドメインが公開しているロゴを、認証局の証明書（VMC・CMC）で確かめてから表示します。確かめるために、差出人のドメインの DNS と、証明書が置かれたサーバーに問い合わせます。")
                .font(.callout)
                .foregroundStyle(.secondary)
            if let error = model.brandSettingError {
                Text(error)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }
        } header: {
            Text("送信元の検証")
        } footer: {
            Text("カードの件名の下に、Gmail が確かめた SPF・DKIM・DMARC の結果を表示します。")
                .foregroundStyle(.secondary)
        }
    }
}

/// What the view form holds: a view being made (no id yet) or edited.
struct ViewDraft: Identifiable {
    var id: String?
    var name: String
    var accounts: Set<String>
    var query: String
}

/// Makes or edits a custom view.
private struct ViewForm: View {
    let model: AppModel
    @State var draft: ViewDraft
    let close: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Form {
                TextField("名前", text: $draft.name)
                Section("アカウント（選ばなければすべて）") {
                    ForEach(model.accounts, id: \.self) { account in
                        Toggle(account, isOn: Binding(
                            get: { draft.accounts.contains(account) },
                            set: { on in
                                if on { draft.accounts.insert(account) } else { draft.accounts.remove(account) }
                            }
                        ))
                    }
                }
                Section("検索条件（Gmail の検索と同じ書き方）") {
                    TextField("検索条件", text: $draft.query, prompt: Text("例: from:example.com is:unread"))
                        .labelsHidden()
                }
                if let error = model.viewError {
                    Text(error)
                        .foregroundStyle(.red)
                        .textSelection(.enabled)
                }
            }
            .formStyle(.grouped)
            .autocorrectionDisabled()

            HStack {
                Spacer()
                Button("キャンセル", role: .cancel) {
                    model.clearViewError()
                    close()
                }
                .keyboardShortcut(.cancelAction)
                Button("保存") { Task { await save() } }
                    .keyboardShortcut(.defaultAction)
            }
            .padding([.horizontal, .bottom], 20)
        }
        .frame(width: 460)
    }

    private func save() async {
        guard let view = model.view(
            id: draft.id, name: draft.name, accounts: draft.accounts, query: draft.query
        ) else { return }
        if await model.saveView(view) { close() }
    }
}
