"""Static Android manifest contract; never substitutes for an OS callback/device run."""

from pathlib import Path
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
        # Operators supply validated nonsecret origins; no live values are shipped here.
        self.assertEqual(application.findall("meta-data"), [])

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


if __name__ == "__main__":
    unittest.main()
