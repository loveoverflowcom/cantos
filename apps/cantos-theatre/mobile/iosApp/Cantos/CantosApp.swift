import SwiftUI
import UIKit
import CantosTheatre

@main @MainActor
struct CantosApp: App {
    @StateObject private var owner = CatalogOwner()
    var body: some Scene {
        WindowGroup { CantosComposeView(owner: owner).ignoresSafeArea() }
    }
}

@MainActor private final class CatalogOwner: ObservableObject {
    private let native: IosCatalogOwner
    private let transport: CatalogPlatform
    init() {
        let origin = Bundle.main.object(forInfoDictionaryKey: "CantosApiOrigin") as? String
        let platform = CatalogPlatform(apiOrigin: origin ?? "")
        transport = platform
        native = IosCatalogOwner(apiOrigin: origin, platform: platform)
    }
    func viewController() -> UIViewController { native.viewController() }
    deinit { native.close() }
}

private struct CantosComposeView: UIViewControllerRepresentable {
    let owner: CatalogOwner
    func makeUIViewController(context: Context) -> UIViewController {
        let controller = owner.viewController()
        controller.view.backgroundColor = UIColor(named: "LaunchBackground")
        return controller
    }
    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {}
}
