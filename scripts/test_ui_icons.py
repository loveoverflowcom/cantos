import unittest
import xml.etree.ElementTree as ET
from ui_icons import ALLOWED, ASSETS, CMP_OUTPUT, OUTPUT, node_markup, render, render_kotlin
import json


class UiIcons(unittest.TestCase):
    def test_official_hashes_and_generated_anatomy_match(self):
        self.assertEqual(OUTPUT.read_text(), render())
        self.assertEqual(CMP_OUTPUT.read_text(), render_kotlin())

    def test_each_official_icon_has_native_visible_path_data(self):
        kotlin = render_kotlin()
        manifest = json.loads((ASSETS / "manifest.json").read_text())
        for name in manifest["icons"]:
            self.assertIn(f'CantosIcon("{name}", "0 0 24 24", listOf(', kotlin)
        self.assertNotIn('listOf("M0 0h24v24H0V0z")', kotlin)

    def test_untrusted_svg_features_cannot_enter_generated_code(self):
        for markup in ['<script/>', '<path onload="alert(1)"/>', '<foreignObject/>', '<g>text</g>']:
            with self.subTest(markup=markup), self.assertRaises(ValueError):
                node_markup(ET.fromstring(markup))
        self.assertEqual(set(ALLOWED), {"g", "rect", "path"})
