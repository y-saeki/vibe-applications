import SwiftUI
import WebKit

/// Signs in to Hatena: fetches a request token, then shows Hatena's approval
/// page in a sheet until it redirects to the app's callback.
struct SignInButton: View {
    var title = "はてなでログイン"
    @Environment(AccountStore.self) private var accounts
    @State private var pending: AccountStore.PendingSignIn?
    @State private var isWorking = false
    @State private var error: Error?

    var body: some View {
        Button {
            Task { await begin() }
        } label: {
            if isWorking {
                ProgressView()
            } else {
                Text(title)
            }
        }
        .disabled(isWorking || !accounts.canSignIn)
        .sheet(item: $pending) { pending in
            NavigationStack {
                SignInWebView(url: pending.approvalURL) { callback in
                    self.pending = nil
                    Task { await finish(pending, callback: callback) }
                }
                .navigationTitle("はてなにログイン")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("キャンセル") { self.pending = nil }
                    }
                }
            }
        }
        .alert("ログインできませんでした", isPresented: Binding(get: { error != nil }, set: { if !$0 { error = nil } })) {
            Button("OK", role: .cancel) {}
        } message: {
            Text(error.map { ErrorBanner.title(for: $0) + "\n" + ErrorBanner.detail(for: $0) } ?? "")
        }
    }

    private func begin() async {
        await run { pending = try await accounts.beginSignIn() }
    }

    private func finish(_ pending: AccountStore.PendingSignIn, callback: URL) async {
        // Turning the request down on Hatena's page is the user's choice, not a failure.
        await run { _ = try await accounts.finishSignIn(pending, callback: callback) }
    }

    private func run(_ step: () async throws -> Void) async {
        isWorking = true
        defer { isWorking = false }
        do {
            try await step()
        } catch {
            FailureLog.record(error)
            self.error = error
        }
    }
}

/// Hatena's approval page. Hatena only redirects to http(s) URLs, which the
/// system's authentication session cannot hand back to the app without a
/// server, so the page is shown here and the redirect is caught before it
/// leaves the device.
private struct SignInWebView: UIViewRepresentable {
    let url: URL
    let onCallback: (URL) -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onCallback: onCallback)
    }

    func makeUIView(context: Context) -> WKWebView {
        let webView = WKWebView()
        webView.navigationDelegate = context.coordinator
        webView.load(URLRequest(url: url))
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKNavigationDelegate {
        let onCallback: (URL) -> Void

        init(onCallback: @escaping (URL) -> Void) {
            self.onCallback = onCallback
        }

        func webView(
            _ webView: WKWebView,
            decidePolicyFor navigationAction: WKNavigationAction,
            decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
        ) {
            if let url = navigationAction.request.url, AccountStore.isCallback(url) {
                decisionHandler(.cancel)
                onCallback(url)
                return
            }
            decisionHandler(.allow)
        }
    }
}

/// Stands in for a screen that needs an account when there is none, or when
/// Hatena refused the saved one.
struct SignInPrompt: View {
    @Environment(AccountStore.self) private var accounts

    var body: some View {
        ContentUnavailableView {
            Label(accounts.needsSignInAgain ? "ログインし直してください" : "ログインが必要です", systemImage: "person.crop.circle.badge.exclamationmark")
        } description: {
            if !accounts.canSignIn {
                Text("このビルドには、はてなの consumer key が入っていません。")
            } else if accounts.needsSignInAgain {
                Text("はてなが、保存してあるログイン情報を受け付けませんでした。")
            } else {
                Text("ブックマークするには、はてなにログインします。")
            }
        } actions: {
            SignInButton(title: accounts.needsSignInAgain ? "ログインし直す" : "はてなでログイン")
                .buttonStyle(.borderedProminent)
        }
    }
}
