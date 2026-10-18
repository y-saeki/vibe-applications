import SwiftUI

/// The settings window: the shortcuts at the left, the selected one's fields
/// and a preview of where it puts a window at the right, and below them the
/// theme, launching at login and the buttons that close the window.
struct SettingsView: View {
    @ObservedObject var model: SettingsModel
    let path: String

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let error = model.error {
                ErrorBanner(message: error) { model.error = nil }
            }
            HStack(alignment: .top, spacing: 24) {
                ShortcutList(model: model)
                    .frame(width: 436)
                FormView(model: model)
            }
            footer
        }
        .padding(24)
        .frame(minWidth: 900, maxWidth: 900, minHeight: 690)
        .alert("変更を保存せずに閉じますか?", isPresented: $model.confirming) {
            Button("閉じる", role: .destructive) { model.onClose() }
            Button("戻る", role: .cancel) {}
        } message: {
            Text("保存していない変更は失われます。")
        }
        .sheet(isPresented: Binding(get: { model.isRecording }, set: { _ in })) {
            RecorderSheet(model: model)
        }
    }

    private var footer: some View {
        HStack(alignment: .bottom) {
            VStack(alignment: .leading, spacing: 12) {
                Picker("テーマ", selection: Binding(get: { Int(model.theme) }, set: { model.setTheme($0) })) {
                    ForEach(Array(model.themes.enumerated()), id: \.offset) { i, label in
                        Text(label).tag(i)
                    }
                }
                .fixedSize()
                Toggle("ログイン時に winwin を起動する", isOn: $model.autostart)
                    .toggleStyle(.checkbox)
            }
            Spacer()
            Button("保存して閉じる") { model.save(path: path) }
                .buttonStyle(.borderedProminent)
                .frame(minWidth: 96)
            Button("キャンセル") { model.cancel() }
                .frame(minWidth: 96)
        }
    }
}

/// Why 保存して閉じる failed, until it is closed.
private struct ErrorBanner: View {
    let message: String
    let dismiss: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "xmark.octagon.fill").foregroundColor(.red)
            VStack(alignment: .leading, spacing: 2) {
                Text("保存できませんでした").bold()
                Text(message).fixedSize(horizontal: false, vertical: true)
            }
            Spacer()
            Button(action: dismiss) { Image(systemName: "xmark") }
                .buttonStyle(.borderless)
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 8).fill(Color.red.opacity(0.12)))
    }
}

/// The shortcuts, each with where it puts a window, the buttons that change
/// the list, and the test window's.
private struct ShortcutList: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        VStack(spacing: 12) {
            List {
                ForEach(Array(model.rows.enumerated()), id: \.element.id) { i, row in
                    RowLine(row: row)
                        .contentShape(Rectangle())
                        .onTapGesture { model.click(i) }
                        .listRowBackground(
                            RoundedRectangle(cornerRadius: 4)
                                .fill(row.selected ? Color.secondary.opacity(0.18) : Color.clear)
                        )
                }
                .onMove { from, to in model.move(from: from, to: to) }
            }
            .listStyle(.bordered(alternatesRowBackgrounds: false))

            HStack(spacing: 6) {
                listButton("追加", enabled: true) { model.add() }
                listButton("複製", enabled: model.hasSelection) { model.duplicate() }
                listButton("削除", enabled: model.hasSelection) { model.delete() }
                listButton("上へ", enabled: model.canMove(up: true)) { model.move(up: true) }
                listButton("下へ", enabled: model.canMove(up: false)) { model.move(up: false) }
            }

            HStack {
                Button {
                    model.setTesting(!model.testing)
                } label: {
                    Label(
                        model.testing ? "テスト用ウィンドウを閉じる" : "テスト用ウィンドウを開く",
                        systemImage: model.testing ? "xmark" : "macwindow"
                    )
                }
                Spacer()
            }
            .padding(.top, 4)
        }
    }

    private func listButton(_ label: String, enabled: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) { Text(label).frame(maxWidth: .infinity) }
            .disabled(!enabled)
    }
}

/// A line of the list: the keys at the left, the anchor, the width and the
/// height in columns of their own at the right, with an accent bar for a
/// selected row.
private struct RowLine: View {
    let row: Row
    static let columns: [CGFloat] = [36, 48, 48]

    var body: some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 1.5)
                .fill(row.selected ? Color.accentColor : Color.clear)
                .frame(width: 3, height: 16)
            ShortcutView(keys: row.keys, size: .small)
                .frame(maxWidth: .infinity, alignment: .leading)
            // リセット takes the room of the anchor and both sizes, フィット
            // that of both sizes.
            if row.reset {
                cell(row.anchor, width: RowLine.columns.reduce(16, +))
            } else {
                cell(row.anchor, width: RowLine.columns[0])
                if row.fit {
                    cell(row.width, width: RowLine.columns[1] + RowLine.columns[2] + 8)
                } else {
                    cell(row.width, width: RowLine.columns[1])
                    cell(row.height, width: RowLine.columns[2])
                }
            }
        }
        .padding(.vertical, 4)
    }

    private func cell(_ text: String, width: CGFloat) -> some View {
        Text(text)
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(width: width, alignment: .leading)
    }
}

/// The selected shortcut's fields and a preview of where it puts a window.
/// With anything but one row selected the fields are empty and disabled.
private struct FormView: View {
    @ObservedObject var model: SettingsModel
    @State private var choosingAnchor = false

    var body: some View {
        let fields = model.fields
        let enabled = fields != nil
        let placing = enabled && !(fields?.reset ?? false)
        let anchor = Int(fields?.anchor ?? 4)
        VStack(alignment: .leading, spacing: 16) {
            labeled("ショートカット") {
                HStack(spacing: 12) {
                    ShortcutView(keys: fields?.keys ?? [], size: .medium)
                    Button {
                        model.startRecording()
                    } label: {
                        Label("変更", systemImage: "pencil")
                    }
                    .disabled(!enabled)
                }
            }
            labeled("動作") {
                Picker("", selection: Binding(get: { fields?.reset ?? false }, set: { model.setReset($0) })) {
                    Text("配置").tag(false)
                    Text("リセット").tag(true)
                }
                .pickerStyle(.radioGroup)
                .horizontalRadioGroupLayout()
                .labelsHidden()
                .disabled(!enabled)
            }
            labeled("基準位置") {
                HStack(spacing: 16) {
                    Button {
                        choosingAnchor = true
                    } label: {
                        HStack {
                            Text(model.anchors[anchor].label)
                            Spacer()
                            Image(systemName: "chevron.down")
                        }
                        .frame(width: 140)
                    }
                    .disabled(!placing)
                    .popover(isPresented: $choosingAnchor, arrowEdge: .bottom) {
                        AnchorPicker(model: model, current: anchor, fit: fields?.fit ?? false)
                    }
                    Toggle("フィット", isOn: Binding(get: { fields?.fit ?? false }, set: { model.setFit($0) }))
                        .toggleStyle(.checkbox)
                        .disabled(!placing || !model.anchors[anchor].canFit)
                }
            }
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 24) {
                    labeled("幅") {
                        TextField("（幅を維持）", text: Binding(get: { fields?.width ?? "" }, set: { model.setWidth($0) }))
                            .frame(width: 144)
                    }
                    labeled("高さ") {
                        TextField("（高さを維持）", text: Binding(get: { fields?.height ?? "" }, set: { model.setHeight($0) }))
                            .frame(width: 144)
                    }
                }
                .disabled(!placing || (fields?.fit ?? false))
                if let problem = fields?.problem {
                    Text(problem)
                        .foregroundColor(.red)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            labeled("プレビュー") {
                PreviewView(preview: enabled ? model.preview : nil)
                    .padding(12)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(Color(nsColor: .controlBackgroundColor))
                    )
                    .overlay(
                        RoundedRectangle(cornerRadius: 8)
                            .stroke(Color(nsColor: .separatorColor))
                    )
            }
            Spacer()
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    /// A label above a control.
    private func labeled<Content: View>(_ label: String, @ViewBuilder content: () -> Content) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
            content()
        }
    }
}

/// The nine anchors as they lie on the screen; the current one in the accent
/// color, and with フィット on, the center out of reach. It stays open as an
/// anchor is chosen, until a click outside it or Esc.
private struct AnchorPicker: View {
    @ObservedObject var model: SettingsModel
    let current: Int
    let fit: Bool

    var body: some View {
        VStack(spacing: 4) {
            ForEach(0..<3, id: \.self) { row in
                HStack(spacing: 4) {
                    ForEach(0..<3, id: \.self) { column in
                        tile(row * 3 + column)
                    }
                }
            }
        }
        .padding(8)
    }

    @ViewBuilder private func tile(_ i: Int) -> some View {
        let choice = model.anchors[i]
        let button = Button {
            model.setAnchor(i)
        } label: {
            Text(choice.label).frame(width: 52, height: 28)
        }
        .disabled(fit && !choice.canFit)
        if i == current {
            button.buttonStyle(.borderedProminent)
        } else {
            button.buttonStyle(.bordered)
        }
    }
}
