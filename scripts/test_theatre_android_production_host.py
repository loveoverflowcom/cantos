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

    def test_manifest_declares_only_native_playback_and_identity_capabilities(self):
        self.assertEqual({p.get(ANDROID + "name") for p in self.manifest.findall("uses-permission")},
                         {"android.permission.INTERNET", "android.permission.WAKE_LOCK", "android.permission.FOREGROUND_SERVICE",
                          "android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK"})
        self.assertEqual(self.manifest.findall("permission"), [])
        application = self.manifest.find("application")
        self.assertEqual(application.get(ANDROID + "allowBackup"), "false")
        self.assertEqual(application.get(ANDROID + "theme"), "@style/Theme.Cantos")
        self.assertIsNone(application.get(ANDROID + "debuggable"))
        self.assertEqual(application.get(ANDROID + "usesCleartextTraffic"), "false")
        self.assertEqual({m.get(ANDROID + "name"):m.get(ANDROID + "value") for m in application.findall("meta-data")},
                         {"CantosApiOrigin":"${cantosApiOrigin}", "CantosNativeIssuer":"${cantosNativeIssuer}"})
        services = application.findall("service")
        self.assertEqual(len(services), 1)
        self.assertEqual(services[0].get(ANDROID + "name"), "com.cantos.theatre.adapters.playback.CantosPlaybackService")
        self.assertEqual(services[0].get(ANDROID + "exported"), "false")
        self.assertEqual(services[0].get(ANDROID + "foregroundServiceType"), "mediaPlayback")
        for component in ("receiver", "provider", "activity-alias"):
            self.assertEqual(application.findall(component), [], component)

    def test_single_launcher_activity_retains_system_browser_callback(self):
        activities = self.manifest.findall("application/activity")
        self.assertEqual([activity.get(ANDROID + "name") for activity in activities], [".app.TheatreActivity"])
        self.assertEqual(activities[0].get(ANDROID + "launchMode"), "singleTask")
        routes = activities[0].findall("intent-filter")
        self.assertEqual(len(routes), 2)
        self.assertEqual(routes[0].find("action").get(ANDROID + "name"), "android.intent.action.MAIN")
        self.assertEqual(routes[1].find("action").get(ANDROID + "name"), "android.intent.action.VIEW")
        self.assertEqual({node.get(ANDROID + "name") for node in routes[1].findall("category")},
                         {"android.intent.category.DEFAULT", "android.intent.category.BROWSABLE"})
        self.assertEqual(routes[1].find("data").get(ANDROID + "scheme"), "com.cantos.theatre")

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
        self.assertNotIn("CantosTheatreDemo", activity)
        self.assertNotIn("mockAppContainer", activity)
        self.assertIn("setLiveCatalogContent", activity)
        self.assertIn("parseAndroidCallback", activity)
        self.assertIn("onNewIntent", activity)



if __name__ == "__main__":
    unittest.main()
