import HomeShelves
import SwiftUI

/// Chooses which shelves the home screen shows, and in what order.
struct ShelfEditorView: View {
    @Environment(ShelfSettingsStore.self) private var shelfSettings
    @Environment(\.dismiss) private var dismiss

    private var shownShelves: Binding<Set<HomeShelf>> {
        Binding(
            get: { Set(shelfSettings.shownShelves) },
            set: { shown in
                for entry in shelfSettings.preferences.entries where shown.contains(entry.shelf) != entry.isShown {
                    shelfSettings.setShown(!entry.isShown, for: entry.shelf)
                }
            }
        )
    }

    var body: some View {
        // The list's own selection marks the shelves shown: in edit mode a
        // control inside a row takes no tap, and the marks sit beside the
        // reorder handles without one.
        List(selection: shownShelves) {
            Section {
                ForEach(shelfSettings.preferences.entries, id: \.shelf) { entry in
                    Text(entry.shelf.displayName)
                        // The mark says enough; a shown shelf is not to look pressed.
                        .listRowBackground(Color(uiColor: .secondarySystemGroupedBackground))
                }
                .onMove { shelfSettings.move(fromOffsets: $0, toOffset: $1) }
            } footer: {
                Text("チェックを付けた棚がホームに出ます。右端をドラッグすると並べ替えられます。")
            }
        }
        // Always reorderable: the list exists to be rearranged.
        .environment(\.editMode, .constant(.active))
        .navigationTitle("棚を編集")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .confirmationAction) {
                Button("完了") { dismiss() }
            }
            // In the bar rather than the list, where edit mode keeps a button from taking taps.
            ToolbarItem(placement: .bottomBar) {
                Button("最初の状態に戻す") { shelfSettings.reset() }
                    .disabled(shelfSettings.preferences == .default)
            }
        }
    }
}
