import AppKit

/// The resident part: the menu bar item, and who the keys go to. Normally
/// that is the config's shortcuts. While the settings window is open they
/// are let go of, so that pressing one while editing does not move the
/// settings window about; the test window's shortcuts, or the recording
/// dialog, take the keys instead.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let resident = Resident()
    /// The shortcuts the test window answers to: the rows as they stand.
    private let trial = Resident()
    private let tap = KeyTap()
    private var statusItem: NSStatusItem?
    /// Watches for the Accessibility permission while it is not granted.
    private var trustTimer: Timer?
    private var settings: SettingsWindow?
    private var testWindow: TestWindow?

    /// `~/Library/Application Support/winwin/config.toml`.
    static let configURL = FileManager.default
        .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("winwin")
        .appendingPathComponent("config.toml")

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = makeMainMenu()
        statusItem = makeStatusItem()
        if let problem = resident.load(path: Self.configURL.path) {
            alert(problem, detail: "ショートカットは使えません。設定を開いて保存し直すか、ファイルを修正してください。")
        }
        applyTheme(resident.theme())
        routeKeys()
        if Permission.isTrusted {
            startTap()
        } else {
            Permission.ask()
            trustTimer = Timer.scheduledTimer(
                timeInterval: 1, target: self, selector: #selector(checkTrust), userInfo: nil, repeats: true
            )
        }
    }

    /// Starts taking keys once the permission is granted.
    @objc private func checkTrust() {
        guard Permission.isTrusted else { return }
        trustTimer?.invalidate()
        trustTimer = nil
        startTap()
    }

    /// Opening winwin again while it runs opens the settings.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        openSettings()
        return false
    }

    private func startTap() {
        if !tap.start() {
            alert("ショートカットを受け取れませんでした")
        }
    }

    // MARK: Keys

    /// Sends the keys to whoever takes them now.
    private func routeKeys() {
        if let settings {
            let model = settings.model
            if model.isRecording {
                tap.handler = { type, event in
                    // Only the keys typed into winwin's own windows.
                    guard NSApp.isActive else { return false }
                    model.record(type, event)
                    return true
                }
            } else if testWindow != nil {
                tap.handler = { [weak self] type, event in
                    self?.press(type, event, on: self?.trial) { result in
                        switch result {
                        case let .place(placement): self?.testWindow?.place(placement)
                        case .reset: self?.testWindow?.reset()
                        case .pass, .take: break
                        }
                    } ?? false
                }
            } else {
                tap.handler = { _, _ in false }
            }
        } else {
            tap.handler = { [weak self] type, event in
                guard let resident = self?.resident else { return false }
                return self?.press(type, event, on: resident) { result in
                    switch result {
                    case let .place(placement): Mover.place(placement, resident: resident)
                    case .reset: Mover.reset(resident: resident)
                    case .pass, .take: break
                    }
                } ?? false
            }
        }
    }

    /// Hands a key event to `shortcuts`, and what it says to do to `act`
    /// once the tap has answered: moving a window can be slow. True keeps
    /// the event from the application.
    private func press(
        _ type: CGEventType,
        _ event: CGEvent,
        on shortcuts: Resident?,
        act: @escaping (KeyResult) -> Void
    ) -> Bool {
        guard let shortcuts else { return false }
        switch type {
        case .flagsChanged:
            shortcuts.flagsChanged(flags: event.flags.rawValue)
            return false
        case .keyDown:
            let result = shortcuts.keyDown(code: event.keyCode, flags: event.flags.rawValue, held: event.isRepeat)
            if case .pass = result { return false }
            Task { @MainActor in act(result) }
            return true
        default:
            return false
        }
    }

    // MARK: Settings

    @objc private func openSettings() {
        if let settings {
            settings.show()
            return
        }
        var problem: String?
        guard let model = SettingsModel(path: Self.configURL.path, work: workSize(), problem: &problem) else {
            alert("設定を開けませんでした", detail: problem ?? "")
            return
        }
        let window = SettingsWindow(model: model, path: Self.configURL.path)
        model.onClose = { [weak self] in self?.closeSettings() }
        model.onChange = { [weak self] in self?.settingsChanged() }
        model.onTest = { [weak self] on in self?.setTesting(on) }
        model.onTheme = { [weak self] theme in self?.applyTheme(theme) }
        settings = window
        applyTheme(model.editor.theme())
        routeKeys()
        window.show()
    }

    /// The settings window has closed, saved or not. The file is the truth
    /// either way, so it is read again.
    private func closeSettings() {
        setTesting(false)
        settings?.close()
        settings = nil
        if let problem = resident.load(path: Self.configURL.path) {
            alert(problem)
        }
        applyTheme(resident.theme())
        routeKeys()
    }

    /// The rows changed, or recording started or stopped.
    private func settingsChanged() {
        if let model = settings?.model {
            trial.useTrial(editor: model.editor)
        }
        routeKeys()
    }

    private func setTesting(_ on: Bool) {
        if on, testWindow == nil {
            let window = TestWindow { [weak self] in
                self?.settings?.model.setTesting(false)
            }
            testWindow = window
            window.show()
        } else if !on, let window = testWindow {
            window.close()
            testWindow = nil
        }
        settings?.setAbove(testWindow != nil)
        routeKeys()
    }

    /// The size of the visible frame of the screen the pointer is on, which
    /// previews are drawn to the shape of. That is where the window opens.
    private func workSize() -> (width: Int32, height: Int32) {
        let mouse = NSEvent.mouseLocation
        let screen = NSScreen.screens.first { NSMouseInRect(mouse, $0.frame, false) } ?? NSScreen.main
        let visible = screen?.visibleFrame.size ?? NSSize(width: 16, height: 9)
        return (Int32(visible.width), Int32(visible.height))
    }

    // MARK: Appearance

    /// The settings window and the menu light or dark as the settings say,
    /// following macOS unless told otherwise.
    private func applyTheme(_ theme: UInt8) {
        let appearance: NSAppearance?
        switch theme {
        case 1: appearance = NSAppearance(named: .aqua)
        case 2: appearance = NSAppearance(named: .darkAqua)
        default: appearance = nil
        }
        NSApp.appearance = appearance
        settings?.setAppearance(appearance)
    }

    // MARK: Menus

    private func makeStatusItem() -> NSStatusItem {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = Icon.image(points: 18)
        item.button?.toolTip = "winwin"
        let menu = NSMenu()
        let open = menu.addItem(withTitle: "設定を開く", action: #selector(openSettings), keyEquivalent: "")
        open.target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "終了", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.menu = menu
        return item
    }

    /// Never shown, winwin having no menu bar of its own, but the keys it
    /// lists work: editing in the settings window's fields, and closing the
    /// window in front.
    private func makeMainMenu() -> NSMenu {
        let main = NSMenu()
        let app = NSMenu()
        app.addItem(withTitle: "終了", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        let file = NSMenu(title: "ファイル")
        file.addItem(withTitle: "閉じる", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        let edit = NSMenu(title: "編集")
        edit.addItem(withTitle: "取り消す", action: Selector(("undo:")), keyEquivalent: "z")
        edit.addItem(withTitle: "やり直す", action: Selector(("redo:")), keyEquivalent: "Z")
        edit.addItem(.separator())
        edit.addItem(withTitle: "カット", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        edit.addItem(withTitle: "コピー", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        edit.addItem(withTitle: "ペースト", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        edit.addItem(withTitle: "すべてを選択", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        for submenu in [app, file, edit] {
            let item = NSMenuItem()
            item.submenu = submenu
            main.addItem(item)
        }
        return main
    }

    private func alert(_ message: String, detail: String = "") {
        let alert = NSAlert()
        alert.messageText = message
        alert.informativeText = detail
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }
}
