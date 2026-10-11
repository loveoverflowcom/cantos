import SwiftUI
import TheatreShared

@main
struct CantosTheatreApp: App {
    var body: some Scene {
        WindowGroup {
            TheatreHost().ignoresSafeArea()
        }
    }
}

private struct TheatreHost: UIViewControllerRepresentable {
    func makeUIViewController(context: Context) -> UIViewController {
        MainViewControllerKt.MainViewController()
    }

    func updateUIViewController(_ uiViewController: UIViewController, context: Context) {}
}
