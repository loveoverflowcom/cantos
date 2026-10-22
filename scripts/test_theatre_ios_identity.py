"""Static iOS configuration contract; built Info.plist and OS login need separate evidence."""

from pathlib import Path
import plistlib
import re
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
MOBILE = ROOT / "apps/cantos-theatre/mobile"
PROJECT = MOBILE / "iosApp/Cantos.xcodeproj/project.pbxproj"


class TheatreIosIdentityConfigurationTest(unittest.TestCase):
    def test_debug_and_release_expand_nonsecret_build_settings_in_the_custom_plist(self):
        project = PROJECT.read_text()
        targets = re.findall(
            r"/\* (Debug|Release) target \*/ = \{\s*isa = XCBuildConfiguration;"
            r"\s*buildSettings = \{(.*?)\n\s*\};\s*name = (Debug|Release);",
            project,
            re.S,
        )
        self.assertEqual([(label, name) for label, _, name in targets], [
            ("Debug", "Debug"), ("Release", "Release"),
        ])
        for _, settings, _ in targets:
            values = dict(re.findall(r"^\s*(\w+) = ([^;]*);", settings, re.M))
            self.assertEqual(values["GENERATE_INFOPLIST_FILE"], "YES")
            self.assertEqual(values["CANTOS_API_ORIGIN"], '"__CANTOS_DEFAULT_API_ORIGIN__"')
            self.assertNotIn("CANTOS_NATIVE_ISSUER", values)
            self.assertEqual(values["INFOPLIST_FILE"], '"Cantos/Info.plist"')
            self.assertNotIn("INFOPLIST_KEY_CantosApiOrigin", values)
            self.assertNotIn("INFOPLIST_KEY_CantosNativeIssuer", values)
            self.assertEqual(values["PRODUCT_BUNDLE_IDENTIFIER"], "com.cantos.theatre")
        # The build consumes Info.plist through INFOPLIST_FILE, never Copy Bundle Resources.
        self.assertNotIn("Info.plist in Resources", project)

    def test_custom_plist_references_only_the_api_authority_and_declares_no_url_scheme(self):
        path = MOBILE / "iosApp/Cantos/Info.plist"
        with path.open("rb") as source:
            plist = plistlib.load(source)
        self.assertEqual(plist["CantosApiOrigin"], "$(CANTOS_API_ORIGIN)")
        self.assertEqual(plist["UIBackgroundModes"], ["audio"])
        self.assertNotIn("CantosNativeIssuer", plist)
        # Identity v1 is first-party JSON on the API origin: no URL scheme may route into the app.
        self.assertNotIn("CFBundleURLTypes", plist)
        self.assertNotIn("CFBundleURLSchemes", path.read_text())

    def test_shared_scheme_builds_the_production_target_for_debug_and_release(self):
        scheme = ET.parse(PROJECT.parent / "xcshareddata/xcschemes/Cantos.xcscheme").getroot()
        entry = scheme.find("BuildAction/BuildActionEntries/BuildActionEntry/BuildableReference")
        self.assertEqual(entry.get("BlueprintIdentifier"), "A10000000000000000000008")
        self.assertEqual(entry.get("BlueprintName"), "Cantos")
        self.assertEqual(scheme.find("LaunchAction").get("buildConfiguration"), "Debug")
        self.assertEqual(scheme.find("ArchiveAction").get("buildConfiguration"), "Release")

    def test_swift_forwards_optional_bundle_values_to_the_single_typed_owner(self):
        swift = (MOBILE / "iosApp/Cantos/CantosApp.swift").read_text()
        self.assertIn('Bundle.main.object(forInfoDictionaryKey: "CantosApiOrigin") as? String', swift)
        self.assertNotIn("CantosNativeIssuer", swift)
        self.assertEqual(swift.count('IosNativeIdentityOwner(apiOrigin: origin ?? packagedOrigin ?? "",'), 1)
        owner = (MOBILE / "shared/src/iosMain/kotlin/com/cantos/theatre/app/IosNativeOwner.kt").read_text()
        # One origin owner, HTTPS only on iOS; the Identity v1 client is built over that admitted origin.
        self.assertIn("selectPackagedApiOrigin(apiOrigin, allowLoopbackHttp = false)", owner)
        self.assertRegex(owner, r"NativeIdentityV1Client\(\s*\w+\s*,\s*platform\s*,\s*now\s*\)")
        self.assertIn("SecureNativeCredentialJournal(object : CredentialStore", owner)
        self.assertNotRegex(owner, r"\bfun\s+(callback|browserCancelled)\b")
        for retired in ("NativeIdentityV1Configuration", "NativeClientConfiguration", "NativeRegistrationHttp",
                        "availability", "CantosNativeIssuer"):
            self.assertNotIn(retired, owner)

    def test_no_ios_origin_selection_admits_loopback_http(self):
        sources = sorted((MOBILE / "shared/src/iosMain").rglob("*.kt"))
        calls = [arguments for source in sources
                 for arguments in re.findall(r"\bselect(?:PackagedApi|Catalog)Origin\(([^()]*)\)", source.read_text())]
        # The catalog owner and the identity owner both select; neither may opt in to cleartext.
        self.assertGreaterEqual(len(calls), 2)
        for arguments in calls:
            self.assertRegex(arguments, r"allowLoopbackHttp\s*=\s*false\b", arguments)

    def test_swift_has_no_browser_session_pkce_or_url_scheme_handling(self):
        sources = sorted((MOBILE / "iosApp/Cantos").glob("*.swift"))
        self.assertIn("TheatreIdentity.swift", [source.name for source in sources])
        for source in sources:
            text = source.read_text()
            with self.subTest(source=source.name):
                self.assertNotRegex(text, r"import\s+(AuthenticationServices|CryptoKit|SafariServices)\b")
                for retired in ("ASWebAuthenticationSession", "SFSafariViewController", "onOpenURL",
                                "callbackURLScheme", "code_challenge", "CantosNativeIssuer"):
                    self.assertNotIn(retired, text)


if __name__ == "__main__":
    unittest.main()
