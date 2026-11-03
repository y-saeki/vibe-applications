import SwiftUI

@main
struct TenabukuApp: App {
    @State private var readHistory = ReadHistoryStore()

    var body: some Scene {
        WindowGroup {
            HotEntryListView()
                .environment(readHistory)
        }
    }
}
