import AppKit
import Combine

/// What the settings window shows, read back from the library's editor after
/// every change. The window reports clicks and typing here and draws what
/// comes back; what a change does is the editor's to decide.
@MainActor
final class SettingsModel: ObservableObject {
    let editor = SettingsEditor()
    let recorder = KeyRecorder()
    let anchors = Winwin.anchors()
    let themes = Winwin.themes()
    /// The size of the visible frame of the screen the window opened on,
    /// which previews are drawn to the shape of.
    let work: (width: Int32, height: Int32)

    @Published private(set) var rows: [Row] = []
    @Published private(set) var fields: Fields?
    @Published private(set) var preview: PlacementPreview?
    @Published private(set) var theme: UInt8 = 0
    @Published var autostart: Bool
    private let autostartWas: Bool
    /// Shown above everything when 保存して閉じる fails.
    @Published var error: String?
    /// The 変更を保存せずに閉じますか? dialog is open.
    @Published var confirming = false
    /// The recording dialog is open, with what it has recorded.
    @Published private(set) var recording: Recording?
    /// The test window is open.
    @Published private(set) var testing = false

    /// The window asks to close itself; whoever opened it does the closing.
    var onClose: () -> Void = {}
    /// The rows changed, or recording started or stopped: the test window's
    /// shortcuts follow.
    var onChange: () -> Void = {}
    /// The test window is to open (true) or close.
    var onTest: (Bool) -> Void = { _ in }
    /// The theme changed: the window and the menu follow.
    var onTheme: (UInt8) -> Void = { _ in }

    /// Reads the config file, or says why it cannot.
    init?(path: String, work: (width: Int32, height: Int32), problem: inout String?) {
        self.work = work
        let autostart = Autostart.isEnabled
        self.autostart = autostart
        autostartWas = autostart
        if let e = editor.load(path: path) {
            problem = e
            return nil
        }
        refresh()
    }

    /// Reads everything back from the editor.
    private func refresh() {
        rows = editor.rows()
        fields = editor.fields()
        theme = editor.theme()
        preview = editor.preview(
            workWidth: work.width,
            workHeight: work.height,
            width: Int32(PreviewView.size.width * PreviewView.subpixel),
            height: Int32(PreviewView.size.height * PreviewView.subpixel)
        )
        onChange()
    }

    private func change(_ f: () -> Void) {
        f()
        refresh()
    }

    var hasSelection: Bool { editor.hasSelection() }
    func canMove(up: Bool) -> Bool { editor.canMove(up: up) }

    /// Row `index` was clicked, with the modifiers held at the time.
    func click(_ index: Int) {
        let held = NSEvent.modifierFlags
        change { editor.click(index: UInt32(index), shift: held.contains(.shift), command: held.contains(.command)) }
    }

    func add() { change { editor.add() } }
    func duplicate() { change { editor.duplicate() } }
    func delete() { change { editor.delete() } }
    func move(up: Bool) { change { editor.moveSelected(up: up) } }

    /// The list was dragged: the rows at `from` now go before the row that
    /// was at `to`.
    func move(from: IndexSet, to: Int) {
        var ids = rows.map(\.id)
        let moving = from.map { ids[$0] }
        let before = ids[..<to].filter { !moving.contains($0) }.count
        ids.removeAll { moving.contains($0) }
        ids.insert(contentsOf: moving, at: before)
        change { editor.reorder(ids: ids) }
    }

    func setAnchor(_ index: Int) { change { editor.setAnchor(index: UInt8(index)) } }
    func setWidth(_ text: String) { change { editor.setWidth(text: text) } }
    func setHeight(_ text: String) { change { editor.setHeight(text: text) } }
    func setFit(_ on: Bool) { change { editor.setFit(on: on) } }
    func setReset(_ on: Bool) { change { editor.setReset(on: on) } }

    func setTheme(_ index: Int) {
        change { editor.setTheme(index: UInt8(index)) }
        onTheme(theme)
    }

    // MARK: Recording

    var isRecording: Bool { recording != nil }

    func startRecording() {
        guard fields != nil else { return }
        editor.startRecording(recorder: recorder)
        recording = recorder.recording()
        onChange()
    }

    /// A key event while the dialog is open.
    func record(_ type: CGEventType, _ event: CGEvent) {
        switch type {
        case .keyDown: recorder.keyDown(code: event.keyCode)
        case .keyUp: recorder.keyUp(code: event.keyCode)
        case .flagsChanged: recorder.flagsChanged(flags: event.flags.rawValue)
        default: return
        }
        recording = recorder.recording()
    }

    func resetRecording() {
        recorder.reset()
        recording = recorder.recording()
    }

    func clearRecording() {
        recorder.clear()
        recording = recorder.recording()
    }

    /// The dialog closed with 決定 (true) or キャンセル.
    func endRecording(keep: Bool) {
        recording = nil
        if keep {
            change { editor.takeRecording(recorder: recorder) }
        } else {
            onChange()
        }
    }

    // MARK: Test window

    func setTesting(_ on: Bool) {
        testing = on
        onTest(on)
    }

    // MARK: Closing

    var isChanged: Bool {
        editor.isChanged() || autostart != autostartWas
    }

    func save(path: String) {
        if let e = editor.save(path: path) {
            refresh()
            error = e
            return
        }
        if autostart != autostartWas, let e = Autostart.set(autostart) {
            error = e
            return
        }
        onClose()
    }

    /// キャンセル, or the window's close button.
    func cancel() {
        if isRecording { return }
        if isChanged {
            confirming = true
        } else {
            onClose()
        }
    }
}
