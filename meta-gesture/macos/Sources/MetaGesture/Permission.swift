import ApplicationServices

/// The Accessibility permission, without which macOS lets meta-gesture
/// neither follow the mouse in other applications nor send them keys.
enum Permission {
    static var isTrusted: Bool {
        AXIsProcessTrusted()
    }

    /// Shows macOS's own request, which lists meta-gesture in System Settings
    /// and offers to open the pane where it is switched on.
    static func ask() {
        let prompt = kAXTrustedCheckOptionPrompt.takeUnretainedValue() as String
        _ = AXIsProcessTrustedWithOptions([prompt: true] as CFDictionary)
    }
}
