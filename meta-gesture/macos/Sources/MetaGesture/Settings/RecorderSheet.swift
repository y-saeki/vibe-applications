import SwiftUI

/// The dialog that records keys: it shows them as they are pressed. While it
/// is open, every key pressed in meta-gesture comes here (see
/// SettingsWindow), so its buttons are clicked rather than pressed, and
/// none of them takes the keyboard focus.
struct RecorderSheet: View {
    @ObservedObject var model: SettingsModel

    var body: some View {
        let caps = model.recording?.caps ?? []
        VStack(alignment: .leading, spacing: 16) {
            Text("送るキーを設定").font(.title3).bold()
            Group {
                if caps.isEmpty {
                    Text("キーを押してください").foregroundColor(.secondary)
                } else {
                    KeycapsView(caps: caps, size: .large)
                }
            }
            .frame(maxWidth: .infinity, minHeight: 112)
            .background(RoundedRectangle(cornerRadius: 8).fill(Color(nsColor: .controlBackgroundColor)))
            .overlay(RoundedRectangle(cornerRadius: 8).stroke(Color(nsColor: .separatorColor)))
            Button("クリア") { model.clearRecording() }
                .buttonStyle(.link)
                .focusable(false)
                .frame(maxWidth: .infinity)
            HStack {
                Spacer()
                Button("キャンセル") { model.endRecording(keep: false) }
                    .focusable(false)
                Button("決定") { model.endRecording(keep: true) }
                    .buttonStyle(.borderedProminent)
                    .focusable(false)
                    .disabled(!(model.recording?.complete ?? false))
            }
        }
        .padding(24)
        .frame(width: 448)
    }
}

/// How large keycaps are drawn.
struct CapSize {
    let height: CGFloat
    let minWidth: CGFloat
    let text: CGFloat
    let radius: CGFloat
    let padding: CGFloat
    let spacing: CGFloat

    /// The recording dialog's, big enough to read at a glance.
    static let large = CapSize(height: 56, minWidth: 64, text: 18, radius: 6, padding: 10, spacing: 12)
    /// A line of keys'.
    static let small = CapSize(height: 24, minWidth: 24, text: 12, radius: 4, padding: 6, spacing: 3)
}

/// Keys drawn in the accent color as macOS's own settings draw them. The
/// caps are pictures, not buttons.
struct KeycapsView: View {
    let caps: [String]
    let size: CapSize

    var body: some View {
        HStack(spacing: size.spacing) {
            ForEach(Array(caps.enumerated()), id: \.offset) { _, cap in
                Text(cap)
                    .font(.system(size: size.text, weight: .medium))
                    .foregroundColor(.white)
                    .padding(.horizontal, size.padding)
                    .frame(minWidth: size.minWidth, minHeight: size.height, maxHeight: size.height)
                    .background(RoundedRectangle(cornerRadius: size.radius).fill(Color.accentColor))
            }
        }
    }
}
