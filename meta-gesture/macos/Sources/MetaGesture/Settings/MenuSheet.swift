import SwiftUI

/// The dialog that chooses an item of an application's menu bar for a line
/// of keys: found by name, it either gives the line its shortcut or is
/// chosen by the gesture itself.
struct MenuSheet: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        let picking = model.menuPicking
        let selected = model.menuLines.first { $0.item == model.menuItem }
        VStack(alignment: .leading, spacing: 16) {
            Text("メニューから選ぶ").font(.title3).bold()
            if let picking, !picking.apps.isEmpty {
                if picking.apps.count > 1 {
                    Picker("アプリ", selection: Binding(
                        get: { picking.app ?? "" },
                        set: { model.pickMenuApp($0) }
                    )) {
                        ForEach(picking.apps, id: \.id) { app in
                            Text(app.name).tag(app.id)
                        }
                    }
                    .frame(width: 320)
                }
                TextField("検索", text: $model.menuQuery)
                    .textFieldStyle(.roundedBorder)
                list(loading: picking.loading)
            } else {
                Text(picking?.notRunning ?? "")
                    .foregroundColor(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 320)
            }
            HStack {
                Spacer()
                Button("キャンセル") { model.endPickingMenu(nil) }
                    .keyboardShortcut(.cancelAction)
                Button("ショートカットを送る") { model.endPickingMenu(.keys) }
                    .disabled(selected?.caps.isEmpty ?? true)
                Button("メニュー項目を実行") { model.endPickingMenu(.item) }
                    .buttonStyle(.borderedProminent)
                    .disabled(selected == nil)
            }
        }
        .padding(24)
        .frame(width: 560)
    }

    @ViewBuilder
    private func list(loading: Bool) -> some View {
        if loading {
            ProgressView().frame(maxWidth: .infinity, minHeight: 320)
        } else if model.menuLines.isEmpty {
            Text("見つかりません")
                .foregroundColor(.secondary)
                .frame(maxWidth: .infinity, minHeight: 320)
        } else {
            List(model.menuLines, id: \.item, selection: $model.menuItem) { line in
                HStack(spacing: 12) {
                    Text(line.path).lineLimit(1)
                    Spacer()
                    if !line.caps.isEmpty {
                        KeycapsView(caps: line.caps, size: .small)
                    }
                }
                .padding(.vertical, 2)
            }
            .listStyle(.bordered(alternatesRowBackgrounds: false))
            .frame(minHeight: 320)
        }
    }
}
