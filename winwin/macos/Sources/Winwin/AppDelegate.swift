import AppKit

final class AppDelegate: NSObject, NSApplicationDelegate {
    private let resident = Resident()
    private lazy var tap = KeyTap(resident: resident)
    private var statusItem: NSStatusItem?
    /// Watches for the Accessibility permission while it is not granted.
    private var trustTimer: Timer?

    /// `~/Library/Application Support/winwin/config.toml`.
    static let configURL = FileManager.default
        .urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        .appendingPathComponent("winwin")
        .appendingPathComponent("config.toml")

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = makeStatusItem()
        loadConfig()
        if Permission.isTrusted {
            startTap()
        } else {
            Permission.ask()
            trustTimer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in
                guard Permission.isTrusted else { return }
                self?.trustTimer?.invalidate()
                self?.trustTimer = nil
                self?.startTap()
            }
        }
    }

    private func startTap() {
        if !tap.start() {
            alert("ショートカットを受け取れませんでした")
        }
    }

    private func loadConfig() {
        if let problem = resident.load(path: Self.configURL.path) {
            alert("設定ファイルを読み込めませんでした", detail: problem)
        }
    }

    private func makeStatusItem() -> NSStatusItem {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        item.button?.image = Icon.image(points: 18)
        item.button?.toolTip = "winwin"
        let menu = NSMenu()
        menu.addItem(withTitle: "設定ファイルを開く", action: #selector(openConfig), keyEquivalent: "")
        menu.addItem(withTitle: "設定を読み込み直す", action: #selector(reloadConfig), keyEquivalent: "")
        menu.addItem(.separator())
        menu.addItem(withTitle: "終了", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        for menuItem in menu.items where menuItem.action != #selector(NSApplication.terminate(_:)) {
            menuItem.target = self
        }
        item.menu = menu
        return item
    }

    @objc private func openConfig() {
        NSWorkspace.shared.open(Self.configURL)
    }

    @objc private func reloadConfig() {
        loadConfig()
    }

    private func alert(_ message: String, detail: String = "") {
        let alert = NSAlert()
        alert.messageText = message
        alert.informativeText = detail
        NSApp.activate(ignoringOtherApps: true)
        alert.runModal()
    }
}
