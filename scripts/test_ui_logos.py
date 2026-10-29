import json
import math
import unittest
from unittest.mock import patch
import xml.etree.ElementTree as ET

import ui_logos
from ui_logos import ASSETS, CMP_OUTPUT, checked_assets, render_kotlin, shapes, validate_anatomy


class UiLogos(unittest.TestCase):
    def test_official_checksums_and_generated_vector_geometry_match(self):
        self.assertEqual(CMP_OUTPUT.read_text(), render_kotlin())
        manifest = checked_assets()
        self.assertEqual(len(manifest["variants"]), 9)
        self.assertEqual(len(manifest["files"]), 11)
        self.assertIn("does not specify", manifest["rights"]["mark"])

    def test_mask_punches_out_only_the_two_original_round_strokes(self):
        svg = ET.fromstring((ASSETS / "svg/cantos-symbol-mono.svg").read_bytes())
        geometry = shapes(svg)
        self.assertEqual(len(geometry), 3)
        self.assertEqual((geometry[0]["width"], geometry[0]["height"], geometry[0]["radius"]), (64, 64, 22))
        self.assertEqual(geometry[1]["pathData"], "M42 15a20 20 0 1 0 0 34")
        self.assertEqual(geometry[2]["pathData"], "M27 27v10m7-16v22m7-17v12")
        self.assertEqual([shape["strokeWidth"] for shape in geometry[1:]], [5, 3])
        self.assertTrue(all(shape["knockout"] and shape["roundCap"] for shape in geometry[1:]))

    def test_lettering_stays_paths_and_no_remote_or_executable_feature_is_admitted(self):
        for markup in ['<script/>', '<image href="remote"/>', '<text>Cantos</text>',
                       '<path d="M0 0" onload="alert(1)"/>', '<foreignObject/>',
                       '<g style="fill: red"/>']:
            with self.subTest(markup=markup), self.assertRaises(ValueError):
                validate_anatomy(ET.fromstring(markup))
        for variant in json.loads((ASSETS / "provenance.json").read_text())["variants"]:
            svg = ET.fromstring((ASSETS / "svg" / variant["file"]).read_bytes())
            geometry = shapes(svg)
            if variant["kind"] != "symbol":
                self.assertGreaterEqual(sum(shape.get("fill") is not None and "pathData" in shape
                                            for shape in geometry), 6)

    def test_unrecognized_transform_or_mask_cannot_be_silently_omitted(self):
        for markup in ['<svg><g transform="scale(2)"><path d="M0 0"/></g></svg>',
                       '<svg><rect width="64" height="64" fill="#292331" mask="url(#missing)"/></svg>']:
            with self.subTest(markup=markup), self.assertRaises(ValueError):
                shapes(ET.fromstring(markup))


class NativeLauncherLogos(unittest.TestCase):
    android = "{" + ui_logos.ANDROID_NAMESPACE + "}"

    def test_manifest_resolves_legacy_and_adaptive_icons_without_touching_splash_or_callback(self):
        application = ET.parse(ui_logos.MOBILE / "androidApp/src/main/AndroidManifest.xml").getroot().find("application")
        for attribute in ("icon", "roundIcon"):
            self.assertEqual(application.attrib[self.android + attribute], "@mipmap/ic_launcher")
        self.assertEqual(application.attrib[self.android + "label"], "@string/app_name")
        activity = application.find("activity")
        self.assertEqual(activity.attrib[self.android + "launchMode"], "singleTask")
        self.assertEqual(activity.attrib[self.android + "theme"], "@style/Theme.Cantos.Starting")
        # The launcher entry stays; the retired provider sign-in callback must not come back. The
        # only other filter is ADR 0048's exact signup App Link (owned by test_theatre_android_host).
        filters = [{element.attrib[self.android + "name"] for element in intent if element.tag in {"action", "category"}}
                   for intent in activity.findall("intent-filter")]
        self.assertEqual(filters, [{"android.intent.action.MAIN", "android.intent.category.LAUNCHER"},
                                   {"android.intent.action.VIEW", "android.intent.category.DEFAULT",
                                    "android.intent.category.BROWSABLE"}])
        self.assertEqual([dict(data.attrib) for data in activity.iter("data")],
                         [{self.android + "scheme": "https", self.android + "host": "cantos.cc",
                           self.android + "path": "/verify-email"}])
        # Night is selected before density/API. A night base vector must never override the
        # adaptive icon on API26+, so both appearances need their own anydpi-v26 descriptor.
        for qualifier in ("mipmap-anydpi-v26", "mipmap-night-anydpi-v26"):
            adaptive = ET.parse(ui_logos.ANDROID_RES / qualifier / "ic_launcher.xml").getroot()
            self.assertEqual(adaptive.tag, "adaptive-icon")
            self.assertEqual({child.tag: child.attrib[self.android + "drawable"] for child in adaptive}, {
                "background": "@color/ic_launcher_background", "foreground": "@drawable/ic_launcher_foreground",
                "monochrome": "@drawable/ic_launcher_monochrome"})
        for qualifier in ("mipmap-anydpi", "mipmap-night-anydpi"):
            vector = ET.parse(ui_logos.ANDROID_RES / qualifier / "ic_launcher.xml").getroot()
            self.assertEqual(vector.tag, "vector")
            self.assertEqual(vector.find("path").attrib[self.android + "pathData"], "M0 0h108v108H0z")
            self.assertEqual(len(vector.findall(".//group/path")), 2)
        self.assertEqual(list(ui_logos.ANDROID_RES.rglob("*.png")), [])
        ui_logos.native_launchers(check=True)

    def test_native_foreground_preserves_official_paths_strokes_and_theme_palette(self):
        for theme, qualifier in [("light", ""), ("dark", "-night"), ("mono", "")]:
            source = ui_logos.launcher_symbol(theme)
            canonical = [node for node in source.iter() if ui_logos.tag(node) == "path"]
            name = "monochrome" if theme == "mono" else "foreground"
            vector = ET.parse(ui_logos.ANDROID_RES / f"drawable{qualifier}/ic_launcher_{name}.xml").getroot()
            self.assertEqual(vector.attrib[self.android + "viewportWidth"], "108")
            self.assertEqual(vector.attrib[self.android + "viewportHeight"], "108")
            actual = vector.findall(".//path")
            self.assertEqual(len(actual), 2)
            for expected, path in zip(canonical, actual):
                self.assertEqual(path.attrib[self.android + "pathData"], expected.attrib["d"])
                self.assertEqual(path.attrib[self.android + "strokeWidth"], expected.attrib["stroke-width"])
                self.assertEqual(path.attrib[self.android + "strokeLineCap"], "round")
                self.assertEqual(path.attrib[self.android + "strokeColor"],
                                 "#FFFFFF" if theme == "mono" else expected.attrib["stroke"])
            if theme != "mono":
                colors = ET.parse(ui_logos.ANDROID_RES / f"values{qualifier}/launcher_colors.xml").getroot()
                self.assertEqual(colors.find("color").text, source.find(ui_logos.SVG_NAMESPACE + "rect").attrib["fill"])

    def test_all_ink_is_inside_the_66dp_safe_circle_even_with_round_caps(self):
        vector = ET.parse(ui_logos.ANDROID_RES / "drawable/ic_launcher_foreground.xml").getroot()
        group = vector.find("group")
        sx, sy = [float(group.attrib[self.android + key]) for key in ("scaleX", "scaleY")]
        tx, ty = [float(group.attrib[self.android + key]) for key in ("translateX", "translateY")]
        self.assertEqual(sx, sy)
        # Independent analytic envelope of the source C arc (radius20, endpoints42,15/49,
        # 5-unit round stroke), plus the three round bars. Entire circle is conservative.
        cx, cy = 42 - math.sqrt(20 ** 2 - 17 ** 2), 32
        self.assertLessEqual(math.hypot(cx * sx + tx - 54, cy * sy + ty - 54) + 22.5 * sx, 33)
        for x, top, bottom in [(27, 27, 37), (34, 21, 43), (41, 26, 38)]:
            for y in (top, bottom):
                self.assertLessEqual(math.hypot(x * sx + tx - 54, y * sy + ty - 54) + 1.5 * sx, 33)

    def test_ios_target_binds_svg_icon_composer_bundle_and_both_appearances(self):
        project = (ui_logos.MOBILE / "iosApp/Cantos.xcodeproj/project.pbxproj").read_text()
        self.assertIn("ASSETCATALOG_COMPILER_APPICON_NAME = AppIcon;", project)
        self.assertIn("lastKnownFileType = folder.icon;", project)
        self.assertIn("AppIcon.icon in Resources", project)
        descriptor = json.loads((ui_logos.IOS_ICONS / "icon.json").read_text())
        layer = descriptor["groups"][0]["layers"][0]
        self.assertEqual(layer["image-name"], "cantos-light.svg")
        self.assertEqual(layer["image-name-specializations"], [{"appearance": "dark", "value": "cantos-dark.svg"}])
        self.assertFalse(layer["glass"])
        self.assertEqual(descriptor["supported-platforms"], {"squares": ["iOS"]})
        for theme in ("light", "dark"):
            svg = ET.parse(ui_logos.IOS_ICONS / f"Assets/cantos-{theme}.svg").getroot()
            self.assertEqual(svg.attrib["viewBox"], "0 0 64 64")
            self.assertEqual(svg.attrib["width"], "1024")
            paths = [node for node in svg.iter() if ui_logos.tag(node) == "path"]
            self.assertEqual(len(paths), 4)
            self.assertTrue(all(set(node.attrib) == {"d", "fill"} for node in paths))
            self.assertIn("A22.5 22.5 0 1 0", paths[0].attrib["d"])
            self.assertIn("A17.5 17.5 0 1 1", paths[0].attrib["d"])
            self.assertTrue(all("a1.5 1.5" in node.attrib["d"] for node in paths[1:]))
            self.assertFalse(any(ui_logos.tag(node) == "rect" for node in svg.iter()))
        self.assertEqual(list(ui_logos.IOS_ICONS.rglob("*.png")), [])

    def test_changed_source_provenance_is_rejected(self):
        with patch.object(ui_logos, "launcher_sources", return_value={}):
            with self.assertRaisesRegex(ValueError, "source provenance drift"):
                ui_logos.native_launchers(check=True)
