import AppKit
import SwiftUI

/// The menu bar item and its menu: the list on top, then the settings and
/// Quit.
@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuDelegate {
    private let model = Model()
    private var statusItem: NSStatusItem?
    private var autostartItem: NSMenuItem?
    private var networkItem: NSMenuItem?
    private var logMemoryItem: NSMenuItem?

    func applicationDidFinishLaunching(_ notification: Notification) {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
        if let button = item.button {
            let image = NSImage(systemSymbolName: "waveform.path.ecg", accessibilityDescription: "myaku")
            image?.isTemplate = true
            button.image = image
        }
        item.menu = makeMenu()
        statusItem = item
    }

    private func makeMenu() -> NSMenu {
        let menu = NSMenu()
        menu.delegate = self

        let list = NSMenuItem()
        let host = NSHostingView(rootView: ContentView(model: model))
        host.frame = NSRect(x: 0, y: 0, width: ContentView.width, height: ContentView.height)
        list.view = host
        menu.addItem(list)
        menu.addItem(.separator())

        let autostart = NSMenuItem(title: "ログイン時に開く", action: #selector(toggleAutostart), keyEquivalent: "")
        autostart.target = self
        menu.addItem(autostart)
        autostartItem = autostart

        let network = NSMenuItem(title: "ネットワークの使用量を測る", action: #selector(toggleNetwork), keyEquivalent: "")
        network.target = self
        menu.addItem(network)
        networkItem = network

        let logMemory = NSMenuItem(title: "メモリを対数で表示", action: #selector(toggleLogMemory), keyEquivalent: "")
        logMemory.target = self
        menu.addItem(logMemory)
        logMemoryItem = logMemory

        menu.addItem(.separator())
        let quit = NSMenuItem(title: "myaku を終了", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        menu.addItem(quit)
        return menu
    }

    func menuWillOpen(_ menu: NSMenu) {
        autostartItem?.state = Autostart.isEnabled ? .on : .off
        networkItem?.state = model.network ? .on : .off
        logMemoryItem?.state = model.memoryScale == .log ? .on : .off
        model.visible = true
    }

    func menuDidClose(_ menu: NSMenu) {
        model.visible = false
    }

    @objc private func toggleAutostart() {
        if let problem = Autostart.set(!Autostart.isEnabled) {
            let alert = NSAlert()
            alert.messageText = problem
            NSApp.activate(ignoringOtherApps: true)
            alert.runModal()
        }
    }

    @objc private func toggleLogMemory() {
        model.setMemoryScale(model.memoryScale == .log ? .linear : .log)
        logMemoryItem?.state = model.memoryScale == .log ? .on : .off
    }

    @objc private func toggleNetwork() {
        model.setNetwork(!model.network)
    }
}
