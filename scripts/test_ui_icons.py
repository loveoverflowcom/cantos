import unittest
import xml.etree.ElementTree as ET
from ui_icons import ALLOWED, OUTPUT, node_markup, render


class UiIcons(unittest.TestCase):
    def test_official_hashes_and_generated_anatomy_match(self):
        self.assertEqual(OUTPUT.read_text(), render())

    def test_untrusted_svg_features_cannot_enter_generated_code(self):
        for markup in ['<script/>', '<path onload="alert(1)"/>', '<foreignObject/>', '<g>text</g>']:
            with self.subTest(markup=markup), self.assertRaises(ValueError):
                node_markup(ET.fromstring(markup))
        self.assertEqual(set(ALLOWED), {"g", "rect", "path"})
