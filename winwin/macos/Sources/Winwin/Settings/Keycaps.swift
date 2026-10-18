import SwiftUI

/// How large keycaps are drawn.
struct CapSize {
    let height: CGFloat
    let minWidth: CGFloat
    let text: CGFloat
    let radius: CGFloat
    /// Inside a cap, at either side of its text.
    let padding: CGFloat
    /// Between caps, and around the plus and the comma between them.
    let spacing: CGFloat

    /// The recording dialog's, big enough to read at a glance.
    static let large = CapSize(height: 56, minWidth: 64, text: 18, radius: 6, padding: 10, spacing: 12)
    /// The shortcut field's, and the dialog's when large ones do not fit.
    static let medium = CapSize(height: 32, minWidth: 36, text: 14, radius: 4, padding: 10, spacing: 4)
    /// The list's, narrow enough that four modifiers and two keys fit beside
    /// the placement columns.
    static let small = CapSize(height: 24, minWidth: 24, text: 12, radius: 4, padding: 6, spacing: 3)
}

/// Strokes one after another as keycaps, with commas between them as the
/// file writes them and a plus between the modifiers and the key; the
/// second without the modifiers it shares with the first.
struct KeysView: View {
    let keys: [[Cap]]
    let size: CapSize

    var body: some View {
        HStack(spacing: size.spacing) {
            ForEach(Array(keys.enumerated()), id: \.offset) { i, group in
                if i > 0 {
                    Text(",").font(.system(size: size.text))
                }
                ForEach(Array(group.enumerated()), id: \.offset) { _, cap in
                    switch cap {
                    case .plus:
                        Text("+").font(.system(size: size.text))
                    default:
                        KeycapView(cap: cap, size: size)
                    }
                }
            }
        }
    }
}

/// One key, drawn in the accent color as macOS's own settings draw keys.
/// The caps are pictures, not buttons: they do nothing when clicked.
struct KeycapView: View {
    let cap: Cap
    let size: CapSize

    var body: some View {
        face
            .foregroundColor(.white)
            .padding(.horizontal, size.padding)
            .frame(minWidth: size.minWidth, minHeight: size.height, maxHeight: size.height)
            .background(RoundedRectangle(cornerRadius: size.radius).fill(Color.accentColor))
    }

    @ViewBuilder private var face: some View {
        switch cap {
        case let .text(label):
            Text(label).font(.system(size: size.text, weight: .medium))
        case let .arrow(direction):
            Image(systemName: direction.symbol).font(.system(size: size.text * 0.8, weight: .bold))
        case .plus:
            // KeysView draws it between the caps, not on one.
            EmptyView()
        }
    }
}

extension Direction {
    var symbol: String {
        switch self {
        case .left: return "arrow.left"
        case .up: return "arrow.up"
        case .right: return "arrow.right"
        case .down: return "arrow.down"
        }
    }
}

/// A row's shortcut as keycaps, or 未設定.
struct ShortcutView: View {
    let keys: [[Cap]]
    let size: CapSize

    var body: some View {
        if keys.isEmpty {
            Text("未設定").foregroundColor(.secondary)
        } else {
            KeysView(keys: keys, size: size)
        }
    }
}
