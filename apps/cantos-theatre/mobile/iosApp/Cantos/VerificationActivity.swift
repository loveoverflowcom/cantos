import Foundation

/// Transient UIKit/Handoff intake. Parsing and authority stay in the shared link controller.
@MainActor enum VerificationActivity {
    static func receive(_ activity: NSUserActivity, deliver: (String) -> Void) {
        guard activity.activityType == NSUserActivityTypeBrowsingWeb else { return }
        let url = activity.webpageURL
        // Clear the foreign envelope before invoking any app code. No activity is retained,
        // donated, indexed or used for scene restoration by Cantos.
        activity.webpageURL = nil
        activity.userInfo = nil
        activity.isEligibleForHandoff = false
        activity.isEligibleForSearch = false
        activity.isEligibleForPublicIndexing = false
        guard let url = url else { return }
        deliver(url.absoluteString)
    }
}
