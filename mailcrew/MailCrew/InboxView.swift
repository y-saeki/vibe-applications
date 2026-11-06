import SwiftUI

/// The inbox: the views in a sidebar, and one card per thread, newest at the left.
struct InboxView: View {
    let model: AppModel

    var body: some View {
        NavigationSplitView {
            Sidebar(model: model)
                .navigationSplitViewColumnWidth(min: 180, ideal: 220, max: 320)
        } detail: {
            StripView(model: model)
        }
    }
}

/// The tabs: the inbox and あとで読む across every account, then each custom
/// view made in the settings. The settings open from the bottom.
struct Sidebar: View {
    let model: AppModel

    var body: some View {
        List(selection: Binding(
            get: { model.activeView },
            set: { id in
                if let id { Task { await model.select(view: id) } }
            }
        )) {
            ForEach(model.tabs) { tab in
                Label(tab.name, systemImage: tab.systemImage)
                    .lineLimit(1)
                    .tag(tab.id)
            }
        }
        .safeAreaInset(edge: .bottom) {
            SettingsLink {
                Label("設定", systemImage: "gearshape")
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
            .buttonStyle(.borderless)
            .padding(12)
        }
    }
}

/// The cards of the tab picked in the sidebar.
struct StripView: View {
    let model: AppModel
    @State private var pull = PullGesture()
    /// A pull let go past the threshold, until the reload it started ends.
    @State private var refreshing = false

    /// The gap kept open left of the first card while a pull's reload runs.
    static let refreshingGap: CGFloat = 96

    /// How far the cards sit right of where they rest.
    private var pullGap: CGFloat {
        refreshing ? max(Self.refreshingGap, pull.distance) : pull.distance
    }

    /// The Tauri app's `--card-width` and `--gap`.
    static let cardWidth: CGFloat = 570
    static let gap: CGFloat = 12

    var body: some View {
        Group {
            if model.cards.isEmpty && !model.loading {
                ContentUnavailableView(
                    model.emptyState.title,
                    systemImage: "tray",
                    description: Text(model.emptyState.lead)
                )
            } else {
                strip
            }
        }
        .navigationTitle(model.title)
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
                        CardView(card: card, model: model)
                            .frame(width: width)
                            .onAppear { model.cardAppeared(card) }
                    }
                    if !model.cards.isEmpty {
                        StripEnd(model: model)
                            .frame(width: width)
                    }
                }
                .scrollTargetLayout()
                .padding(Self.gap)
                .offset(x: pullGap)
                .background {
                    PullToRefreshMonitor(gesture: $pull, enabled: !model.loading && !refreshing) {
                        reloadFromPull()
                    }
                }
            }
            .scrollTargetBehavior(.viewAligned)
            .overlay(alignment: .leading) {
                if pullGap > 0 {
                    PullIndicator(gesture: pull, refreshing: refreshing, width: pullGap)
                }
            }
        }
    }

    /// Keeps the gap open with a spinner in it until the reload ends.
    private func reloadFromPull() {
        withAnimation(.spring(duration: 0.35)) { refreshing = true }
        Task {
            await model.load()
            withAnimation(.spring(duration: 0.35)) { refreshing = false }
        }
    }
}

/// The end of the strip: what まとめて完了 does, and the button.
struct StripEnd: View {
    let model: AppModel

    var body: some View {
        VStack(spacing: 16) {
            Text(model.endSummary)
                .multilineTextAlignment(.center)
                .foregroundStyle(.secondary)
            Button {
                Task { await model.archiveAll() }
            } label: {
                if model.archivingAll {
                    ProgressView().controlSize(.small)
                } else {
                    Text("まとめて完了")
                }
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .disabled(model.archivingAll || model.loading)
        }
        .padding(24)
        .frame(maxHeight: .infinity)
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
