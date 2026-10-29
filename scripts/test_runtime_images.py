"""Format guards reject real legacy bytes, renamed payloads and hidden SVG images."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from runtime_images import image_failure, repository_failures, signature


ROOT = Path(__file__).resolve().parents[1]
LEGACY = json.loads((ROOT / "services/cantos-server/src/contexts/media/image_intake/fixtures/legacy-rejection.json").read_text())
PNG = bytes.fromhex(LEGACY["python-rgb-2x3.png"])
JPEG = bytes.fromhex(LEGACY["pillow-rgb-2x3.jpg"])
AVIF = (ROOT / "scripts/fixtures/runtime-images/red-2x3.avif").read_bytes()


class RuntimeImagePolicy(unittest.TestCase):
    def test_actual_avif_and_plain_svg_are_identified_without_claiming_decode(self):
        self.assertEqual(signature(AVIF), ".avif")
        self.assertIsNone(image_failure("assets/a.avif", AVIF))
        self.assertIsNone(image_failure("assets/a.svg", b'<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1v1z"/></svg>'))

    def test_real_png_jpeg_and_webp_bytes_cannot_be_renamed_or_embedded(self):
        # Independent Pillow-encoded WebP bytes are used only as rejection input.
        webp = bytes.fromhex("524946461c000000574542505650384c0f0000002f018000000710fd8ffe0722a2ff0100")
        for data in (PNG, JPEG, webp):
            for name in ("assets/a.avif", "assets/a.svg", "assets/a.bin"):
                with self.subTest(name=name, kind=signature(data)):
                    self.assertIn("legacy", image_failure(name, data))
        for href in ("data:image/png;base64,aA==", "data:image/jpeg;base64,aA==", "a.webp?hash=1"):
            self.assertIn("legacy", image_failure("a.svg", f'<svg><image href="{href}"/></svg>'.encode()))

    def test_malformed_avif_sequences_and_suffix_spoofs_are_rejected(self):
        for data in (b"", b"not avif", b"\0\0\0\x10ftypavif", b"\0\0\0\x10ftypavis\0\0\0\0"):
            self.assertIsNotNone(image_failure("a.avif", data))
        self.assertIn("suffix", image_failure("a.bin", AVIF))
        self.assertIn("legacy", image_failure("a.png", AVIF))
        self.assertIn("malformed", image_failure("a.svg", b"not XML"))

    def test_docs_only_images_are_allowed_but_new_runtime_and_fixture_debt_is_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            names = ["docs/evidence/screen.png", "apps/a/assets/screen.png", "services/a/fixtures/positive.jpg", "docs/design/new.webp"]
            for name, data in zip(names, (PNG, PNG, JPEG, b"RIFF\0\0\0\0WEBP")):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            errors, debt = repository_failures(root, names)
            self.assertEqual(debt, [])
            self.assertEqual([row.split(":", 1)[0] for row in errors], sorted(names[1:]))

    def test_former_media_fixture_paths_have_no_legacy_permission(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            name = "services/cantos-server/src/contexts/media/image_intake/fixtures/python-rgb-2x3.png"
            path = root / name
            path.parent.mkdir(parents=True)
            path.write_bytes(PNG)
            errors, debt = repository_failures(root, [name])
            self.assertEqual(debt, [])
            self.assertIn("legacy/unsupported", errors[0])

    def test_nonimage_bmff_audio_and_fonts_remain_outside_image_policy(self):
        audio_header = b"\0\0\0\x10ftypM4A \0\0\0\0"
        self.assertIsNone(image_failure("assets/chapter.m4a", audio_header))
        self.assertIsNone(image_failure("assets/font.ttf", b"\0\1\0\0font"))
        self.assertIsNone(image_failure("docs/archive/notes.txt", b"BM25 ranking and historical notes are text, not a bitmap."))
