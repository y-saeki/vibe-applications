import Foundation

/// What the list shows, kept up to date while the menu is open.
@MainActor
final class Model: ObservableObject {
    /// How many processes the list shows.
    static let rowCount = 20
    private static let networkKey = "networkStatsEnabled"

    @Published private(set) var rows: [Row] = []
    @Published private(set) var resource: Resource = .cpu
    @Published private(set) var network: Bool

    /// Whether the menu is open. The list is only fetched while it is.
    var visible = false {
        didSet { if visible { refresh() } }
    }

    private var vitals: Vitals?

    init() {
        network = UserDefaults.standard.bool(forKey: Self.networkKey)
        vitals = Vitals(network: network, listener: Updates(model: self))
    }

    func refresh() {
        guard visible, let vitals else { return }
        rows = vitals.top(resource: resource, max: UInt32(Self.rowCount))
    }

    var title: String { resourceTitle(resource: resource) }

    /// The arrows: -1 for the left one, 1 for the right.
    func cycle(_ step: Int32) {
        resource = cycleResource(resource: resource, step: step, network: network)
        refresh()
    }

    func setNetwork(_ on: Bool) {
        network = on
        UserDefaults.standard.set(on, forKey: Self.networkKey)
        vitals?.setNetwork(on: on)
        resource = availableResource(resource: resource, network: on)
        refresh()
    }
}

/// Hears each reading on the library's thread and refreshes the list on the
/// main one. The main queue is also served while a menu is open.
private final class Updates: Listener, @unchecked Sendable {
    private weak var model: Model?

    init(model: Model) {
        self.model = model
    }

    func updated() {
        DispatchQueue.main.async { [weak model] in
            MainActor.assumeIsolated { model?.refresh() }
        }
    }
}
