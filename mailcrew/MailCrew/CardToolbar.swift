import SwiftUI

/// The row of buttons at the foot of a card: タグ, あとで読む and スター at
/// the left, 完了 at the right. Each one acts on the card's own account.
struct CardToolbar: View {
    let card: Card
    let model: AppModel
    @State private var picking = false

    var body: some View {
        HStack(spacing: 2) {
            ToolButton(title: "タグ", systemImage: "tag") {
                model.tagPickerOpened(for: card)
                picking = true
            }
            .popover(isPresented: $picking, arrowEdge: .top) {
                TagPicker(labels: model.labels(of: card)) { choice in
                    picking = false
                    Task { await model.applyTag(choice, to: card) }
                }
            }
            if model.offersSetAside {
                ToolButton(title: "あとで読む", systemImage: "clock") {
                    Task { await model.setAside(card) }
                }
            }
            ToolButton(
                title: "スター",
                systemImage: card.starred ? "star.fill" : "star",
                tint: card.starred ? .yellow : nil
            ) {
                Task { await model.toggleStar(card) }
            }
            .disabled(model.starring.contains(card.key))
            .accessibilityAddTraits(card.starred ? .isSelected : [])
            Spacer(minLength: 8)
            Button {
                Task { await model.archive(card) }
            } label: {
                Label("完了", systemImage: "checkmark")
                    .fontWeight(.semibold)
                    .padding(.horizontal, 4)
            }
            .buttonStyle(.borderedProminent)
            .buttonBorderShape(.capsule)
            .help("完了")
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
    }
}

/// An icon button, named for VoiceOver and in its tooltip.
private struct ToolButton: View {
    let title: String
    let systemImage: String
    var tint: Color?
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 15))
                .foregroundStyle(tint ?? .secondary)
                .frame(width: 34, height: 30)
                .contentShape(Rectangle())
        }
        .buttonStyle(.borderless)
        .help(title)
        .accessibilityLabel(title)
    }
}

/// The list that opens from a card's タグ: the labels of the card's account,
/// narrowed by what is typed, with a row that creates a new one when nothing
/// is called that. ↑ and ↓ move, Return picks, Escape closes.
struct TagPicker: View {
    let labels: [TagLabel]
    let pick: (TagChoice) -> Void
    @State private var filter = ""
    @State private var active = 0
    @FocusState private var focused: Bool

    private var rows: [TagRow] { AppModel.tagRows(labels: labels, filter: filter) }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            TextField("タグを検索、または作成", text: $filter)
                .textFieldStyle(.roundedBorder)
                .focused($focused)
                .onChange(of: filter) { active = 0 }
                .onSubmit { pickActive() }
                .onKeyPress(.downArrow) { move(1) }
                .onKeyPress(.upArrow) { move(-1) }
            ScrollViewReader { proxy in
                ScrollView(.vertical) {
                    VStack(alignment: .leading, spacing: 0) {
                        if rows.isEmpty {
                            Text("タグがありません")
                                .foregroundStyle(.secondary)
                                .padding(.horizontal, 8)
                                .padding(.vertical, 5)
                        }
                        ForEach(Array(rows.enumerated()), id: \.offset) { index, row in
                            TagRowView(row: row, active: index == active)
                                .id(index)
                                .onTapGesture { pick(row.choice) }
                                .onHover { if $0 { active = index } }
                        }
                    }
                }
                .frame(maxHeight: 240)
                .fixedSize(horizontal: false, vertical: true)
                .onChange(of: active) { proxy.scrollTo(active) }
            }
        }
        .padding(8)
        .frame(width: 260)
        .onAppear { focused = true }
    }

    private func move(_ step: Int) -> KeyPress.Result {
        let count = rows.count
        guard count > 0 else { return .handled }
        active = (active + step + count) % count
        return .handled
    }

    private func pickActive() {
        let rows = rows
        guard rows.indices.contains(active) else { return }
        pick(rows[active].choice)
    }
}

private struct TagRowView: View {
    let row: TagRow
    let active: Bool

    private var creates: Bool {
        if case .new = row.choice { return true }
        return false
    }

    var body: some View {
        Text(row.text)
            .lineLimit(1)
            .truncationMode(.tail)
            .foregroundStyle(active ? Color.white : creates ? Color.accentColor : Color.primary)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(active ? Color.accentColor : .clear, in: RoundedRectangle(cornerRadius: 5))
            .contentShape(Rectangle())
            .accessibilityAddTraits(active ? [.isSelected, .isButton] : .isButton)
    }
}
