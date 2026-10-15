"""Independent small masks specify accounting, bounds and grouping semantics."""

import random
import importlib.metadata
import json
from pathlib import Path
import tempfile
import unittest

from visual_compare_core import Rect, analyze_mask


def bitmap(width, height, points):
    return bytes(255 if (x, y) in points else 0
                 for y in range(height) for x in range(width))


class MaskTests(unittest.TestCase):
    def test_empty_diff_has_no_regions_and_zero_ratio(self):
        report, visible = analyze_mask(3, 2, bytes(6))
        self.assertEqual((report["diff_pixels"], report["diff_ratio"], report["regions"]),
                         (0, 0.0, []))
        self.assertEqual(visible, bytes(6))

    def test_rectangle_has_exact_bounds_count_and_local_ratio(self):
        points = {(x, y) for y in range(3, 6) for x in range(5, 9)}
        report, _ = analyze_mask(20, 10, bitmap(20, 10, points))
        region = report["regions"][0]
        self.assertEqual({k: region[k] for k in ("x", "y", "width", "height", "pixels")},
                         dict(x=5, y=3, width=4, height=3, pixels=12))
        self.assertEqual((region["local_ratio"], report["diff_ratio"]), (1, 12 / 200))

    def test_distant_regions_sort_by_pixels_then_position(self):
        points = {(1, 1), (20, 1), (21, 1), (1, 20)}
        report, _ = analyze_mask(30, 30, bitmap(30, 30, points), tile_size=4)
        self.assertEqual([(r["id"], r["x"], r["y"], r["pixels"]) for r in report["regions"]],
                         [(1, 20, 1, 2), (2, 1, 1, 1), (3, 1, 20, 1)])

    def test_adjacent_tiles_merge_without_expanding_exact_pixel_bounds(self):
        report, _ = analyze_mask(20, 20, bitmap(20, 20, {(3, 3), (4, 4)}), tile_size=4)
        self.assertEqual(len(report["regions"]), 1)
        self.assertEqual(report["regions"][0]["width"], 2)
        self.assertEqual(report["regions"][0]["pixels"], 2)

    def test_overlapping_masks_count_union_and_preserve_input(self):
        original = b"\xff" * 8
        report, visible = analyze_mask(4, 2, original,
                                       (Rect(0, 0, 2, 1), Rect(1, 0, 2, 1)))
        self.assertEqual(original, b"\xff" * 8)
        self.assertEqual(visible, b"\x00\x00\x00\xff\xff\xff\xff\xff")
        self.assertEqual((report["excluded_pixels"], report["diff_pixels"],
                          report["diff_ratio"], report["unmasked_diff_ratio"]),
                         (3, 5, 5 / 8, 1))

    def test_invalid_or_vacuous_comparisons_are_rejected(self):
        for args in [(0, 1, b""), (1, 1, b""), (1, 1, b"\x01"),
                     (1, 1, b"\xff", (Rect(0, 0, 1, 1),)),
                     (1, 1, b"\xff", (Rect(1, 0, 1, 1),)),
                     (1, 1, b"\xff", (), 0)]:
            with self.subTest(args=args), self.assertRaises(ValueError):
                analyze_mask(*args)
        for args in [(False, 0, 1, 1), (0, 0, -1, 1), (-1, 0, 1, 1)]:
            with self.assertRaises(ValueError):
                Rect(*args)
        with self.assertRaisesRegex(ValueError, "tile grid"):
            analyze_mask(513, 513, bytes(513 * 513), tile_size=1)

    def test_seeded_masks_agree_with_naive_set_accounting(self):
        rng = random.Random(140)
        for _ in range(100):
            points = {(x, y) for y in range(8) for x in range(8) if rng.random() < .2}
            report, _ = analyze_mask(8, 8, bitmap(8, 8, points), (Rect(0, 0, 2, 2),), 1)
            expected = points - {(0, 0), (0, 1), (1, 0), (1, 1)}
            self.assertEqual(report["diff_pixels"], len(expected))
            self.assertEqual(sum(r["pixels"] for r in report["regions"]), len(expected))
            self.assertEqual(report, analyze_mask(8, 8, bitmap(8, 8, points),
                                                  (Rect(0, 0, 2, 2),), 1)[0])
            for x, y in expected:
                self.assertTrue(any(r["x"] <= x < r["x"] + r["width"] and
                                    r["y"] <= y < r["y"] + r["height"]
                                    for r in report["regions"]))


def pinned_pillow_available():
    try:
        return importlib.metadata.version("Pillow") == "11.3.0"
    except importlib.metadata.PackageNotFoundError:
        return False


@unittest.skipUnless(pinned_pillow_available(), "optional pinned Pillow PNG adapter not installed")
class PngAdapterTests(unittest.TestCase):
    def setUp(self):
        import visual_compare
        self.adapter = visual_compare
        self.Image, _, _ = visual_compare.pillow()
        parent = visual_compare.ROOT / "artifacts/ui-evidence"
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix="visual-test-", dir=parent)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def png(self, name, color=(0, 0, 0, 255), size=(8, 8)):
        path = self.root / name
        self.Image.new("RGBA", size, color).save(path)
        return path

    def compare(self, name, reference, actual, **kwargs):
        return self.adapter.compare(reference, actual, "calibration-v1", kwargs.pop("metadata", {}),
                                    self.root / name, **kwargs)

    def test_real_png_rectangle_emits_exact_mask_boxes_and_crops(self):
        reference = self.png("reference.png")
        actual = self.png("actual.png")
        with self.Image.open(actual) as image:
            image.paste((255, 0, 0, 255), (2, 3, 5, 5))
            image.save(actual)
        output, report = self.compare("rectangle", reference, actual)
        self.assertEqual((report["result"], report["diff_pixels"]), ("different", 6))
        self.assertEqual([report["regions"][0][k] for k in ("x", "y", "width", "height")], [2, 3, 3, 2])
        with self.Image.open(output / "mask.png") as mask:
            self.assertEqual(mask.histogram()[255], 6)
            self.assertEqual(mask.getbbox(), (2, 3, 5, 5))
        for name in ("reference.png", "actual.png", "overlay.png", "diff.png", "heatmap.png",
                     "region-1-actual.png", "report.json", "report.md"):
            self.assertTrue((output / name).is_file(), name)
        self.assertFalse(json.loads((output / "report.json").read_text())["images_inspected"])

    def test_threshold_boundary_is_explicit_and_hidden_rgb_does_not_diff(self):
        reference = self.png("reference.png", (0, 0, 0, 255))
        for channel, expected in ((8, 0), (9, 64)):
            actual = self.png("actual.png", (channel, 0, 0, 255))
            _, report = self.compare(f"threshold-{channel}", reference, actual, threshold=8)
            self.assertEqual(report["diff_pixels"], expected)
        a = self.png("transparent-a.png", (255, 0, 0, 0))
        b = self.png("transparent-b.png", (0, 255, 0, 0))
        self.assertEqual(self.compare("hidden", a, b)[1]["result"], "equal")
        b = self.png("transparent-b.png", (0, 255, 0, 1))
        self.assertEqual(self.compare("alpha", a, b)[1]["diff_pixels"], 64)

    def test_dimension_and_scenario_mismatch_never_resize_or_score(self):
        reference = self.png("reference.png")
        actual = self.png("actual.png", size=(9, 8))
        output, report = self.compare("dimensions", reference, actual)
        self.assertEqual(report["result"], "dimension_mismatch")
        self.assertNotIn("diff_ratio", report)
        self.assertFalse((output / "overlay.png").exists())
        actual = self.png("actual.png")
        _, report = self.compare("scenario", reference, actual,
                                 metadata={"reference": {"theme": "light"}, "actual": {"theme": "dark"}})
        self.assertEqual((report["result"], report["pairing_mismatches"]), ("scenario_mismatch", ["theme"]))

    def test_reviewed_masks_and_baseline_hash_are_required(self):
        reference = self.png("reference.png")
        actual = self.png("actual.png", (255, 0, 0, 255))
        metadata_path = self.root / "metadata.json"
        for invalid in ('{"masks": [], "masks": []}', '{"actual": {"pixel_ratio": NaN}}'):
            metadata_path.write_text(invalid)
            with self.assertRaises(ValueError):
                self.adapter.metadata_from(metadata_path)
        metadata_path.write_text(json.dumps({"masks": [{"x": 0, "y": 0, "width": 2, "height": 2}]}))
        with self.assertRaisesRegex(ValueError, "mask requires"):
            self.adapter.metadata_from(metadata_path)
        metadata_path.write_text(json.dumps({"masks": [dict(x=0, y=0, width=2, height=2,
            reason="calibration clock", reviewed_by="fixture author", review_record="test fixture v1")]}))
        metadata = self.adapter.metadata_from(metadata_path)
        _, report = self.compare("mask", reference, actual, metadata=metadata)
        self.assertEqual((report["diff_pixels"], report["excluded_pixels"]), (60, 4))
        with self.assertRaisesRegex(ValueError, "baseline SHA-256"):
            self.compare("hash", reference, actual, metadata={"baseline": {"sha256": "0" * 64}})

    def test_recheck_policy_pins_reference_and_settings_but_allows_new_capture(self):
        reference = self.png("reference.png")
        actual = self.png("actual.png")
        first = self.compare("first", reference, actual)[1]
        second = self.compare("second", reference, actual,
                              metadata={"actual": {"source_sha": "new build", "captured_at": "new time"}})[1]
        changed = self.compare("changed", reference, actual, threshold=8)[1]
        self.assertEqual(first["recheck_policy_sha256"], second["recheck_policy_sha256"])
        self.assertNotEqual(first["recheck_policy_sha256"], changed["recheck_policy_sha256"])
        with self.assertRaises(FileExistsError):
            self.compare("first", reference, actual)
        bad = self.root / "invalid.png"
        bad.write_bytes(b"not a PNG")
        with self.assertRaisesRegex(ValueError, "PNG signature"):
            self.compare("bad", bad, actual)
        import struct
        bad.write_bytes(b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" + struct.pack(">II", 4001, 4001))
        with self.assertRaisesRegex(ValueError, "above 16 million"):
            self.compare("huge", bad, actual)
        bad.write_bytes(b"\x89PNG\r\n\x1a\n" + struct.pack(">I", 13) + b"IHDR" +
                        struct.pack(">II", 8, 8) + bytes([16, 0, 0, 0, 0]))
        with self.assertRaisesRegex(ValueError, "16-bit quantization"):
            self.compare("sixteen-bit", bad, actual)


if __name__ == "__main__":
    unittest.main()
