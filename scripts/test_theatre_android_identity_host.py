"""Static Android manifest contract; never substitutes for an OS callback/device run."""

from pathlib import Path
import json
import unittest
import xml.etree.ElementTree as ET


ROOT = Path(__file__).resolve().parents[1]
ANDROID = "{http://schemas.android.com/apk/res/android}"
MANIFEST = ROOT / "apps/cantos-theatre/mobile/androidApp/src/main/AndroidManifest.xml"


class TheatreAndroidIdentityHostTest(unittest.TestCase):
    def setUp(self):
        self.manifest = ET.parse(MANIFEST).getroot()

    def test_source_has_only_authorized_internet_permission_and_no_trust_override(self):
        self.assertEqual(
            [node.get(ANDROID + "name") for node in self.manifest.findall("uses-permission")],
            ["android.permission.INTERNET"],
        )
        application = self.manifest.find("application")
        self.assertEqual(application.get(ANDROID + "name"), ".CantosDemoApplication")
        self.assertEqual(application.get(ANDROID + "allowBackup"), "false")
        self.assertIsNone(application.get(ANDROID + "networkSecurityConfig"))
        self.assertNotEqual(application.get(ANDROID + "usesCleartextTraffic"), "true")
        # Operators supply nonsecret origins through build inputs; source ships no live values.
        self.assertEqual(
            {node.get(ANDROID + "name"): node.get(ANDROID + "value")
             for node in application.findall("meta-data")},
            {"CantosApiOrigin": "${cantosApiOrigin}",
             "CantosNativeIssuer": "${cantosNativeIssuer}"},
        )

    def test_native_configuration_build_inputs_reach_metadata_with_empty_defaults(self):
        gradle = (MANIFEST.parents[2] / "build.gradle.kts").read_text()
        for placeholder, property_name, environment_name in [
            ("cantosApiOrigin", "cantos.apiOrigin", "CANTOS_API_ORIGIN"),
            ("cantosNativeIssuer", "cantos.nativeIssuer", "CANTOS_NATIVE_ISSUER"),
        ]:
            self.assertRegex(
                gradle,
                rf'manifestPlaceholders\["{placeholder}"\]\s*=\s*'
                rf'providers\.gradleProperty\("{property_name}"\)\s*'
                rf'\.orElse\(providers\.environmentVariable\("{environment_name}"\)\)\s*'
                r'\.orElse\(""\)\.get\(\)',
            )
        application = (MANIFEST.parent / "kotlin/com/cantos/theatre/demo/CantosDemoApplication.kt").read_text()
        self.assertIn("NativeClientConfiguration.resolve(", application)
        self.assertIn("is NativeIdentityAvailability.Available", application)
        self.assertNotIn("NativeClientConfiguration.create(", application)
        # Android may coerce literal booleans/numbers; preserve supplied input for invalid refusal.
        for key in ["CantosApiOrigin", "CantosNativeIssuer"]:
            self.assertIn(f'metadata?.get("{key}")?.toString()', application)
            self.assertNotIn(f'getString("{key}")', application)

    def test_oauth_scheme_dispatch_reuses_the_process_host_without_extra_routes(self):
        activities = self.manifest.findall("application/activity")
        self.assertEqual(len(activities), 1)
        activity = activities[0]
        self.assertEqual(activity.get(ANDROID + "name"), ".MainActivity")
        self.assertEqual(activity.get(ANDROID + "launchMode"), "singleTask")
        self.assertEqual(activity.get(ANDROID + "exported"), "true")
        filters = activity.findall("intent-filter")
        routes = []
        for route in filters:
            routes.append((
                tuple(node.get(ANDROID + "name") for node in route.findall("action")),
                tuple(node.get(ANDROID + "name") for node in route.findall("category")),
                tuple(dict(node.attrib) for node in route.findall("data")),
            ))
        self.assertEqual(routes, [
            (("android.intent.action.MAIN",), ("android.intent.category.LAUNCHER",), ()),
            (("android.intent.action.VIEW",),
             ("android.intent.category.DEFAULT", "android.intent.category.BROWSABLE"),
             ({ANDROID + "scheme": "com.cantos.theatre"},)),
        ])
        # URI path/query validity and pending state belong to the actual parser/controller.
        self.assertEqual(self.manifest.findall("application/activity-alias"), [])

    def test_launch_theme_and_native_backgrounds_match_canonical_tokens(self):
        tokens = json.loads((ROOT / "packages/ui-system/shared/tokens.json").read_text())
        activity = self.manifest.find("application/activity")
        self.assertEqual(activity.get(ANDROID + "theme"), "@style/Theme.Cantos.Starting")
        for theme, qualifier in [("light", "values"), ("dark", "values-night")]:
            color = ET.parse(MANIFEST.parent / "res" / qualifier / "colors.xml").find("color").text
            self.assertEqual(color.lower(), tokens["themes"][theme]["color.background"]["value"])
        styles = ET.parse(MANIFEST.parent / "res/values/styles.xml").findall("style")
        launch = next(style for style in styles if style.get("name") == "Theme.Cantos.Starting")
        self.assertEqual(launch.get("parent"), "Theme.SplashScreen")
        self.assertEqual({item.get("name"): item.text for item in launch}, {
            "windowSplashScreenBackground": "@color/cantos_launch_background",
            "windowSplashScreenAnimatedIcon": "@drawable/cantos_splash_symbol",
            "postSplashScreenTheme": "@style/Theme.Cantos",
        })
        # Icon background mode would apply a different 240/160 safe-area contract.
        for qualifier in ["drawable", "drawable-night"]:
            vector = ET.parse(MANIFEST.parent / "res" / qualifier / "cantos_splash_symbol.xml").getroot()
            self.assertEqual((vector.get(ANDROID + "viewportWidth"), vector.get(ANDROID + "viewportHeight")), ("288", "288"))
            group = vector.find("group")
            self.assertEqual((group.get(ANDROID + "translateX"), group.get(ANDROID + "translateY")), ("80", "80"))
            self.assertEqual((group.get(ANDROID + "scaleX"), group.get(ANDROID + "scaleY")), ("2", "2"))
            self.assertEqual([p.get(ANDROID + "pathData") for p in group.findall("path")][1:], [
                "M42 15a20 20 0 1 0 0 34", "M27 27v10m7-16v22m7-17v12",
            ])

    def test_ios_launch_is_background_only_with_adaptive_canonical_color(self):
        mobile = ROOT / "apps/cantos-theatre/mobile"
        scene = ET.parse(mobile / "iosApp/CantosDemo/LaunchScreen.storyboard").getroot()
        self.assertEqual(scene.get("launchScreen"), "YES")
        self.assertEqual(len(scene.findall(".//view")), 1)
        for decoration in ["imageView", "label", "activityIndicatorView", "subviews"]:
            self.assertEqual(scene.findall(".//" + decoration), [])
        self.assertEqual(scene.find(".//view/color").get("name"), "LaunchBackground")
        colors = json.loads((mobile / "iosApp/CantosDemo/Assets.xcassets/LaunchBackground.colorset/Contents.json").read_text())["colors"]
        tokens = json.loads((ROOT / "packages/ui-system/shared/tokens.json").read_text())
        for entry, theme in zip(colors, ["light", "dark"]):
            c = entry["color"]["components"]
            actual = "#" + "".join(c[k].removeprefix("0x") for k in ["red", "green", "blue"])
            self.assertEqual(actual.lower(), tokens["themes"][theme]["color.background"]["value"])
        self.assertEqual(colors[1]["appearances"], [{"appearance": "luminosity", "value": "dark"}])
        project = (mobile / "iosApp/CantosDemo.xcodeproj/project.pbxproj").read_text()
        self.assertEqual(project.count("INFOPLIST_KEY_UILaunchStoryboardName = LaunchScreen;"), 2)
        self.assertNotIn("INFOPLIST_KEY_UILaunchScreen_Generation", project)

    def test_install_precedes_super_and_does_not_hold_for_network_or_an_animation(self):
        source = (MANIFEST.parent / "kotlin/com/cantos/theatre/demo/MainActivity.kt").read_text()
        self.assertLess(source.index("installSplashScreen()"), source.index("super.onCreate(savedInstanceState)"))
        self.assertNotIn("setKeepOnScreenCondition", source)
        self.assertNotIn("setOnExitAnimationListener", source)
        self.assertIn("deliverNativeCallback(intent)", source)
        self.assertIn("intent.data = null", source)


if __name__ == "__main__":
    unittest.main()
