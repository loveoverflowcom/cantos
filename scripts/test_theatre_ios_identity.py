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

    def test_custom_plist_references_only_the_api_authority(self):
        with (MOBILE / "iosApp/Cantos/Info.plist").open("rb") as source:
            plist = plistlib.load(source)
        self.assertEqual(plist["CantosApiOrigin"], "$(CANTOS_API_ORIGIN)")
        self.assertNotIn("CantosNativeIssuer", plist)

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
        self.assertIn("NativeIdentityV1Configuration.resolve(apiOrigin)", owner)
        self.assertIn("NativeIdentityV1Client(it, platform, now)", owner)
        self.assertIn("SecureNativeCredentialJournal(object : CredentialStore", owner)
        self.assertNotIn("NativeClientConfiguration.resolve", owner)
        self.assertNotIn("NativeRegistrationHttp", owner)
        self.assertIn("fun browserCancelled() = Unit", owner)


if __name__ == "__main__":
    unittest.main()
