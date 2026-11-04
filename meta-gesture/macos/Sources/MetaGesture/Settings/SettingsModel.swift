import AppKit
import Combine
import UniformTypeIdentifiers

/// What failed, and why: the banner above everything.
struct Problem: Equatable {
    let title: String
    let message: String
}

/// An application that can be added to a set.
struct Candidate: Hashable {
    let id: String
    let name: String
}

/// The menu dialog: which line it is for, the running applications it can
/// list the menus of, and the one it lists.
struct MenuPicking {
    let line: Int
    let apps: [Candidate]
    var app: String?
    /// What the dialog says when none of the applications is running.
    let notRunning: String
    /// The menus are being read.
    var loading: Bool
}

/// What a line takes from the chosen menu item.
enum MenuUse {
    case keys
    case item
}

/// What the settings window shows, read back from the library's editor after
/// every change. The window reports clicks and typing here and draws what
/// comes back; what a change does is the editor's to decide.
@MainActor
final class SettingsModel: ObservableObject {
    let editor: SettingsEditor
    let recorder = KeyRecorder()
    private let path: String

    @Published private(set) var sets: [SetEntry] = []
    @Published private(set) var selectedSet: Int = 0
    @Published private(set) var isCommon = true
    @Published private(set) var setName = ""
    @Published private(set) var apps: [AppEntry] = []
    @Published private(set) var commands: [CommandEntry] = []
    @Published private(set) var selectedCommand: Int?
    @Published private(set) var commandName = ""
    @Published private(set) var gesture = ""
    @Published private(set) var conflict: String?
    @Published private(set) var keys: [KeyEntry] = []
    @Published var autostart: Bool
    private let autostartWas: Bool
    /// Shown above everything when something fails.
    @Published var error: Problem?
    /// The 変更を保存せずに閉じますか? dialog is open.
    @Published var confirming = false
    /// The recording dialog is open for the line of keys `.0`, with the caps
    /// recorded so far.
    @Published private(set) var recording: (line: Int, caps: [String], complete: Bool)?

    /// The menu dialog is open for the line of keys `line`, choosing from the
    /// menu bar of one of `apps`.
    @Published private(set) var menuPicking: MenuPicking?
    /// The menu dialog's search, which starts out as the command's name.
    @Published var menuQuery = "" {
        didSet { searchMenus() }
    }
    @Published private(set) var menuLines: [MenuLine] = []
    /// The menu dialog's selected item, by its place in the catalog.
    @Published var menuItem: UInt32?
    private var catalog: MenuCatalog?

    /// The window asks to close itself; whoever opened it does the closing.
    var onClose: () -> Void = {}

    /// Reads the config file, or throws why it cannot.
    init(path: String) throws {
        self.path = path
        editor = try SettingsEditor.open(path: path)
        let autostart = Autostart.isEnabled
        self.autostart = autostart
        autostartWas = autostart
        refresh()
    }

    /// Reads everything back from the editor.
    private func refresh() {
        sets = editor.setLines()
        selectedSet = Int(editor.selectedSet())
        isCommon = editor.isCommon()
        setName = editor.setName()
        apps = editor.apps()
        commands = editor.commandLines()
        selectedCommand = editor.selectedCommand().map(Int.init)
        commandName = editor.commandName() ?? ""
        gesture = editor.gesture()
        conflict = editor.gestureConflict()
        keys = editor.keyLines()
    }

    private func change(_ f: () -> Void) {
        f()
        refresh()
    }

    // MARK: Sets

    func selectSet(_ line: Int) { change { editor.selectSet(line: UInt32(line)) } }
    func addSet() { change { editor.addSet() } }
    var presets: [String] { editor.presets() }
    /// Adds a set made from preset `index`, with the running applications
    /// it knows.
    func addPreset(_ index: Int) {
        let apps = running.map { AppEntry(id: $0.id, name: $0.name) }
        change { editor.addPreset(index: UInt32(index), running: apps) }
    }
    func deleteSet() { change { editor.deleteSet() } }
    func canMoveSet(up: Bool) -> Bool { editor.canMoveSet(up: up) }
    func moveSet(up: Bool) { change { editor.moveSet(up: up) } }
    func renameSet(_ name: String) { change { editor.renameSet(name: name) } }

    // MARK: Applications

    /// The applications running now that show in the Dock, but for
    /// meta-gesture itself, by name.
    var running: [Candidate] {
        let own = Bundle.main.bundleIdentifier
        var seen = Set<String>()
        return NSWorkspace.shared.runningApplications
            .filter { $0.activationPolicy == .regular }
            .compactMap { app -> Candidate? in
                guard let id = app.bundleIdentifier, id != own, seen.insert(id).inserted else { return nil }
                return Candidate(id: id, name: app.localizedName ?? id)
            }
            .sorted { $0.name.localizedStandardCompare($1.name) == .orderedAscending }
    }

    func add(_ app: Candidate) {
        change {
            if let why = editor.addApp(id: app.id, name: app.name) {
                error = Problem(title: "アプリを追加できませんでした", message: why)
            }
        }
    }

    /// 選択...: asks for an application in the Finder.
    func browse() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.application]
        panel.directoryURL = URL(fileURLWithPath: "/Applications")
        guard panel.runModal() == .OK, let url = panel.url else { return }
        guard let id = Bundle(url: url)?.bundleIdentifier else {
            error = Problem(title: "アプリを追加できませんでした", message: "\(url.lastPathComponent) はアプリとして使えません")
            return
        }
        let name = FileManager.default.displayName(atPath: url.path)
            .replacingOccurrences(of: ".app", with: "")
        add(Candidate(id: id, name: name))
    }

    func removeApp(_ line: Int) { change { editor.removeApp(line: UInt32(line)) } }

    // MARK: Commands

    func selectCommand(_ line: Int) { change { editor.selectCommand(line: UInt32(line)) } }
    func addCommand() { change { editor.addCommand() } }
    func deleteCommand() { change { editor.deleteCommand() } }
    func canMoveCommand(up: Bool) -> Bool { editor.canMoveCommand(up: up) }
    func moveCommand(up: Bool) { change { editor.moveCommand(up: up) } }
    func renameCommand(_ name: String) { change { editor.renameCommand(name: name) } }
    /// 0 left, 1 up, 2 right, 3 down.
    func push(_ direction: Int) { change { editor.pushDirection(direction: UInt8(direction)) } }
    func pop() { change { editor.popDirection() } }
    func clearGesture() { change { editor.clearGesture() } }
    func clearKeys(_ line: Int) { change { editor.clearKeys(line: UInt32(line)) } }

    // MARK: Recording

    var isRecording: Bool { recording != nil }

    func startRecording(_ line: Int) {
        editor.startRecording(line: UInt32(line), recorder: recorder)
        recording = (line, recorder.caps(), recorder.isComplete())
    }

    /// A key event while the dialog is open, as the window hears it. Its
    /// modifier flags have the same bits as an event tap's.
    func record(_ event: NSEvent) {
        switch event.type {
        case .keyDown: recorder.keyDown(code: event.keyCode)
        case .keyUp: recorder.keyUp(code: event.keyCode)
        case .flagsChanged: recorder.flagsChanged(flags: UInt64(event.modifierFlags.rawValue))
        default: return
        }
        if let line = recording?.line {
            recording = (line, recorder.caps(), recorder.isComplete())
        }
    }

    func clearRecording() {
        recorder.clear()
        if let line = recording?.line {
            recording = (line, recorder.caps(), recorder.isComplete())
        }
    }

    /// The dialog closed with 決定 (true) or キャンセル.
    func endRecording(keep: Bool) {
        guard let line = recording?.line else { return }
        recording = nil
        if keep {
            change { editor.takeRecording(line: UInt32(line), recorder: recorder) }
        }
    }

    // MARK: Menus

    /// 'メニューから選ぶ...' on the line of keys `line`: the applications whose
    /// menus can be chosen from are the line's, or for the first line any
    /// of the set's, of those that are running.
    func startPickingMenu(_ line: Int) {
        let candidates: [Candidate]
        if line > 0 {
            guard line - 1 < apps.count else { return }
            candidates = [Candidate(id: apps[line - 1].id, name: apps[line - 1].name)]
        } else if isCommon {
            candidates = running
        } else {
            candidates = apps.map { Candidate(id: $0.id, name: $0.name) }
        }
        let open = candidates.filter { !NSRunningApplication.runningApplications(withBundleIdentifier: $0.id).isEmpty }
        let notRunning: String
        if line > 0 {
            notRunning = "\(apps[line - 1].name) が起動していません"
        } else {
            notRunning = "セットのアプリが起動していません"
        }
        menuPicking = MenuPicking(line: line, apps: open, app: open.first?.id, notRunning: notRunning, loading: false)
        menuQuery = commandName
        readMenus()
    }

    /// Chooses whose menus the dialog lists.
    func pickMenuApp(_ id: String) {
        guard menuPicking != nil, menuPicking?.app != id else { return }
        menuPicking?.app = id
        readMenus()
    }

    /// Reads the menus of the dialog's application, off the main thread, as
    /// the accessibility API asks the application for every item.
    private func readMenus() {
        catalog = nil
        menuLines = []
        menuItem = nil
        guard let id = menuPicking?.app,
              let pid = NSRunningApplication.runningApplications(withBundleIdentifier: id).first?.processIdentifier
        else { return }
        menuPicking?.loading = true
        Task {
            let entries = await withCheckedContinuation { (done: CheckedContinuation<[MenuEntry], Never>) in
                DispatchQueue.global(qos: .userInitiated).async {
                    done.resume(returning: Menus.entries(of: pid))
                }
            }
            // Another application may have been chosen meanwhile.
            guard menuPicking?.app == id else { return }
            menuPicking?.loading = false
            catalog = MenuCatalog(entries: entries)
            searchMenus()
        }
    }

    private func searchMenus() {
        menuLines = catalog?.search(query: menuQuery) ?? []
        if let item = menuItem, !menuLines.contains(where: { $0.item == item }) {
            menuItem = nil
        }
    }

    /// The dialog closed: with the chosen item's shortcut as the line's keys
    /// (`.keys`), with the item itself (`.item`), or with nothing.
    func endPickingMenu(_ use: MenuUse?) {
        guard let line = menuPicking?.line else { return }
        menuPicking = nil
        defer { catalog = nil }
        guard let use, let item = menuItem, let catalog else { return }
        change {
            switch use {
            case .keys: editor.useMenuKeys(line: UInt32(line), catalog: catalog, item: item)
            case .item: editor.useMenuItem(line: UInt32(line), catalog: catalog, item: item)
            }
        }
    }

    // MARK: Closing

    var isChanged: Bool {
        editor.isChanged() || autostart != autostartWas
    }

    func save() {
        if let e = editor.save(path: path) {
            refresh()
            error = Problem(title: "保存できませんでした", message: e)
            return
        }
        if autostart != autostartWas, let e = Autostart.set(autostart) {
            error = Problem(title: "保存できませんでした", message: e)
            return
        }
        onClose()
    }

    // MARK: Export and import

    private static let types = [UTType(filenameExtension: "toml") ?? .data]

    /// エクスポート: asks where to, then writes the settings as they stand,
    /// checked as saving checks them.
    func export() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = Self.types
        panel.nameFieldStringValue = "meta-gesture.toml"
        guard panel.runModal() == .OK, let url = panel.url else { return }
        if let e = editor.exportFile(path: url.path) {
            refresh()
            error = Problem(title: "エクスポートできませんでした", message: e)
        }
    }

    /// インポート: asks for a file, then puts it in place of the settings,
    /// unsaved.
    func importFile() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = Self.types
        panel.allowsOtherFileTypes = true
        guard panel.runModal() == .OK, let url = panel.url else { return }
        if let e = editor.importFile(path: url.path) {
            error = Problem(title: "インポートできませんでした", message: e)
            return
        }
        error = nil
        refresh()
    }

    /// キャンセル, or the window's close button.
    func cancel() {
        if isRecording || menuPicking != nil { return }
        if isChanged {
            confirming = true
        } else {
            onClose()
        }
    }
}
