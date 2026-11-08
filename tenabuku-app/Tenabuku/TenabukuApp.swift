import SwiftUI

@main
struct TenabukuApp: App {
    @State private var readHistory = ReadHistoryStore()
    @State private var accounts = AccountStore()
    @State private var mutes = MuteStore()
    @State private var webSession = WebSessionStore()
    @State private var favicons = FaviconSettingsStore()
    @State private var shelves = ShelfSettingsStore()

    var body: some Scene {
        WindowGroup {
            HomeView()
                .environment(readHistory)
                .environment(accounts)
                .environment(mutes)
                .environment(webSession)
                .environment(favicons)
                .environment(shelves)
        }
    }
}
