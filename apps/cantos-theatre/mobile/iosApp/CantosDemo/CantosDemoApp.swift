import SwiftUI
import UIKit
import CantosTheatreDemo

@main @MainActor
struct CantosDemoApp: App {
    @StateObject private var owner = DemoOwner()

    var body: some Scene {
        WindowGroup {
            CantosComposeView(owner: owner)
                .ignoresSafeArea()
        }
    }
}

@MainActor private final class DemoOwner: ObservableObject {
    private let native: IosNativeOwner
    private let platform: TheatreNativePlatform

    init() {
        let api = Bundle.main.object(forInfoDictionaryKey: "CantosApiOrigin") as? String
        let issuer = Bundle.main.object(forInfoDictionaryKey: "CantosNativeIssuer") as? String
        let adapter = TheatreNativePlatform(apiOrigin: api ?? "")
        let owner = IosNativeOwner(apiOrigin: api, issuer: issuer, platform: adapter)
        adapter.owner = owner
        platform = adapter; native = owner
    }
    func viewController() -> UIViewController { native.viewController() }

    deinit { native.close() }
}

private struct CantosComposeView: UIViewControllerRepresentable {
    let owner: DemoOwner

    func makeUIViewController(context: Context) -> UIViewController {
        let controller = owner.viewController()
        // Match the launch layer while Compose prepares its first frame; this named color
        // follows system appearance without creating another owner or launch view.
        controller.view.backgroundColor = UIColor(named: "LaunchBackground")
        return controller
    }

    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {
        // The application owner survives route/view remounts; all state stays in memory.
    }
}
