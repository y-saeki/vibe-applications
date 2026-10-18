import ApplicationServices

/// The Accessibility permission, without which macOS lets winwin neither
/// see the keys pressed in other applications nor move their windows.
enum Permission {
    static var isTrusted: Bool {
        AXIsProcessTrusted()
    }

    /// Shows macOS's own request, which lists winwin in System Settings and
    /// offers to open the pane where it is switched on.
    static func ask() {
        let prompt = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        _ = AXIsProcessTrustedWithOptions([prompt: true] as CFDictionary)
    }
}
