import SwiftUI

/// One thread on the strip. Until the body comes in (step 3 of #298) it
/// shows Gmail's snippet.
struct CardView: View {
    let card: Card
    let showsAccount: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            head
                .padding(16)
            Divider()
            ScrollView(.vertical) {
                Text(card.snippet)
                    .textSelection(.enabled)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(16)
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .background(Color(nsColor: .textBackgroundColor), in: RoundedRectangle(cornerRadius: 12))
        .shadow(color: .black.opacity(0.12), radius: 3, y: 1)
    }

    private var head: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack(spacing: 6) {
                if card.unread {
                    Circle()
                        .fill(Color.accentColor)
                        .frame(width: 9, height: 9)
                        .accessibilityLabel("未読")
                }
                Text(card.from.name.isEmpty ? card.from.email : card.from.name)
                    .fontWeight(.semibold)
                    .lineLimit(1)
                    .truncationMode(.tail)
                    .help(card.from.email)
                Spacer(minLength: 4)
                if showsAccount {
                    Badge(text: card.account)
                }
                if card.messageCount > 1 {
                    Badge(text: "\(card.messageCount) 通")
                }
                Text(CardDate.format(epochMs: card.date))
                    .foregroundStyle(.secondary)
                    .monospacedDigit()
            }
            // Two lines whether the subject needs them or not, so every head is as tall.
            Text(card.subject.isEmpty ? "(件名なし)" : card.subject)
                .font(.system(size: 17, weight: .bold))
                .lineLimit(2, reservesSpace: true)
        }
    }
}

private struct Badge: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.caption)
            .foregroundStyle(.secondary)
            .lineLimit(1)
            .truncationMode(.middle)
            .padding(.horizontal, 7)
            .frame(height: 18)
            .background(.quaternary, in: Capsule())
            .help(text)
    }
}
