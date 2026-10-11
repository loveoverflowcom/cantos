import json
import re
import unittest
from studio_tokens import ROOT, SOURCE, OUTPUT, THEATRE_OUTPUT, KOTLIN_OUTPUT, render, render_kotlin, variable


def luminance(color):
    values = [int(color[index:index+2], 16) / 255 for index in (1, 3, 5)]
    linear = [value/12.92 if value <= .04045 else ((value+.055)/1.055)**2.4 for value in values]
    return sum(value * weight for value, weight in zip(linear, (.2126, .7152, .0722)))


def contrast(first, second):
    dark, light = sorted((luminance(first), luminance(second)))
    return (light+.05)/(dark+.05)


class StudioTokens(unittest.TestCase):
    def test_contrast_oracle_is_anchored_and_both_themes_meet_document_floors(self):
        self.assertAlmostEqual(contrast("#000000", "#ffffff"), 21)
        self.assertEqual(contrast("#654390", "#654390"), 1)
        source = json.loads(SOURCE.read_text())
        for theme, colors in source["themes"].items():
            for first, second, floor in source["contrast_pairs"]:
                self.assertGreaterEqual(contrast(colors[first], colors[second]), floor, (theme, first, second))

    def test_generated_css_and_consumed_roles_match_source(self):
        self.assertEqual(OUTPUT.read_text(), render(SOURCE.read_bytes()))
        self.assertEqual(THEATRE_OUTPUT.read_text(), render(SOURCE.read_bytes()))
        self.assertEqual(KOTLIN_OUTPUT.read_text(), render_kotlin(SOURCE.read_bytes()))
        source = json.loads(SOURCE.read_text())
        roles = set(source["logical_dimensions"]) | set(source["themes"]["light"])
        defined = {variable(role) for role in roles}
        styles = "\n".join((ROOT / path).read_text() for path in
                           ("apps/web/studio.css", "apps/theatre-web/theatre.css"))
        used = set(re.findall(r"var\((--cantos-[a-z-]+)\)", styles))
        self.assertLessEqual(used, defined)
        self.assertEqual(set(source["themes"]["light"]), set(source["themes"]["dark"]))

    def test_generator_refuses_an_unresolved_palette(self):
        source = json.loads(SOURCE.read_text())
        source["themes"]["light"]["color.primary"] = "<pending>"
        with self.assertRaisesRegex(ValueError, "unresolved or invalid color"):
            render(json.dumps(source).encode())
