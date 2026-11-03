import SwiftUI

@main
struct TenabukuApp: App {
    @State private var readHistory = ReadHistoryStore()
    @State private var accounts = AccountStore()

    var body: some Scene {
        WindowGroup {
            HotEntryListView()
                .environment(readHistory)
                .environment(accounts)
        }
    }
}
