import SwiftUI

/// One thread on the strip. Until its body comes in it shows Gmail's snippet.
struct CardView: View {
    let card: Card
    let model: AppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            head
                .padding(16)
            Divider()
            if model.asksForRemoteImages(card) {
                RemoteImagesBar(card: card, model: model)
                Divider()
            }
            // The only thing in a card that scrolls; never sideways.
            ScrollView(.vertical) {
                MessageBodyView(card: card, model: model)
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
                if model.showsAccounts {
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

/// What a card's body shows: the message once it is here, the snippet until then.
private struct MessageBodyView: View {
    let card: Card
    let model: AppModel
    @State private var height: CGFloat = 1

    var body: some View {
        switch model.bodies[card.bodyKey]?.body {
        case .html(let html, _):
            MessageWebView(
                html: html,
                allowRemoteImages: model.showsRemoteImages(card),
                openLink: { model.openLink($0) },
                height: $height
            )
            .frame(height: height)
        case .text(let text):
            Text(MessageText.attributed(text, isOpenable: AppModel.isOpenableLink))
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
                .environment(\.openURL, OpenURLAction { url in
                    model.openLink(url)
                    return .handled
                })
        case .empty:
            Text("本文はありません")
                .foregroundStyle(.secondary)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
        case nil:
            Text(card.snippet)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(16)
        }
    }
}

/// Offers the images from the network a message holds back.
private struct RemoteImagesBar: View {
    let card: Card
    let model: AppModel
    @State private var saving = false

    var body: some View {
        HStack(spacing: 12) {
            Text("外部の画像を読み込んでいません")
            Spacer(minLength: 8)
            Button("表示する") { model.showRemoteImages(card) }
            if !card.from.email.isEmpty {
                Button("この差出人は常に表示") {
                    saving = true
                    Task {
                        await model.alwaysShowImages(from: card)
                        saving = false
                    }
                }
                .disabled(saving)
                .help("\(card.from.email) からのメールでは、外部の画像を常に読み込みます")
            }
        }
        .buttonStyle(.link)
        .font(.caption)
        .foregroundStyle(.secondary)
        .padding(.horizontal, 16)
        .padding(.vertical, 6)
    }
}
