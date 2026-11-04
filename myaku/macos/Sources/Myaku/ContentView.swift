import SwiftUI

/// The list inside the menu: the resource's name and the arrows that switch
/// it, then one line per process with its last minute as a graph.
struct ContentView: View {
    @ObservedObject var model: Model

    static let width: CGFloat = 460
    static let rowHeight: CGFloat = 18
    static let rowSpacing: CGFloat = 4
    static let headerHeight: CGFloat = 22
    static let padding = EdgeInsets(top: 6, leading: 14, bottom: 6, trailing: 12)
    static var height: CGFloat {
        let rows = CGFloat(Model.rowCount)
        return padding.top + headerHeight + 6 + rows * rowHeight + (rows - 1) * rowSpacing + padding.bottom
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 2) {
                Text(model.title).bold()
                Spacer()
                arrow("chevron.left", step: -1, label: "前へ")
                arrow("chevron.right", step: 1, label: "次へ")
            }
            .frame(height: Self.headerHeight)

            VStack(spacing: Self.rowSpacing) {
                ForEach(model.rows, id: \.pid) { row in
                    RowView(row: row)
                        .frame(height: Self.rowHeight)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(Self.padding)
        .frame(width: Self.width, height: Self.height)
    }

    private func arrow(_ symbol: String, step: Int32, label: String) -> some View {
        Button {
            model.cycle(step)
        } label: {
            Image(systemName: symbol)
                .font(.system(size: 11, weight: .semibold))
                .frame(width: 22, height: 20)
                .contentShape(Rectangle())
        }
        .buttonStyle(.borderless)
        .accessibilityLabel(label)
    }
}

private struct RowView: View {
    let row: Row

    var body: some View {
        HStack(spacing: 10) {
            Group {
                if row.alive {
                    Text(row.name)
                } else {
                    Text(row.name).italic().foregroundStyle(.secondary)
                }
            }
            .lineLimit(1)
            .truncationMode(.tail)
            .frame(width: 150, alignment: .leading)

            Sparkline(values: row.chart)
                .frame(maxWidth: .infinity)

            Text(row.value)
                .font(.caption.monospacedDigit())
                .foregroundStyle(.secondary)
                .frame(width: 52, alignment: .trailing)
        }
    }
}
