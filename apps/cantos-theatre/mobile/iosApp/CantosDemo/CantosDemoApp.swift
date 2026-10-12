import SwiftUI
import UIKit
import CantosTheatreDemo

@main
struct CantosDemoApp: App {
    @StateObject private var owner = DemoOwner()

    var body: some Scene {
        WindowGroup {
            CantosComposeView(owner: owner.bridge)
                .ignoresSafeArea()
        }
    }
}

private final class DemoOwner: ObservableObject {
    let bridge = IosDemoOwner()

    deinit { bridge.close() }
}

private struct CantosComposeView: UIViewControllerRepresentable {
    let owner: IosDemoOwner

    func makeUIViewController(context: Context) -> UIViewController {
        owner.viewController()
    }

    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {
        // The application owner survives route/view remounts; all state stays in memory.
    }
}
