import HomeShelves
import SwiftUI

/// Chooses which shelves the home screen shows, and in what order. Opened
/// from Settings.
struct ShelfEditorView: View {
    @Environment(ShelfSettingsStore.self) private var shelfSettings

    var body: some View {
        List {
            Section {
                ForEach(shelfSettings.preferences.entries, id: \.shelf) { entry in
                    // A button of its own rather than the list's selection:
                    // dragging one selected row drags every selected row with it.
                    Button {
                        shelfSettings.setShown(!entry.isShown, for: entry.shelf)
                    } label: {
                        HStack(spacing: 12) {
                            Image(systemName: entry.isShown ? "checkmark.circle.fill" : "circle")
                                .font(.title3)
                                .foregroundStyle(entry.isShown ? Color.accentColor : Color.secondary)
                            Text(entry.shelf.displayName)
                                // A color, not `.primary`, which the button tints.
                                .foregroundStyle(Color(uiColor: .label))
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                    }
                    // Borderless, so that the button still takes taps while
                    // the list stays in edit mode for the reorder handles.
                    .buttonStyle(.borderless)
                    .accessibilityAddTraits(entry.isShown ? .isSelected : [])
                }
                .onMove { shelfSettings.move(fromOffsets: $0, toOffset: $1) }
            } footer: {
                Text("チェックを付けた棚がホームに出ます。右端をドラッグすると並べ替えられます。")
            }

            Section {
                Button("最初の状態に戻す") { shelfSettings.reset() }
                    .buttonStyle(.borderless)
                    .disabled(shelfSettings.preferences == .default)
            }
        }
        // Always reorderable: the list exists to be rearranged.
        .environment(\.editMode, .constant(.active))
        .navigationTitle("表示する棚")
        .navigationBarTitleDisplayMode(.inline)
    }
}
