import SwiftUI

/// The inbox: one card per thread, newest at the left, across every account.
struct InboxView: View {
    let model: AppModel

    /// The Tauri app's `--card-width` and `--gap`.
    static let cardWidth: CGFloat = 570
    static let gap: CGFloat = 12

    var body: some View {
        Group {
            if model.cards.isEmpty && !model.loading {
                ContentUnavailableView(
                    "受信トレイは空です",
                    systemImage: "tray",
                    description: Text("新しいメールが届いたら、再読み込みで表示されます。")
                )
            } else {
                strip
            }
        }
        .navigationTitle("受信トレイ")
        .navigationSubtitle(model.cards.isEmpty ? "" : "\(model.cards.count) 件")
        .toolbar {
            ToolbarItem {
                if model.loading {
                    ProgressView().controlSize(.small)
                } else {
                    Button {
                        Task { await model.load() }
                    } label: {
                        Label("再読み込み", systemImage: "arrow.clockwise")
                    }
                    .help("再読み込み")
                }
            }
        }
        .overlay(alignment: .bottom) {
            if let notice = model.notice {
                NoticeView(text: notice) { model.notice = nil }
                    .padding(Self.gap)
            }
        }
    }

    private var strip: some View {
        GeometryReader { geometry in
            // Narrower only when the window cannot fit one card at its full width.
            let width = max(0, min(Self.cardWidth, geometry.size.width - 2 * Self.gap))
            ScrollView(.horizontal) {
                LazyHStack(spacing: Self.gap) {
                    ForEach(model.cards, id: \.key) { card in
                        CardView(card: card, showsAccount: model.showsAccounts)
                            .frame(width: width)
                    }
                }
                .scrollTargetLayout()
                .padding(Self.gap)
            }
            .scrollTargetBehavior(.viewAligned)
        }
    }
}

/// A notice that stays until it is closed.
struct NoticeView: View {
    let text: String
    let close: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            Text(text)
                .textSelection(.enabled)
                .frame(maxWidth: .infinity, alignment: .leading)
            Button(action: close) {
                Image(systemName: "xmark")
            }
            .buttonStyle(.borderless)
            .help("閉じる")
        }
        .padding(12)
        .frame(maxWidth: 560)
        .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 10))
        .shadow(radius: 4, y: 1)
    }
}
