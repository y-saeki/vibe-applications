import AppKit

/// The resident part: the menu bar item, the event tap that follows the
/// right button, and what a gesture comes to.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let resident = Resident()
    private let tap = MouseTap()
    private let toast = Toast()
    private var statusItem: NSStatusItem?
    /// Watches for the Accessibility permission while it is not granted.
    private var trustTimer: Timer?
    private var settings: SettingsWindow?
    /// The application under the pointer when the right button went down,
    /// while the press is followed.
    private var pressed: NSRunningApplication?

    /// `~/Library/Application Support/meta-gesture/config.toml`.
    static let configURL = FileManager.default
        .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("meta-gesture")
        .appendingPathComponent("config.toml")

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.mainMenu = makeMainMenu()
        statusItem = makeStatusItem()
        if let problem = resident.load(path: Self.configURL.path) {
            alert(problem, detail: "ジェスチャーは使えません。設定を開いて保存し直すか、ファイルを修正してください。")
        }
        tap.handler = { [weak self] type, event in
            self?.handle(type, event) ?? false
        }
        if Permission.isTrusted {
            startTap()
        } else {
            Permission.ask()
            trustTimer = Timer.scheduledTimer(
                timeInterval: 1, target: self, selector: #selector(checkTrust), userInfo: nil, repeats: true
            )
        }
    }

    /// Starts following the mouse once the permission is granted.
    @objc private func checkTrust() {
        guard Permission.isTrusted else { return }
        trustTimer?.invalidate()
        trustTimer = nil
        startTap()
    }

    /// Opening meta-gesture again while it runs opens the settings.
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        openSettings()
        return false
    }

    private func startTap() {
        if !tap.start() {
            alert("マウスの操作を受け取れませんでした")
        }
    }

    // MARK: Gestures

    /// A right-button event, as the tap hears it. True keeps it from the
    /// application.
    private func handle(_ type: CGEventType, _ event: CGEvent) -> Bool {
        let at = event.location
        switch type {
        case .rightMouseDown:
            guard let app = Sender.application(at: at),
                  app.processIdentifier != ProcessInfo.processInfo.processIdentifier,
                  let id = app.bundleIdentifier,
                  resident.begin(app: id, x: at.x, y: at.y)
            else { return false }
            pressed = app
            return true
        case .rightMouseDragged:
            guard resident.isFollowing() else { return false }
            resident.moved(x: at.x, y: at.y)
            return true
        case .rightMouseUp:
            guard resident.isFollowing(), let app = pressed else { return false }
            pressed = nil
            let result = resident.ended()
            // Once the tap has answered: activating an application and
            // waiting for it takes a while.
            Task { @MainActor in self.act(result, app: app, at: at) }
            return true
        default:
            return false
        }
    }

    private func act(_ result: GestureResult, app: NSRunningApplication, at point: CGPoint) {
        switch result {
        case .click:
            Sender.clickRight(at: point)
        case .nothing:
            break
        case let .send(command, code, flags):
            toast.show(command)
            Sender.send(code: code, flags: flags, to: app)
        case let .menu(command, path):
            toast.show(command)
            Sender.choose(path, in: app) { [weak self] pressed in
                switch pressed {
                case .done: break
                case .missing: self?.toast.show("\(command): メニュー項目が見つかりません")
                case .disabled: self?.toast.show("\(command): いまは選べないメニュー項目です")
                }
            }
        case let .unassigned(command):
            toast.show("\(command): このアプリには未設定")
        case let .unsendable(command):
            toast.show("\(command): このキーは Mac のキーボードにありません")
        case let .unknown(gesture):
            toast.show("\(gesture): 未登録のジェスチャー")
        }
    }

    // MARK: Settings

    @objc private func openSettings() {
        if let settings {
            settings.show()
            return
        }
        let model: SettingsModel
        do {
            model = try SettingsModel(path: Self.configURL.path)
        } catch {
            alert("設定を開けませんでした", detail: "\(error)")
            return
        }
        let window = SettingsWindow(model: model)
        model.onClose = { [weak self] in self?.closeSettings() }
        settings = window
        window.show()
    }

    /// The settings window has closed, saved or not. The file is the truth
    /// either way, so it is read again.
    private func closeSettings() {
        settings?.close()
        settings = nil
        if let problem = resident.load(path: Self.configURL.path) {
            alert(problem)
        }
    }

    // MARK: Menus

    private func makeStatusItem() -> NSStatusItem {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = Icon.menuBarImage(points: 18)
        item.button?.toolTip = "meta-gesture"
        let menu = NSMenu()
        let open = menu.addItem(withTitle: "設定を開く", action: #selector(openSettings), keyEquivalent: "")
        open.target = self
        menu.addItem(.separator())
        menu.addItem(withTitle: "終了", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        item.menu = menu
        return item
    }

    /// Never shown, meta-gesture having no menu bar of its own, but the
    /// keys it lists work: editing in the settings window's fields, and
    /// closing the window in front.
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
