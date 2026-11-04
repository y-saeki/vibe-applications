import SwiftUI

/// The first screen while nobody is signed in: the OAuth client to sign in
/// with, then Google's sign-in in the browser.
struct SignInView: View {
    let model: AppModel
    @State private var clientId = ""
    @State private var clientSecret = ""

    private static let help = URL(string: "https://github.com/y-saeki/vibe-applications/blob/main/mailcrew/README.md")!

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            Text("Mail Crew")
                .font(.largeTitle.bold())
            Text("Gmail の受信トレイを、1 通ずつのカードで読むメーラーです。")
                .foregroundStyle(.secondary)

            Form {
                TextField("OAuth クライアント ID", text: $clientId)
                SecureField("クライアント シークレット", text: $clientSecret)
            }
            .formStyle(.columns)
            .autocorrectionDisabled()
            .disabled(model.signingIn)

            Text("どちらも Google Cloud で作ったものを使います。作り方は [README](\(Self.help.absoluteString)) にあります。")
                .font(.callout)
                .foregroundStyle(.secondary)

            if let error = model.signInError {
                Text(error)
                    .foregroundStyle(.red)
                    .textSelection(.enabled)
            }

            if model.signingIn {
                HStack(spacing: 12) {
                    ProgressView().controlSize(.small)
                    Text("ブラウザで Google にログインしてください。完了すると、ここに受信トレイが開きます。")
                    Spacer()
                    Button("中止") { model.cancelSignIn() }
                }
            } else {
                Button("Google でログイン") {
                    Task { await model.signIn(clientId: clientId, clientSecret: clientSecret) }
                }
                .buttonStyle(.borderedProminent)
                .keyboardShortcut(.defaultAction)
                .disabled(clientId.isEmpty || clientSecret.isEmpty)
            }
        }
        .padding(32)
        .frame(maxWidth: 520)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}
