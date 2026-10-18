import SwiftUI

/// The dialog that records a shortcut: it shows the keys as they are
/// pressed, and saves only a combination that can be a shortcut. While it is
/// open, every key pressed in winwin comes here (see AppDelegate), so its
/// buttons are clicked rather than pressed.
struct RecorderSheet: View {
    @ObservedObject var model: SettingsModel
    /// The most keycaps drawn large; more would not fit.
    private static let largeCaps = 5

    var body: some View {
        let recording = model.recording
        let keys = recording?.keys ?? []
        VStack(alignment: .leading, spacing: 16) {
            Text("ショートカットを設定").font(.title3).bold()
            Text("設定するキーの組み合わせを押してください。\nControl・Option・Command のいずれかを含める必要があります。")
                .fixedSize(horizontal: false, vertical: true)
            Group {
                if keys.isEmpty {
                    Text("キーを押してください").foregroundColor(.secondary)
                } else {
                    let count = keys.reduce(0) { $0 + $1.count }
                    KeysView(keys: keys, size: count <= RecorderSheet.largeCaps ? .large : .medium)
                }
            }
            .frame(maxWidth: .infinity, minHeight: 112)
            .background(RoundedRectangle(cornerRadius: 8).fill(Color(nsColor: .controlBackgroundColor)))
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor)))
            Text(recording?.problem ?? " ")
                .foregroundColor(.red)
                .frame(maxWidth: .infinity)
            HStack(spacing: 24) {
                Button("リセット") { model.resetRecording() }
                Button("クリア") { model.clearRecording() }
            }
            .buttonStyle(.link)
            .frame(maxWidth: .infinity)
            HStack {
                Spacer()
                Button("キャンセル") { model.endRecording(keep: false) }
                Button("決定") { model.endRecording(keep: true) }
                    .buttonStyle(.borderedProminent)
                    .disabled(!(recording?.complete ?? false))
            }
        }
        .padding(24)
        .frame(width: 448)
    }
}
