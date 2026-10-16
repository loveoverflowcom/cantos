"""Static iOS configuration contract; built Info.plist and OS login need separate evidence."""

from pathlib import Path
import plistlib
import re
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
MOBILE = ROOT / "apps/cantos-theatre/mobile"
PROJECT = MOBILE / "iosApp/CantosDemo.xcodeproj/project.pbxproj"


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
            self.assertEqual(values["CANTOS_API_ORIGIN"], '""')
            self.assertEqual(values["CANTOS_NATIVE_ISSUER"], '""')
            self.assertEqual(values["INFOPLIST_FILE"], '"CantosDemo/Info.plist"')
            self.assertNotIn("INFOPLIST_KEY_CantosApiOrigin", values)
            self.assertNotIn("INFOPLIST_KEY_CantosNativeIssuer", values)
            self.assertEqual(values["PRODUCT_BUNDLE_IDENTIFIER"], "com.cantos.theatre.demo")
        # The build consumes Info.plist through INFOPLIST_FILE, never Copy Bundle Resources.
        self.assertNotIn("Info.plist in Resources", project)

    def test_custom_plist_contains_only_the_two_nonsecret_setting_references(self):
        with (MOBILE / "iosApp/CantosDemo/Info.plist").open("rb") as source:
            self.assertEqual(plistlib.load(source), {
                "CantosApiOrigin": "$(CANTOS_API_ORIGIN)",
                "CantosNativeIssuer": "$(CANTOS_NATIVE_ISSUER)",
            })

    def test_shared_scheme_builds_the_same_demo_target_for_debug_and_release(self):
        scheme = ET.parse(PROJECT.parent / "xcshareddata/xcschemes/CantosDemo.xcscheme").getroot()
        entry = scheme.find("BuildAction/BuildActionEntries/BuildActionEntry/BuildableReference")
        self.assertEqual(entry.get("BlueprintIdentifier"), "A10000000000000000000008")
        self.assertEqual(entry.get("BlueprintName"), "CantosDemo")
        self.assertEqual(scheme.find("LaunchAction").get("buildConfiguration"), "Debug")
        self.assertEqual(scheme.find("ArchiveAction").get("buildConfiguration"), "Release")

    def test_swift_forwards_optional_bundle_values_to_the_single_typed_owner(self):
        swift = (MOBILE / "iosApp/CantosDemo/CantosDemoApp.swift").read_text()
        self.assertIn('let api = Bundle.main.object(forInfoDictionaryKey: "CantosApiOrigin") as? String', swift)
        self.assertIn('let issuer = Bundle.main.object(forInfoDictionaryKey: "CantosNativeIssuer") as? String', swift)
        self.assertEqual(swift.count("IosNativeOwner(apiOrigin: api, issuer: issuer, platform: adapter)"), 1)
        self.assertNotIn("IosDemoOwner()", swift)
        owner = (MOBILE / "shared/src/iosMain/kotlin/com/cantos/theatre/app/IosNativeOwner.kt").read_text()
        self.assertIn("val availability = NativeClientConfiguration.resolve(apiOrigin, issuer)", owner)
        self.assertIn("is NativeIdentityAvailability.Available -> resolved.configuration.let", owner)
        self.assertIn("is NativeIdentityAvailability.Missing, is NativeIdentityAvailability.Invalid -> null", owner)


if __name__ == "__main__":
    unittest.main()
