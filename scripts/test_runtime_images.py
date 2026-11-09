"""Format guards reject real legacy bytes, renamed payloads and hidden SVG images."""

import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from runtime_images import AVIF_REJECTIONS, image_failure, repository_failures, signature


ROOT = Path(__file__).resolve().parents[1]
LEGACY = json.loads((ROOT / "services/cantos-server/src/contexts/media/image_intake/fixtures/legacy-rejection.json").read_text())
PNG = bytes.fromhex(LEGACY["python-rgb-2x3.png"])
JPEG = bytes.fromhex(LEGACY["pillow-rgb-2x3.jpg"])
WEBP = (ROOT / "scripts/fixtures/runtime-images/red-2x3.webp").read_bytes()
AVIF = (ROOT / "scripts/fixtures/runtime-images/red-2x3.avif").read_bytes()


# The four CMP AVIF resources retired by #770, with their former pinned digests and
# Git blob ids. The guard used to admit these exact bytes while the WebP fallback was absent.
RETIRED_CMP = {
    "apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/composeResources/files/artwork/den_khuya.avif":
        ("d053df56699afb2c1ff6031d3a34d636a86c802473b69f2e383bf2e3c76e392f", "caeb915b2f569477bba281a9da3acd98947fbaf1"),
    "apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/composeResources/files/artwork/son_ha_moon.avif":
        ("2abd065841decb84fbe8741f725ed553534f60b50e907ff34d0524dc5a95c171", "710f5566027a9a04da7fe1898593c9d4927c4f8e"),
    "apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/artwork/cantos_cover_fallback.avif":
        ("dfcf6b4d8405426bbd833db6629b6b14c9ca9a4990d9c85bd82321d759b5e4a5", "90fa30a6b281db33f21fd7caeb5bbcadd8a2acd7"),
    "apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/artwork/library_expressive.avif":
        ("88fdbb675291ac016955976b91504c9e5a423293160b9336f563228853cb0f2c", "25b12720b4b29459b76c8e9acad4f3726b819791"),
}
WEBP_FALLBACK = "apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/artwork/cantos_cover_fallback.webp"


def historical_blob(blob):
    """Exact retired bytes from local Git history, or None in a shallow checkout."""
    result = subprocess.run(["git", "cat-file", "blob", blob], cwd=ROOT, capture_output=True, check=False)
    return result.stdout if result.returncode == 0 else None


class RuntimeImagePolicy(unittest.TestCase):
    def test_actual_webp_and_plain_svg_are_identified_without_claiming_decode(self):
        self.assertEqual(signature(AVIF), ".avif")
        self.assertIn("legacy", image_failure("assets/a.avif", AVIF))
        self.assertIsNone(image_failure("assets/a.webp", WEBP))
        self.assertIsNone(image_failure("assets/a.svg", b'<svg xmlns="http://www.w3.org/2000/svg"><path d="M0 0h1v1z"/></svg>'))

    def test_real_png_jpeg_and_webp_bytes_cannot_be_renamed_or_embedded(self):
        # Independent Pillow-encoded WebP bytes are used only as rejection input.
        webp = bytes.fromhex("524946461c000000574542505650384c0f0000002f018000000710fd8ffe0722a2ff0100")
        for data in (PNG, JPEG, AVIF):
            for name in ("assets/a.webp", "assets/a.svg", "assets/a.bin"):
                with self.subTest(name=name, kind=signature(data)):
                    self.assertIn("legacy", image_failure(name, data))
        self.assertIn("opaque", image_failure("assets/a.webp", webp))
        for href in ("data:image/png;base64,aA==", "data:image/jpeg;base64,aA==", "a.avif?hash=1"):
            self.assertIn("legacy", image_failure("a.svg", f'<svg><image href="{href}"/></svg>'.encode()))

    def test_malformed_sequences_and_suffix_spoofs_are_rejected(self):
        for data in (b"", b"not avif", b"\0\0\0\x10ftypavif", b"\0\0\0\x10ftypavis\0\0\0\0"):
            self.assertIsNotNone(image_failure("a.avif", data))
        self.assertIn("suffix", image_failure("a.bin", WEBP))
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
            errors = repository_failures(root, names)
            self.assertEqual([row.split(":", 1)[0] for row in errors], sorted(names[1:]))

    def test_former_media_fixture_paths_have_no_legacy_permission(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            name = "services/cantos-server/src/contexts/media/image_intake/fixtures/python-rgb-2x3.png"
            path = root / name
            path.parent.mkdir(parents=True)
            path.write_bytes(PNG)
            errors = repository_failures(root, [name])
            self.assertIn("legacy/unsupported", errors[0])

    def test_nonimage_bmff_audio_and_fonts_remain_outside_image_policy(self):
        audio_header = b"\0\0\0\x10ftypM4A \0\0\0\0"
        self.assertIsNone(image_failure("assets/chapter.m4a", audio_header))
        self.assertIsNone(image_failure("assets/font.ttf", b"\0\1\0\0font"))
        self.assertIsNone(image_failure("docs/archive/notes.txt", b"BM25 ranking and historical notes are text, not a bitmap."))

    def test_retired_cmp_avif_resources_are_refused_with_or_without_the_webp_fallback(self):
        for restored_fallback in (False, True):
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                names = list(RETIRED_CMP)
                if restored_fallback:
                    names.append(WEBP_FALLBACK)
                for name in names:
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(WEBP if name == WEBP_FALLBACK else AVIF)
                errors = repository_failures(root, names)
                with self.subTest(restored_fallback=restored_fallback):
                    self.assertEqual([row.split(":", 1)[0] for row in errors], sorted(RETIRED_CMP))
                    self.assertTrue(all("legacy/unsupported" in row for row in errors))

    def test_exact_retired_cmp_bytes_have_no_hash_allowance(self):
        restored = {name: historical_blob(blob) for name, (_, blob) in RETIRED_CMP.items()}
        if any(data is None for data in restored.values()):
            self.skipTest("retired CMP blobs are absent from this (shallow) Git checkout")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, data in restored.items():
                self.assertEqual(hashlib.sha256(data).hexdigest(), RETIRED_CMP[name][0])
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
            errors = repository_failures(root, list(restored))
            self.assertEqual([row.split(":", 1)[0] for row in errors], sorted(RETIRED_CMP))

    def test_only_frozen_test_vectors_carry_a_hash_allowance(self):
        for name in AVIF_REJECTIONS:
            with self.subTest(name=name):
                self.assertNotIn("composeResources", name)
                self.assertTrue("/fixtures/" in name or "Test/resources/" in name)
