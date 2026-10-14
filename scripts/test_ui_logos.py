import json
import unittest
import xml.etree.ElementTree as ET

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
