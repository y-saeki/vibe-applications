import ServiceManagement

/// Launching winwin at login, as a login item of the app itself.
enum Autostart {
    static var isEnabled: Bool {
        SMAppService.mainApp.status == .enabled
    }

    /// Switches it on or off, or says why it could not be.
    static func set(_ on: Bool) -> String? {
        do {
            if on {
                try SMAppService.mainApp.register()
            } else {
                try SMAppService.mainApp.unregister()
            }
            return nil
        } catch {
            return "ログイン時の起動を\(on ? "設定" : "解除")できませんでした。\(error.localizedDescription)"
        }
    }
}
