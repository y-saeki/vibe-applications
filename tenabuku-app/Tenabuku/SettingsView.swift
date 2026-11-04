import SwiftUI

struct SettingsView: View {
    @Environment(ReadHistoryStore.self) private var readHistory
    @Environment(AccountStore.self) private var accounts
    @Environment(MuteStore.self) private var mutes
    @Environment(WebSessionStore.self) private var webSession
    @Environment(\.dismiss) private var dismiss
    @State private var isConfirmingClear = false
    @State private var isConfirmingSignOut = false
    @State private var isConfirmingWebSignOut = false

    var body: some View {
        Form {
            Section {
                if let account = accounts.account {
                    LabeledContent("ログイン中", value: "id:\(account.urlName)")
                    if accounts.needsSignInAgain {
                        Text("はてなが、保存してあるログイン情報を受け付けませんでした。")
                            .font(.footnote)
                            .foregroundStyle(.red)
                        SignInButton(title: "ログインし直す")
                    }
                    Button("ログアウト", role: .destructive) { isConfirmingSignOut = true }
                        .confirmationDialog("ログアウトしますか？", isPresented: $isConfirmingSignOut, titleVisibility: .visible) {
                            Button("ログアウト", role: .destructive) { accounts.signOut() }
                        }
                } else {
                    SignInButton()
                }
            } header: {
                Text("はてなアカウント")
            } footer: {
                if accounts.canSignIn {
                    Text("ブックマークの保存と、マイブックマークに使います。")
                } else {
                    Text("このビルドには、はてなの consumer key が入っていません。")
                }
            }

            Section {
                if webSession.isSignedIn {
                    LabeledContent("状態", value: "ログイン中")
                    Button("ログアウト", role: .destructive) { isConfirmingWebSignOut = true }
                        .confirmationDialog("Web からログアウトしますか？", isPresented: $isConfirmingWebSignOut, titleVisibility: .visible) {
                            Button("ログアウト", role: .destructive) {
                                Task { await webSession.signOut() }
                            }
                        }
                } else {
                    WebSignInButton()
                }
            } header: {
                Text("はてなの Web")
            } footer: {
                Text("マイホットエントリーに使います。はてなが公式に提供していない方法で読み込むので、予告なく読めなくなることがあります。")
            }

            Section {
                NavigationLink {
                    MuteRuleListView()
                } label: {
                    LabeledContent("ミュートの条件", value: "\(mutes.rules.count) 件")
                }
            } header: {
                Text("ミュート")
            } footer: {
                Text("条件に合う記事は、一覧に表示されません。")
            }

            Section {
                LabeledContent("記録している記事", value: "\(readHistory.history.count) 件")
                Button("既読の記録を消す", role: .destructive) { isConfirmingClear = true }
                    .disabled(readHistory.history.count == 0)
                    // On the button, not the form: where the dialog shows as a
                    // popover, it points at the view it is attached to.
                    .confirmationDialog("記録を消しますか？", isPresented: $isConfirmingClear, titleVisibility: .visible) {
                        // A verb that answers the title, so the popover does not
                        // repeat the label of the button it points at.
                        Button("消す", role: .destructive) { readHistory.removeAll() }
                    }
            } header: {
                Text("既読")
            } footer: {
                Text("開いた記事は、一覧でタイトルが薄く表示されます。記録は開いてから 30 日たつと消えます。")
            }
        }
        .navigationTitle("設定")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("完了") { dismiss() }
            }
        }
    }
}
