import HatenaUnofficial
import SwiftUI

/// Turns the icon services on and off and orders them. The toggles do not
/// respond in edit mode, so the order is changed under the Edit button, or by
/// holding a row and dragging it.
struct FaviconSettingsView: View {
    @Environment(FaviconSettingsStore.self) private var settings

    var body: some View {
        List {
            Section {
                ForEach(settings.preferences.entries, id: \.source) { entry in
                    Toggle(isOn: Binding(
                        get: { entry.isEnabled },
                        set: { settings.setEnabled($0, for: entry.source) }
                    )) {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(entry.source.displayName)
                            Text(entry.source.sentDescription)
                                .font(.footnote)
                                .foregroundStyle(.secondary)
                        }
                    }
                }
                .onMove { settings.move(fromOffsets: $0, toOffset: $1) }
            } footer: {
                Text("上から順に問い合わせ、最初に見つかったアイコンを使います。どこにもないときは、地球の記号を出します。並べ替えは、右上の「編集」から行います。")
            }

            Section {
                Button("最初の設定に戻す") { settings.reset() }
                    .disabled(settings.preferences == .default)
            }
        }
        .toolbar { EditButton() }
        .navigationTitle("サイトのアイコン")
        .navigationBarTitleDisplayMode(.inline)
    }
}
