import SwiftUI
import UIKit
import CantosTheatre

@main @MainActor
struct CantosProductionApp: App {
    @StateObject private var owner = CatalogOwner()

    var body: some Scene {
        WindowGroup {
            CatalogComposeView(owner: owner).ignoresSafeArea()
        }
    }
}

@MainActor private final class CatalogOwner: ObservableObject {
    private let native: IosCatalogOwner
    private let platform: TheatreCatalogPlatform

    init() {
        let origin = Bundle.main.object(forInfoDictionaryKey: "CantosApiOrigin") as? String
        let transport = TheatreCatalogPlatform(apiOrigin: origin ?? "")
        platform = transport
        native = IosCatalogOwner(apiOrigin: origin, platform: transport)
    }

    func viewController(systemIsDark: Bool) -> UIViewController {
        native.updateAppearance(systemIsDark: systemIsDark)
        return native.viewController()
    }

    func updateAppearance(systemIsDark: Bool) { native.updateAppearance(systemIsDark: systemIsDark) }

    deinit { native.close() }
}

private struct CatalogComposeView: UIViewControllerRepresentable {
    let owner: CatalogOwner
    @Environment(\.colorScheme) private var colorScheme

    func makeUIViewController(context: Context) -> UIViewController {
        let controller = owner.viewController(systemIsDark: colorScheme == .dark)
        controller.view.backgroundColor = UIColor(named: "LaunchBackground")
        return controller
    }

    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {
        owner.updateAppearance(systemIsDark: colorScheme == .dark)
    }
}
