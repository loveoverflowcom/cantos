import SwiftUI
import UIKit
import CantosTheatreDemo

@main
struct CantosDemoApp: App {
    var body: some Scene {
        WindowGroup {
            CantosComposeView()
                .ignoresSafeArea()
        }
    }
}

private struct CantosComposeView: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> UIViewController {
        MainViewControllerKt.MainViewController()
    }

    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {
        // The shared composable owns deterministic, in-memory demo state.
    }
}
