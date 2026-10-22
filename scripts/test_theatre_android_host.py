"""Static contract of the one Android application (#286); packaged bytes are checked by theatre-host."""

from pathlib import Path
import json
import re
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
ANDROID = "{http://schemas.android.com/apk/res/android}"
HOST = ROOT / "apps/cantos-theatre/mobile/androidApp"
MANIFEST = HOST / "src/main/AndroidManifest.xml"


def application_label(manifest):
    return manifest.find("application").get(ANDROID + "label")


class TheatreAndroidHostTest(unittest.TestCase):
    def setUp(self):
        self.manifest = ET.parse(MANIFEST).getroot()

    def test_manifest_declares_only_native_playback_and_identity_capabilities(self):
        self.assertEqual({p.get(ANDROID + "name") for p in self.manifest.findall("uses-permission")},
                         {"android.permission.INTERNET", "android.permission.ACCESS_NETWORK_STATE",
                          "android.permission.WAKE_LOCK", "android.permission.FOREGROUND_SERVICE",
                          "android.permission.FOREGROUND_SERVICE_MEDIA_PLAYBACK"})
        self.assertEqual(self.manifest.findall("permission"), [])
        application = self.manifest.find("application")
        self.assertEqual(application.get(ANDROID + "allowBackup"), "false")
        self.assertEqual(application.get(ANDROID + "theme"), "@style/Theme.Cantos")
        self.assertIsNone(application.get(ANDROID + "debuggable"))
        self.assertEqual(application.get(ANDROID + "usesCleartextTraffic"), "false")
        self.assertEqual({m.get(ANDROID + "name"):m.get(ANDROID + "value") for m in application.findall("meta-data")},
                         {"CantosApiOrigin":"${cantosApiOrigin}", "CantosReleaseAudit":"${cantosReleaseAudit}"})
        services = application.findall("service")
        self.assertEqual(len(services), 1)
        self.assertEqual(services[0].get(ANDROID + "name"), "com.cantos.theatre.adapters.playback.CantosPlaybackService")
        self.assertEqual(services[0].get(ANDROID + "exported"), "false")
        self.assertEqual(services[0].get(ANDROID + "foregroundServiceType"), "mediaPlayback")
        for component in ("receiver", "provider", "activity-alias"):
            self.assertEqual(application.findall(component), [], component)

    def test_single_launcher_activity_has_no_browser_callback_or_url_scheme(self):
        activities = self.manifest.findall("application/activity")
        self.assertEqual([activity.get(ANDROID + "name") for activity in activities], [".app.TheatreActivity"])
        self.assertEqual(activities[0].get(ANDROID + "launchMode"), "singleTask")
        routes = activities[0].findall("intent-filter")
        # Exactly the launcher entry: MAIN/LAUNCHER, nothing else a browser or another app can open.
        self.assertEqual(len(routes), 1)
        self.assertEqual([node.get(ANDROID + "name") for node in routes[0].findall("action")], ["android.intent.action.MAIN"])
        self.assertEqual([node.get(ANDROID + "name") for node in routes[0].findall("category")], ["android.intent.category.LAUNCHER"])
        self.assertEqual(routes[0].findall("data"), [])
        # Whole-manifest negative controls, so a filter added to another component is caught too.
        categories = {node.get(ANDROID + "name") for node in self.manifest.iter("category")}
        self.assertNotIn("android.intent.category.BROWSABLE", categories)
        self.assertEqual(list(self.manifest.iter("data")), [])
        self.assertNotIn("${applicationId}", MANIFEST.read_text())
        self.assertEqual(application_label(self.manifest), "@string/app_name")

    def test_window_backgrounds_match_canonical_tokens_without_a_splash_dependency(self):
        tokens = json.loads((ROOT / "packages/ui-system/shared/tokens.json").read_text())
        for theme, qualifier in [("light", "values"), ("dark", "values-night")]:
            color = ET.parse(HOST / "src/main/res" / qualifier / "colors.xml").find("color").text
            self.assertEqual(color.lower(), tokens["themes"][theme]["color.background"]["value"])
            styles = ET.parse(HOST / "src/main/res" / qualifier / "styles.xml").findall("style")
            self.assertEqual(styles[0].get("name"), "Theme.Cantos")
            self.assertEqual({item.get("name"): item.text for item in styles[0]}["android:windowBackground"],
                             "@color/cantos_launch_background")

    def test_build_composes_shared_only_with_an_isolated_unsigned_audit_of_optimized_release(self):
        build = (HOST / "build.gradle.kts").read_text()
        projects = re.findall(r'project\("([^"]+)"\)', build)
        self.assertEqual(projects, [":shared"])
        self.assertEqual(re.findall(r'applicationId = "([^"]+)"', build), ["com.cantos.theatre"])
        self.assertIn("isMinifyEnabled = !releaseAudit", build)
        self.assertIn("isShrinkResources = !releaseAudit", build)
        self.assertIn("if (!releaseAudit", build)
        self.assertIn("CANTOS_RELEASE_KEYSTORE", build)
        self.assertIn("build-audit", build)
        self.assertIn("ScopedArtifacts.Scope.ALL", build)
        self.assertIn("ScopedArtifact.CLASSES", build)
        self.assertNotIn("debugImplementation", build)
        self.assertNotIn("preview-sandbox", build)
        self.assertNotIn("productFlavors", build)
        self.assertNotIn("isMock", build)
        self.assertIn('it.name == "preReleaseBuild"', build)
        self.assertIn('it.name == "preDebugBuild"', build)
        self.assertIn('"${variant}CompileClasspath", "${variant}RuntimeClasspath"', build)
        activity = (HOST / "src/main/kotlin/com/cantos/theatre/app/TheatreActivity.kt").read_text()
        self.assertNotIn("CantosTheatreDemo", activity)
        self.assertNotIn("mockAppContainer", activity)
        self.assertIn("CantosTheatreApp(", activity)

    def test_activity_receives_no_callback_intent(self):
        activity = (HOST / "src/main/kotlin/com/cantos/theatre/app/TheatreActivity.kt").read_text()
        # singleTask stays for the launcher, but no code may take a browser callback from an Intent.
        self.assertNotRegex(activity, r"\bonNewIntent\b")
        self.assertNotRegex(activity, r"\bdeliverCallback\b|\bparseAndroidCallback\b|\bNativeCallback\w*")
        self.assertNotIn("setIntent", activity)
        self.assertNotRegex(activity, r"\bintent\b\s*\??\.\s*data\b")
        self.assertNotIn("NativeCallbackProfile", activity)

    def test_debug_is_the_side_by_side_dev_identity_and_release_keeps_the_production_id(self):
        build = (HOST / "build.gradle.kts").read_text()
        self.assertIn('applicationIdSuffix = ".dev"', build)
        self.assertIn('versionNameSuffix = "-dev"', build)
        self.assertIn('namespace = "com.cantos.theatre"', build)
        self.assertEqual(re.findall(r"^\s*create\(", build, re.M), [])
        label = lambda path: ET.parse(path).find("string").text
        self.assertEqual(label(HOST / "src/main/res/values/app_name.xml"), "Cantos")
        self.assertEqual(label(HOST / "src/debug/res/values/app_name.xml"), "Cantos Dev")

    def test_native_configuration_build_inputs_preserve_absence_separately_from_empty(self):
        gradle = (HOST / "build.gradle.kts").read_text()
        for placeholder, property_name, environment_name in [
            ("cantosApiOrigin", "cantos.apiOrigin", "CANTOS_API_ORIGIN"),
        ]:
            self.assertRegex(
                gradle,
                rf'manifestPlaceholders\["{placeholder}"\]\s*=\s*'
                rf'providers\.gradleProperty\("{property_name}"\)\s*'
                rf'\.orElse\(providers\.environmentVariable\("{environment_name}"\)\)\s*'
                rf'\.orElse\("{"__CANTOS_DEFAULT_API_ORIGIN__" if placeholder == "cantosApiOrigin" else ""}"\)\.get\(\)',
            )

    def test_identity_v1_is_composed_over_the_one_admitted_api_origin(self):
        shared = HOST.parent / "shared/src/androidMain/kotlin/com/cantos/theatre/adapters/identity"
        process = (HOST / "src/main/kotlin/com/cantos/theatre/app/TheatreProcess.kt").read_text()
        native = (shared / "AndroidNativeAccount.kt").read_text()
        transport = (shared / "AndroidNativeTransport.kt").read_text()
        # The host admits ONE ApiOrigin (loopback http only for a debuggable host) and hands that
        # same value to the catalog, the Identity v1 account and registration, and the JSON transport.
        self.assertRegex(process, r"val\s+allowLoopback\s*=\s*debuggable\s*&&")
        self.assertRegex(process, r"selectPackagedApiOrigin\(\s*raw\s*,\s*allowLoopback\s*\)")
        self.assertRegex(process, r"AndroidNativeHttp\(\s*scope\s*,\s*origin\s*\)")
        self.assertRegex(process, r"createAndroidNativeAccount\(\s*scope\s*,\s*app\s*,\s*origin\s*\)")
        self.assertRegex(process, r"createAndroidRegistration\(\s*scope\s*,\s*origin\s*\)")
        # Account and registration are NativeIdentityV1Client(ApiOrigin, transport bound to that origin, clock).
        self.assertEqual(len(re.findall(
            r"NativeIdentityV1Client\(\s*origin\s*,\s*AndroidNativeHttp\(\s*scope\s*,\s*origin\s*\)\s*,\s*now\s*\)", native)), 2)
        self.assertRegex(native, r"SecureNativeCredentialJournal\(\s*AndroidCredentialStore\(context\)\s*,\s*origin\s*\)")
        self.assertRegex(transport, r"class\s+AndroidNativeHttp\(\s*private\s+val\s+scope:\s*CoroutineScope\s*,\s*private\s+val\s+origin:\s*ApiOrigin\s*\)")
        # Cleartext is the origin's decision, never an independent switch of the transport.
        self.assertIn("origin.isHttps", transport)
        for text in [process, native, transport]:
            for retired in ("CantosNativeIssuer", "NativeClientConfiguration", "NativeIdentityV1Configuration",
                            "AndroidSystemBrowser", "NativeCallbackProfile", "customtabs", "ACTION_VIEW"):
                self.assertNotIn(retired, text)
        self.assertNotIn("cantos.nativeIssuer", (HOST / "build.gradle.kts").read_text())
        self.assertNotIn("nativeIssuer", MANIFEST.read_text())

    def test_release_forbids_cleartext_and_only_debug_adds_the_loopback_policy(self):
        policy = lambda path: ET.parse(path).find("bool").text
        self.assertEqual(policy(HOST / "src/main/res/values/origin_policy.xml"), "false")
        self.assertEqual(policy(HOST / "src/debug/res/values/origin_policy.xml"), "true")
        self.assertEqual(self.manifest.find("application").get(ANDROID + "usesCleartextTraffic"), "false")
        self.assertIsNone(self.manifest.find("application").get(ANDROID + "networkSecurityConfig"))
        debug = ET.parse(HOST / "src/debug/AndroidManifest.xml").getroot()
        self.assertEqual(debug.find("application").get(ANDROID + "networkSecurityConfig"), "@xml/cantos_loopback_network_security")
        domains = {node.text for node in ET.parse(HOST / "src/debug/res/xml/cantos_loopback_network_security.xml").iter("domain")}
        self.assertEqual(domains, {"127.0.0.1", "localhost"})

    def test_removed_demo_modules_and_flavors_do_not_return(self):
        mobile = HOST.parent
        self.assertFalse((mobile / "androidProductionApp").exists())
        self.assertTrue((HOST / "proguard-rules.pro").exists())
        self.assertNotIn("androidProductionApp", (mobile / "settings.gradle.kts").read_text())


if __name__ == "__main__":
    unittest.main()
