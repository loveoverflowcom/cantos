"""iOS link source contract and executable Foundation intake; no signed-device claim."""

from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
MOBILE = ROOT / "apps/cantos-theatre/mobile"
SWIFT = MOBILE / "iosApp/Cantos"
PROJECT = MOBILE / "iosApp/Cantos.xcodeproj/project.pbxproj"


class TheatreIosVerificationTest(unittest.TestCase):
    def test_only_release_claims_the_exact_associated_domain(self):
        project = PROJECT.read_text()
        targets = re.findall(
            r"/\* (Debug|Release) target \*/ = \{\s*isa = XCBuildConfiguration;"
            r"\s*buildSettings = \{(.*?)\n\s*\};\s*name = (Debug|Release);",
            project, re.S,
        )
        self.assertEqual([label for label, _, _ in targets], ["Debug", "Release"])
        for label, settings, _ in targets:
            values = dict(re.findall(r"^\s*(\w+) = ([^;]*);", settings, re.M))
            if label == "Debug":
                self.assertNotIn("CODE_SIGN_ENTITLEMENTS", values)
            else:
                self.assertEqual(values["CODE_SIGN_ENTITLEMENTS"], '"Cantos/Release.entitlements"')
        with (SWIFT / "Release.entitlements").open("rb") as source:
            self.assertEqual(plistlib.load(source), {
                "com.apple.developer.associated-domains": ["applinks:cantos.cc"],
            })
        self.assertNotIn("DEVELOPMENT_TEAM =", project)

    def test_both_swiftui_deliveries_use_the_process_owner_without_saved_link_state(self):
        app = (SWIFT / "CantosApp.swift").read_text()
        self.assertIn(".onContinueUserActivity(NSUserActivityTypeBrowsingWeb)", app)
        self.assertIn(".onOpenURL", app)
        self.assertIn("VerificationActivity.receive(activity)", app)
        self.assertEqual(app.count("@StateObject private var owner = CatalogOwner()"), 1)
        self.assertIn("native.receiveVerificationLink(url:", app)
        bridge = (SWIFT / "VerificationActivity.swift").read_text()
        self.assertIn("VerificationActivity.swift in Sources", PROJECT.read_text())
        for source in (app, bridge):
            self.assertNotRegex(source, r"@SceneStorage|@AppStorage|NSUserActivity\(activityType:")
            self.assertNotRegex(source, r"\bprint\(|\bNSLog\(|\bos_log\(")
        self.assertNotIn("CFBundleURLTypes", plistlib.loads((SWIFT / "Info.plist").read_bytes()))

    def test_ios_composes_the_existing_anonymous_completion_client(self):
        path = MOBILE / "shared/src/iosMain/kotlin/com/cantos/theatre/app"
        identity = (path / "IosNativeOwner.kt").read_text()
        catalog = (path / "IosCatalogOwner.kt").read_text()
        self.assertEqual(identity.count("NativeIdentityV1Client("), 1)
        self.assertIn("EmailVerificationController(scope, it, ProductionVerifyEmailHandler)", identity)
        self.assertIn("verification?.leave()", identity)
        self.assertIn("verification = identity.verification", catalog)
        self.assertIn("shell?.receiveLink(url)", catalog)

    @unittest.skipUnless(sys.platform == "darwin" and shutil.which("swiftc"),
                         "executable NSUserActivity intake needs the installed Apple Swift compiler")
    def test_real_foundation_activity_is_scrubbed_before_delivery_and_cannot_be_replayed(self):
        harness = r'''
import Foundation

@main @MainActor struct IntakeTest {
    static func main() {
        var delivered: [String] = []
        let foreign = NSUserActivity(activityType: "example.foreign")
        foreign.webpageURL = URL(string: "https://cantos.cc/verify-email")!
        VerificationActivity.receive(foreign) { delivered.append($0) }
        precondition(delivered.isEmpty)

        let missing = NSUserActivity(activityType: NSUserActivityTypeBrowsingWeb)
        VerificationActivity.receive(missing) { delivered.append($0) }
        precondition(delivered.isEmpty)

        let activity = NSUserActivity(activityType: NSUserActivityTypeBrowsingWeb)
        let url = "https://cantos.cc/verify-email?mode=verifyEmail&oobCode=synthetic"
        activity.webpageURL = URL(string: url)!
        activity.userInfo = ["untrusted": "synthetic"]
        activity.isEligibleForHandoff = true
        activity.isEligibleForSearch = true
        activity.isEligibleForPublicIndexing = true
        VerificationActivity.receive(activity) { value in
            precondition(activity.webpageURL == nil && (activity.userInfo?.isEmpty ?? true))
            precondition(!activity.isEligibleForHandoff && !activity.isEligibleForSearch)
            precondition(!activity.isEligibleForPublicIndexing)
            delivered.append(value)
        }
        precondition(delivered == [url])
        // Another scene/callback handed this same object cannot replay its cleared URL.
        VerificationActivity.receive(activity) { delivered.append($0) }
        precondition(delivered == [url])
        print("PASS Foundation activity intake, scrub-before-delivery, duplicate, absent, foreign")
    }
}
'''
        with tempfile.TemporaryDirectory(prefix="cantos-ios-intake-") as directory:
            scratch = Path(directory)
            driver = scratch / "IntakeTest.swift"
            driver.write_text(harness)
            binary = scratch / "intake-test"
            compile_result = subprocess.run(
                ["swiftc", "-module-cache-path", str(scratch / "module-cache"),
                 "-parse-as-library", str(SWIFT / "VerificationActivity.swift"),
                 str(driver), "-o", str(binary)], capture_output=True, text=True, timeout=60,
            )
            self.assertEqual(compile_result.returncode, 0, compile_result.stderr)
            result = subprocess.run([str(binary)], capture_output=True, text=True, timeout=15)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("PASS Foundation activity intake", result.stdout)


if __name__ == "__main__":
    unittest.main()
