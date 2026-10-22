import SwiftUI
import UIKit
import CantosTheatre

@main @MainActor
struct CantosApp: App {
    @StateObject private var owner = CatalogOwner()
    @Environment(\.scenePhase) private var scenePhase
    var body: some Scene {
        WindowGroup { CantosComposeView(owner: owner).ignoresSafeArea()
            .onChange(of: scenePhase) { phase in if phase == .background { owner.backgrounded() } }
        }
    }
}

@MainActor private final class CatalogOwner: ObservableObject {
    private let native: IosCatalogOwner
    private let transport: CatalogPlatform
    private let identityPlatform: TheatreNativePlatform
    private let identity: IosNativeIdentityOwner
    private let media: AvPlayerDriver
    init() {
        // The packaged explicit override, if any. The Kotlin owner applies the production default and
        // refuses an invalid value before any request; Swift never picks an environment itself.
        let packagedOrigin = Bundle.main.object(forInfoDictionaryKey: "CantosApiOrigin") as? String
        let origin = HostOriginKt.packagedApiOriginText(packaged: packagedOrigin)
        let platform = CatalogPlatform(apiOrigin: origin ?? "")
        transport = platform
        let nativePlatform = TheatreNativePlatform(apiOrigin: origin ?? "")
        identityPlatform = nativePlatform
        let identityOwner = IosNativeIdentityOwner(apiOrigin: origin ?? packagedOrigin ?? "",
            platform: nativePlatform)
        identity = identityOwner
        nativePlatform.owner = identityOwner
        let driver = AvPlayerDriver()
        media = driver
        let directory = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        // Account data, credentials and device outbox must not enter device/cloud backups.
        try? FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        var protectedDirectory = directory
        var resourceValues = URLResourceValues()
        resourceValues.isExcludedFromBackup = true
        try? protectedDirectory.setResourceValues(resourceValues)
        native = IosCatalogOwner(apiOrigin: packagedOrigin, platform: platform, identity: identityOwner,
            identityPlatform: nativePlatform, mediaDriver: driver, applicationSupportDirectory: directory.path)
    }
    func viewController() -> UIViewController { native.viewController() }
    func backgrounded() { native.backgrounded() }
    deinit { native.close(); identity.close() }
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
