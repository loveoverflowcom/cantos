"""Static production Android host contract; packaged bytes are checked by theatre-host."""

from pathlib import Path
import json
import re
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
ANDROID = "{http://schemas.android.com/apk/res/android}"
HOST = ROOT / "apps/cantos-theatre/mobile/androidProductionApp"
MANIFEST = HOST / "src/main/AndroidManifest.xml"


class TheatreAndroidProductionHostTest(unittest.TestCase):
    def setUp(self):
        self.manifest = ET.parse(MANIFEST).getroot()

    def test_manifest_admits_only_catalog_network_capability_without_identity_or_tls_override(self):
        self.assertEqual([item.get(ANDROID + "name") for item in self.manifest.findall("uses-permission")],
                         ["android.permission.INTERNET"])
        self.assertEqual(self.manifest.findall("permission"), [])
        application = self.manifest.find("application")
        # No Application subclass: there are no process owners until production adapters exist.
        self.assertIsNone(application.get(ANDROID + "name"))
        self.assertEqual(application.get(ANDROID + "allowBackup"), "false")
        self.assertEqual(application.get(ANDROID + "theme"), "@style/Theme.Cantos")
        self.assertIsNone(application.get(ANDROID + "debuggable"))
        self.assertIsNone(application.get(ANDROID + "networkSecurityConfig"))
        self.assertEqual({item.get(ANDROID + "name"): item.get(ANDROID + "value")
                          for item in application.findall("meta-data")},
                         {"CantosApiOrigin": "${cantosApiOrigin}"})
        self.assertEqual(application.get(ANDROID + "usesCleartextTraffic"), "false")
        for component in ("service", "receiver", "provider", "activity-alias"):
            self.assertEqual(application.findall(component), [], component)

    def test_single_launcher_activity_has_no_identity_callback_route(self):
        activities = self.manifest.findall("application/activity")
        self.assertEqual([activity.get(ANDROID + "name") for activity in activities], [".app.TheatreActivity"])
        routes = [
            (tuple(node.get(ANDROID + "name") for node in route.findall("action")),
             tuple(node.get(ANDROID + "name") for node in route.findall("category")),
             route.findall("data"))
            for route in activities[0].findall("intent-filter")
        ]
        self.assertEqual(routes, [(("android.intent.action.MAIN",), ("android.intent.category.LAUNCHER",), [])])

    def test_window_backgrounds_match_canonical_tokens_without_a_splash_dependency(self):
        tokens = json.loads((ROOT / "packages/ui-system/shared/tokens.json").read_text())
        for theme, qualifier in [("light", "values"), ("dark", "values-night")]:
            color = ET.parse(HOST / "src/main/res" / qualifier / "colors.xml").find("color").text
            self.assertEqual(color.lower(), tokens["themes"][theme]["color.background"]["value"])
            styles = ET.parse(HOST / "src/main/res" / qualifier / "styles.xml").findall("style")
            self.assertEqual([style.get("name") for style in styles], ["Theme.Cantos"])
            self.assertEqual({item.get("name"): item.text for item in styles[0]}["android:windowBackground"],
                             "@color/cantos_launch_background")

    def test_build_composes_shared_only_with_an_unshrunk_unsigned_release(self):
        build = (HOST / "build.gradle.kts").read_text()
        projects = re.findall(r'project\("([^"]+)"\)', build)
        self.assertEqual(projects, [":shared"])
        self.assertEqual(re.findall(r'applicationId = "([^"]+)"', build), ["com.cantos.theatre"])
        self.assertIn("isMinifyEnabled = false", build)
        self.assertIn("isShrinkResources = false", build)
        self.assertNotIn("signingConfig", build)
        self.assertNotIn("debugImplementation", build)
        self.assertIn('tasks.named("preBuild") { dependsOn(assertReleaseDependencyBoundary) }', build)
        activity = (HOST / "src/main/kotlin/com/cantos/theatre/app/TheatreActivity.kt").read_text()
        self.assertIn("PreferencesController(localePreferenceStore = AndroidLocalePreferenceStore(this))", activity)
        self.assertIn("setContent { CantosListeningUnavailable(preferences, locale = locale) }", activity)
        # Comment-aware source/import policy belongs to check_theatre_architecture.
        self.assertEqual(sorted(set(re.findall(r"^import ([\w.]+)\.\w+$", activity, re.M))), [
            "android.content.res", "android.os", "androidx.activity", "androidx.activity.compose",
            "com.cantos.theatre.adapters.preferences", "com.cantos.theatre.application.preferences",
            "com.cantos.theatre.features.launch.presentation", "com.cantos.ui",
        ])


if __name__ == "__main__":
    unittest.main()
