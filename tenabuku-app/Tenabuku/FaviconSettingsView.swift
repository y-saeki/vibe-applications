import HatenaUnofficial
import SwiftUI

/// Turns the icon services on and off and orders them. Always in edit mode,
/// so the drag handles show that the order can be changed.
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
                    // The gap Safari's Start Page leaves between a switch and
                    // the drag handle; without it the two read as one control.
                    .padding(.trailing, 12)
                }
                .onMove { settings.move(fromOffsets: $0, toOffset: $1) }
            } footer: {
                Text("上から順に問い合わせ、最初に見つかったアイコンを使います。どこにもないときは、地球の記号を出します。並べ替えは、右端をドラッグします。")
            }

            Section {
                Button("最初の設定に戻す") { settings.reset() }
                    .disabled(settings.preferences == .default)
            }
        }
        .environment(\.editMode, .constant(.active))
        .navigationTitle("サイトのアイコン")
        .navigationBarTitleDisplayMode(.inline)
    }
}
