import SwiftUI

@main
struct TenabukuApp: App {
    @State private var readHistory = ReadHistoryStore()
    @State private var accounts = AccountStore()
    @State private var mutes = MuteStore()
    @State private var webSession = WebSessionStore()
    @State private var favicons = FaviconSettingsStore()

    var body: some Scene {
        WindowGroup {
            HotEntryListView()
                .environment(readHistory)
                .environment(accounts)
                .environment(mutes)
                .environment(webSession)
                .environment(favicons)
        }
    }
}
