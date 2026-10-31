import HatenaShared
import SwiftUI

/// Shows why a fetch failed. A breakage — Hatena's response no longer has the
/// shape the app expects — is shown in red with the endpoint's name, so that
/// it is told apart from a passing network problem.
struct ErrorBanner: View {
    let error: Error

    var body: some View {
        Label {
            VStack(alignment: .leading, spacing: 4) {
                Text(title).font(.subheadline.bold())
                Text(detail).font(.caption).foregroundStyle(.secondary)
            }
        } icon: {
            Image(systemName: isBreakage ? "exclamationmark.octagon.fill" : "wifi.exclamationmark")
                .foregroundStyle(isBreakage ? Color.red : Color.orange)
        }
        .listRowBackground(isBreakage ? Color.red.opacity(0.12) : nil)
    }

    private var isBreakage: Bool {
        (error as? HatenaError)?.isBreakage == true
    }

    private var title: String {
        switch error as? HatenaError {
        case .parseFailed?: "はてな側の仕様が変わった可能性があります"
        case .unauthorized?: "ログインし直してください"
        case .httpStatus?: "はてなからエラーが返りました"
        default: "読み込めませんでした"
        }
    }

    private var detail: String {
        switch error as? HatenaError {
        case let .parseFailed(source, _)?: "取得元: \(source)"
        case let .httpStatus(status)?: "HTTP \(status)"
        case let .transport(message)?: message
        default: error.localizedDescription
        }
    }
}
