import SwiftUI

struct SettingsView: View {
    @Environment(ReadHistoryStore.self) private var readHistory
    @Environment(\.dismiss) private var dismiss
    @State private var isConfirmingClear = false

    var body: some View {
        Form {
            Section {
                LabeledContent("記録している記事", value: "\(readHistory.history.count) 件")
                Button("既読の記録を消す", role: .destructive) { isConfirmingClear = true }
                    .disabled(readHistory.history.count == 0)
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
        .confirmationDialog("既読の記録を消しますか？", isPresented: $isConfirmingClear, titleVisibility: .visible) {
            Button("既読の記録を消す", role: .destructive) { readHistory.removeAll() }
        } message: {
            Text("一覧のすべての記事が未読の表示に戻ります。")
        }
    }
}
