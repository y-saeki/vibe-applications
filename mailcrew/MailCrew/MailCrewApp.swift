import SwiftUI

@main
struct MailCrewApp: App {
    @State private var model = AppModel.live()

    var body: some Scene {
        // One window: the inbox is one strip, and every card names its account.
        Window("Mail Crew", id: "main") {
            ContentView(model: model)
                .frame(minWidth: 480, minHeight: 400)
                .task { await model.start() }
        }
        .defaultSize(width: 1240, height: 820)
        .commands {
            CommandGroup(after: .toolbar) {
                Button("再読み込み") { Task { await model.load() } }
                    .keyboardShortcut("r")
                    .disabled(model.screen != .inbox)
            }
        }
    }
}

struct ContentView: View {
    let model: AppModel

    var body: some View {
        switch model.screen {
        case .starting:
            ProgressView()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        case .signIn:
            SignInView(model: model)
        case .inbox:
            InboxView(model: model)
        }
    }
}
