import SwiftUI
import WebKit

/// Signs in to Hatena's website: shows its login page in a sheet until Hatena
/// sends the browser on to Hatena Bookmark. The cookies the page sets stay in
/// WebKit's default store, where `WebSessionStore` reads them.
struct WebSignInButton: View {
    var title = "Web でログイン"
    @State private var isPresenting = false

    var body: some View {
        Button(title) { isPresenting = true }
            .webSignInSheet(isPresented: $isPresenting)
    }
}

extension View {
    /// Presents the website's login page. `onSignedIn` runs once the login is
    /// done and the sheet has been told to close.
    func webSignInSheet(isPresented: Binding<Bool>, onSignedIn: @escaping () -> Void = {}) -> some View {
        modifier(WebSignInSheet(isPresented: isPresented, onSignedIn: onSignedIn))
    }
}

private struct WebSignInSheet: ViewModifier {
    @Binding var isPresented: Bool
    let onSignedIn: () -> Void
    @Environment(WebSessionStore.self) private var webSession

    func body(content: Content) -> some View {
        content.sheet(isPresented: $isPresented) {
            NavigationStack {
                WebSignInWebView(url: WebSessionStore.signInURL) {
                    isPresented = false
                    webSession.didSignIn()
                    onSignedIn()
                }
                .navigationTitle("はてなにログイン")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .cancellationAction) {
                        Button("キャンセル") { isPresented = false }
                    }
                }
            }
        }
    }
}

private struct WebSignInWebView: UIViewRepresentable {
    let url: URL
    let onSignedIn: () -> Void

    func makeCoordinator() -> Coordinator {
        Coordinator(onSignedIn: onSignedIn)
    }

    func makeUIView(context: Context) -> WKWebView {
        // The default store, so that the cookies outlive the sheet.
        let webView = WKWebView()
        webView.navigationDelegate = context.coordinator
        webView.load(URLRequest(url: url))
        return webView
    }

    func updateUIView(_ webView: WKWebView, context: Context) {}

    final class Coordinator: NSObject, WKNavigationDelegate {
        let onSignedIn: () -> Void
        private var isDone = false

        init(onSignedIn: @escaping () -> Void) {
            self.onSignedIn = onSignedIn
        }

        // On finishing rather than on deciding the navigation, so that the
        // cookies Hatena Bookmark sets on its first page are kept too.
        func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
            guard !isDone, webView.url?.host() == WebSessionStore.landingHost else { return }
            isDone = true
            onSignedIn()
        }
    }
}

/// Stands in for a list that needs the website login when there is none, or
/// when it has expired.
struct WebSignInPrompt: View {
    @Environment(WebSessionStore.self) private var webSession

    var body: some View {
        ContentUnavailableView {
            Label("Web でのログインが必要です", systemImage: "person.crop.circle.badge.exclamationmark")
        } description: {
            Text("マイホットエントリーは、はてなの Web にログインして読み込みます。ブックマークの保存に使うログインとは別のものです。")
        } actions: {
            WebSignInButton(title: "はてなの Web にログイン")
                .buttonStyle(.borderedProminent)
        }
    }
}
