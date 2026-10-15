"""The preview font bundle is local, attributable and covers Vietnamese."""

import hashlib
from html.parser import HTMLParser
import json
from pathlib import Path
import re
import struct
import unittest


ROOT = Path(__file__).resolve().parents[1]
BUNDLE = ROOT / "packages/ui-system/shared/assets/noto-fonts"
PREVIEW = ROOT / "packages/ui-system/leptos"


def table(raw, name):
    """Read a named table from these checked-in, trusted TrueType files."""
    count = struct.unpack_from(">H", raw, 4)[0]
    for index in range(count):
        tag, _, offset, length = struct.unpack_from(">4sIII", raw, 12 + index * 16)
        if tag.decode("ascii") == name:
            return raw[offset:offset + length]
    raise AssertionError(f"missing TrueType table {name}")


def unicode_glyph(raw, codepoint):
    """Resolve the BMP Unicode cmap without depending on an installed font tool."""
    cmap = table(raw, "cmap")
    count = struct.unpack_from(">H", cmap, 2)[0]
    for index in range(count):
        platform, encoding, offset = struct.unpack_from(">HHI", cmap, 4 + index * 8)
        if platform != 0 and (platform, encoding) != (3, 1):
            continue
        if struct.unpack_from(">H", cmap, offset)[0] != 4:
            continue
        segments = struct.unpack_from(">H", cmap, offset + 6)[0] // 2
        end_base = offset + 14
        start_base = end_base + segments * 2 + 2
        delta_base = start_base + segments * 2
        range_base = delta_base + segments * 2
        for segment in range(segments):
            end = struct.unpack_from(">H", cmap, end_base + segment * 2)[0]
            start = struct.unpack_from(">H", cmap, start_base + segment * 2)[0]
            if start <= codepoint <= end:
                delta = struct.unpack_from(">h", cmap, delta_base + segment * 2)[0]
                address = range_base + segment * 2
                shift = struct.unpack_from(">H", cmap, address)[0]
                if shift == 0:
                    return (codepoint + delta) % 65536
                glyph = struct.unpack_from(">H", cmap, address + shift + (codepoint - start) * 2)[0]
                return (glyph + delta) % 65536 if glyph else 0
    return 0


class PreviewFonts(unittest.TestCase):
    def test_every_face_has_attributable_bytes_license_and_vietnamese_glyphs(self):
        record = json.loads((BUNDLE / "provenance.json").read_text())
        self.assertEqual(record["license"], "SIL-OFL-1.1")
        license_text = (BUNDLE / "OFL.txt").read_text()
        self.assertIn("Copyright 2022 The Noto Project Authors", license_text)
        self.assertIn("SIL OPEN FONT LICENSE Version 1.1", license_text)
        self.assertEqual(hashlib.sha256((BUNDLE / "OFL.txt").read_bytes()).hexdigest(),
                         record["license_sha256"])
        required = set(range(0x1EA0, 0x1EFA))
        required.update(map(ord, "ĐđƠơƯưĂăÂâÊêÔôÀÁÃÈÉÌÍÒÓÕÙÚÝàáãèéìíòóõùúý"))
        required.update((0x0300, 0x0301, 0x0303, 0x0309, 0x0323))
        self.assertEqual(set(record["files"]), {
            "noto_sans_regular.ttf", "noto_sans_bold.ttf",
            "noto_serif_regular.ttf", "noto_serif_bold.ttf",
        })
        for filename, face in record["files"].items():
            with self.subTest(face=filename):
                raw = (BUNDLE / filename).read_bytes()
                self.assertEqual(hashlib.sha256(raw).hexdigest(), face["sha256"])
                self.assertEqual(raw, (ROOT / face["source_path"]).read_bytes())
                self.assertEqual(struct.unpack_from(">H", table(raw, "OS/2"), 4)[0], face["weight"])
                self.assertTrue(all(unicode_glyph(raw, codepoint) != 0 for codepoint in required),
                                "Vietnamese letters and combining marks must not fall back")
                self.assertTrue(table(raw, "GPOS"), "mark positioning requires OpenType positioning")

    def test_preview_loads_the_complete_bundle_without_a_host_or_remote_font_source(self):
        class Links(HTMLParser):
            def __init__(self):
                super().__init__()
                self.links = []

            def handle_starttag(self, tag, attributes):
                if tag == "link":
                    self.links.append(dict(attributes))

        document = Links()
        document.feed((PREVIEW / "preview.html").read_text())
        self.assertIn({"data-trunk": None, "rel": "copy-dir", "href": "../shared/assets/noto-fonts",
                       "data-target-path": "fonts"}, document.links)
        self.assertIn({"data-trunk": None, "rel": "css", "href": "preview/fonts.css"}, document.links)
        css = (PREVIEW / "preview/fonts.css").read_text()
        css = re.sub(r"/\*.*?\*/", "", css, flags=re.DOTALL)
        self.assertNotIn("local(", css)
        self.assertNotRegex(css, r"https?://|@import")
        faces = re.findall(r"@font-face\s*\{([^}]+)\}", css)
        self.assertEqual(len(faces), 4)
        expected = json.loads((BUNDLE / "provenance.json").read_text())["files"]
        for filename, face in expected.items():
            matches = [block for block in faces if f'url("fonts/{filename}")' in block]
            self.assertEqual(len(matches), 1, filename)
            self.assertIn(f'font-family: "{face["family"]}";', matches[0])
            self.assertIn(f'font-weight: {face["weight"]};', matches[0])
            self.assertIn("font-style: normal;", matches[0])
            self.assertIn("font-display: block;", matches[0])


if __name__ == "__main__":
    unittest.main()
