import SwiftUI

/// The settings window: the sets at the left, the selected set's commands
/// beside them, and at the right the set's applications and the selected
/// command's gesture and keys; below them launching at login and the
/// buttons that close the window.
struct SettingsView: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            if let error = model.error {
                ErrorBanner(problem: error) { model.error = nil }
            }
            HStack(alignment: .top, spacing: 20) {
                SetList(model: model).frame(width: 220)
                CommandList(model: model).frame(width: 260)
                ScrollView {
                    Details(model: model)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            footer
                .sheet(isPresented: Binding(get: { model.menuPicking != nil }, set: { _ in })) {
                    MenuSheet(model: model)
                }
        }
        .padding(24)
        .frame(minWidth: 1160, minHeight: 680)
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
                Toggle("ログイン時に meta-gesture を起動する", isOn: $model.autostart)
                    .toggleStyle(.checkbox)
                HStack(spacing: 8) {
                    Button("エクスポート...") { model.export() }
                    Button("インポート...") { model.importFile() }
                }
            }
            Spacer()
            Button("保存して閉じる") { model.save() }
                .buttonStyle(.borderedProminent)
                .frame(minWidth: 96)
            Button("キャンセル") { model.cancel() }
                .frame(minWidth: 96)
        }
    }
}

/// What failed and why, until it is closed.
private struct ErrorBanner: View {
    let problem: Problem
    let dismiss: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 8) {
            Image(systemName: "xmark.octagon.fill").foregroundColor(.red)
            VStack(alignment: .leading, spacing: 2) {
                Text(problem.title).bold()
                Text(problem.message).fixedSize(horizontal: false, vertical: true)
            }
            Spacer()
            Button(action: dismiss) { Image(systemName: "xmark") }
                .buttonStyle(.borderless)
        }
        .padding(12)
        .background(RoundedRectangle(cornerRadius: 8).fill(Color.red.opacity(0.12)))
    }
}

/// A list whose selected line is the model's, drawn with a gray behind it.
private struct Lines<Line: View>: View {
    let count: Int
    let selected: Int?
    let select: (Int) -> Void
    @ViewBuilder let line: (Int) -> Line

    var body: some View {
        List {
            ForEach(0..<count, id: \.self) { i in
                line(i)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .contentShape(Rectangle())
                    .onTapGesture { select(i) }
                    .listRowBackground(
                        RoundedRectangle(cornerRadius: 4)
                            .fill(i == selected ? Color.secondary.opacity(0.18) : Color.clear)
                    )
            }
        }
        .listStyle(.bordered(alternatesRowBackgrounds: false))
    }
}

private func listButton(_ label: String, enabled: Bool, action: @escaping () -> Void) -> some View {
    Button(action: action) { Text(label).frame(maxWidth: .infinity) }
        .disabled(!enabled)
}

private struct SetList: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("セット").font(.headline)
            Lines(count: model.sets.count, selected: model.selectedSet, select: model.selectSet) { i in
                VStack(alignment: .leading, spacing: 2) {
                    Text(model.sets[i].name).fontWeight(.semibold).lineLimit(1)
                    Text(model.sets[i].apps).font(.caption).foregroundColor(.secondary).lineLimit(1)
                }
                .padding(.vertical, 2)
            }
            HStack(spacing: 6) {
                listButton("追加", enabled: true) { model.addSet() }
                listButton("削除", enabled: !model.isCommon) { model.deleteSet() }
                listButton("上へ", enabled: model.canMoveSet(up: true)) { model.moveSet(up: true) }
                listButton("下へ", enabled: model.canMoveSet(up: false)) { model.moveSet(up: false) }
            }
            Menu("プリセットから追加") {
                ForEach(Array(model.presets.enumerated()), id: \.offset) { i, name in
                    Button(name) { model.addPreset(i) }
                }
            }
        }
    }
}

private struct CommandList: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("コマンド").font(.headline)
            Lines(count: model.commands.count, selected: model.selectedCommand, select: model.selectCommand) { i in
                HStack(spacing: 8) {
                    Text(model.commands[i].gesture.isEmpty ? "—" : model.commands[i].gesture)
                        .font(.system(size: 16))
                        .frame(width: 76, alignment: .leading)
                    Text(model.commands[i].name).lineLimit(1)
                }
                .padding(.vertical, 2)
            }
            HStack(spacing: 6) {
                listButton("追加", enabled: true) { model.addCommand() }
                listButton("削除", enabled: model.selectedCommand != nil) { model.deleteCommand() }
                listButton("上へ", enabled: model.canMoveCommand(up: true)) { model.moveCommand(up: true) }
                listButton("下へ", enabled: model.canMoveCommand(up: false)) { model.moveCommand(up: false) }
            }
        }
    }
}

/// A label above a control.
private struct Field<Content: View>: View {
    let label: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(label)
            content()
        }
    }
}

private struct Details: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            Field(label: "セット名") {
                TextField("", text: Binding(get: { model.setName }, set: { model.renameSet($0) }))
                    .frame(width: 320)
                    .disabled(model.isCommon)
            }
            if !model.isCommon {
                Field(label: "アプリ") { apps }
            }
            if model.selectedCommand != nil {
                command
            }
        }
        .padding(.trailing, 16)
    }

    private var apps: some View {
        VStack(alignment: .leading, spacing: 10) {
            ForEach(Array(model.apps.enumerated()), id: \.offset) { i, app in
                HStack(spacing: 8) {
                    Text(app.name)
                    Text(app.id).foregroundColor(.secondary)
                    Spacer()
                    Button { model.removeApp(i) } label: { Image(systemName: "xmark") }
                }
            }
            HStack(spacing: 8) {
                Menu("起動中のアプリから追加") {
                    ForEach(model.running, id: \.self) { app in
                        Button(app.name) { model.add(app) }
                    }
                }
                .fixedSize()
                Button("選択...") { model.browse() }
            }
        }
    }

    private var command: some View {
        VStack(alignment: .leading, spacing: 20) {
            Field(label: "コマンド名") {
                TextField("", text: Binding(get: { model.commandName }, set: { model.renameCommand($0) }))
                    .frame(width: 320)
            }
            Field(label: "ジェスチャー") {
                VStack(alignment: .leading, spacing: 10) {
                    Text(model.gesture.isEmpty ? "未設定" : model.gesture)
                        .font(.system(size: 28))
                        .foregroundColor(model.gesture.isEmpty ? .secondary : .primary)
                    HStack(spacing: 6) {
                        ForEach(Array(["←", "↑", "→", "↓"].enumerated()), id: \.offset) { i, arrow in
                            Button(arrow) { model.push(i) }.frame(width: 40)
                        }
                        Button("1 つ戻す") { model.pop() }.disabled(model.gesture.isEmpty)
                        Button("クリア") { model.clearGesture() }.disabled(model.gesture.isEmpty)
                    }
                    if let conflict = model.conflict {
                        Text(conflict).foregroundColor(.red)
                    }
                }
            }
            Field(label: "送るキー") {
                VStack(alignment: .leading, spacing: 8) {
                    ForEach(Array(model.keys.enumerated()), id: \.offset) { i, line in
                        HStack(spacing: 12) {
                            Text(line.label).lineLimit(1).frame(width: 170, alignment: .leading)
                            if !line.caps.isEmpty {
                                KeycapsView(caps: line.caps, size: .small)
                            } else if !line.menu.isEmpty {
                                Label(line.menu, systemImage: "filemenu.and.selection").lineLimit(1)
                            } else {
                                Text(i == 0 ? "未設定" : "セットの既定").foregroundColor(.secondary)
                            }
                            Spacer()
                            Button("変更") { model.startRecording(i) }
                            Button("メニューから選ぶ...") { model.startPickingMenu(i) }
                            Button("クリア") { model.clearKeys(i) }.disabled(line.caps.isEmpty && line.menu.isEmpty)
                        }
                    }
                }
            }
        }
        .padding(20)
        .background(RoundedRectangle(cornerRadius: 8).fill(Color(nsColor: .controlBackgroundColor)))
        .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor)))
    }
}
