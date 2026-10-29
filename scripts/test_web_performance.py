"""Independent synthetic build outputs exercise accounting and fail-closed inventory."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from web_performance import brotli_adapter, compare_assets, inventory, local_reference, measure, prepare


class InventoryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.dist = Path(self.temporary.name)

    def write(self, name, data):
        path = self.dist / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return data

    def bundle(self):
        self.write("index.html", b'''<!doctype html><link rel="stylesheet" href="/style-0123456789abcdef.css">
<link rel="modulepreload" href="/app-0123456789abcdef.js"><link rel="preload" href="/app-0123456789abcdef_bg.wasm" as="fetch" type="application/wasm">
<script type="module">import init from '/app-0123456789abcdef.js'; await init({module_or_path: '/app-0123456789abcdef_bg.wasm'});</script>''')
        self.write("style-0123456789abcdef.css", b'body{font-family:Example} @font-face{src:url("assets/font.woff2")}')
        self.write("app-0123456789abcdef.js", b"return WebAssembly.instantiateStreaming(module, imports)")
        self.write("app-0123456789abcdef_bg.wasm", b"\0asm\x01\0\0\0")
        self.write("assets/font.woff2", b"synthetic font")
        self.write("assets/artwork.webp", b"synthetic image")
        self.write("assets/cover.avif", b"synthetic avif")

    def test_counts_deduplicated_html_links_and_separates_css_and_other_assets(self):
        self.bundle()
        report = inventory(self.dist)
        rows = {row["path"]: row for row in report["assets"]}
        self.assertEqual(report["totals"]["html-linked"]["files"], 4)
        self.assertEqual(rows["assets/font.woff2"]["role"], "css-referenced")
        self.assertEqual(rows["assets/artwork.webp"]["role"], "other-output")
        self.assertEqual(rows["assets/cover.avif"]["expected_content_type"], "image/avif")
        wasm = rows["app-0123456789abcdef_bg.wasm"]
        self.assertEqual(wasm["expected_content_type"], "application/wasm")
        self.assertTrue(wasm["hash_shaped_filename"])
        self.assertEqual(wasm["immutable_cache_eligibility"], "NOT_VERIFIED")
        self.assertEqual(wasm["sha256"], hashlib.sha256(b"\0asm\x01\0\0\0").hexdigest())
        self.assertEqual(wasm["gzip_bytes"], 28)
        self.assertEqual(report["totals"]["all-output"]["raw_bytes"], sum(path.stat().st_size for path in self.dist.rglob("*") if path.is_file()))
        self.assertIsNone(report["totals"]["all-output"]["brotli_bytes"])
        self.assertTrue(report["loader_static_observation"][0]["instantiate_streaming_call_present"])

    def test_optional_brotli_adapter_is_reported_without_requiring_installation(self):
        self.bundle()
        report = inventory(self.dist, lambda data: b"fake")
        self.assertEqual(report["totals"]["all-output"]["brotli_bytes"], 6 * 4)

    def test_brotli_cli_fallback_passes_bytes_and_literal_args_without_a_shell(self):
        with patch.dict(sys.modules, {"brotli": None}), \
                patch("web_performance.shutil.which", return_value="/literal path/brotli"), \
                patch("web_performance.command", return_value="brotli 1.1.0"), \
                patch("web_performance.subprocess.run", return_value=subprocess.CompletedProcess([], 0, b"compressed")) as run:
            compress, metadata = brotli_adapter()
            self.assertEqual(compress(b"synthetic wasm"), b"compressed")
            self.assertEqual(metadata["adapter"], "cli")
            self.assertEqual(metadata["version"], "brotli 1.1.0")
            run.assert_called_once_with(["/literal path/brotli", "--quality=11", "--stdout"],
                                        input=b"synthetic wasm", capture_output=True, check=True)
        with patch.dict(sys.modules, {"brotli": None}), \
                patch("web_performance.shutil.which", return_value=None):
            compress, metadata = brotli_adapter()
            self.assertIsNone(compress)
            self.assertEqual(metadata["status"], "UNSUPPORTED")

    def test_missing_shell_bundle_or_css_reference_fails_instead_of_reporting_partial_bytes(self):
        with self.assertRaisesRegex(ValueError, "real directory"):
            inventory(self.dist / "missing-dist")
        with self.assertRaisesRegex(ValueError, "index.html is missing"):
            inventory(self.dist)
        self.bundle()
        (self.dist / "app-0123456789abcdef_bg.wasm").unlink()
        with self.assertRaisesRegex(ValueError, "HTML references missing"):
            inventory(self.dist)
        self.bundle()
        (self.dist / "assets/font.woff2").unlink()
        with self.assertRaisesRegex(ValueError, "CSS references missing"):
            inventory(self.dist)
        self.write("index.html", b"<html><body>not a Trunk bundle</body></html>")
        with self.assertRaisesRegex(ValueError, "no linked WASM"):
            inventory(self.dist)

    def test_traversal_symlink_and_unsupported_base_are_rejected(self):
        self.bundle()
        with self.assertRaisesRegex(ValueError, "escapes dist"):
            local_reference("/%2e%2e/private", Path("index.html"), self.dist)
        (self.dist / "escape").symlink_to(self.dist.parent)
        with self.assertRaisesRegex(ValueError, "dist symlinks"):
            inventory(self.dist)
        (self.dist / "escape").unlink()
        self.write("index.html", b'<base href="/deployment/"><script></script>')
        with self.assertRaisesRegex(ValueError, "deployment mapping"):
            inventory(self.dist)

    def test_byte_changes_under_same_url_are_visible_in_deployment_comparison(self):
        self.bundle()
        before = dict(inventory(self.dist), schema_version=1, app="cantos-theatre")
        self.write("assets/font.woff2", b"changed font")
        self.write("extra.txt", b"new")
        after = dict(inventory(self.dist), schema_version=1, app="cantos-theatre")
        result = compare_assets(json.loads(json.dumps(before)), after)
        self.assertEqual(result["same_url_changed_bytes"], ["assets/font.woff2"])
        self.assertEqual(result["added_urls"], ["extra.txt"])
        self.assertEqual(result["removed_urls"], [])
        self.assertFalse(result["deployment_or_rollback_observed"])
        with self.assertRaisesRegex(ValueError, "schema/app"):
            compare_assets(dict(before, app="cantos-studio"), after)

    def test_provenance_change_is_rejected_before_inspecting_a_stale_build(self):
        (self.dist / "provenance.json").write_text(json.dumps({
            "app": "cantos-studio", "source_before_build": {"revision": "before"}}))
        with patch("web_performance.source_snapshot", return_value={"revision": "after"}):
            with self.assertRaisesRegex(ValueError, "source changed during the build"):
                measure("cantos-studio", self.dist, None)
        self.assertFalse((self.dist / "inventory.json").exists())

    def test_output_cannot_overwrite_source_or_existing_evidence(self):
        with patch("web_performance.ROOT", self.dist):
            with self.assertRaisesRegex(ValueError, "beneath artifacts/web-performance"):
                prepare("cantos-studio", self.dist / "docs/evidence")
            output = self.dist / "artifacts/web-performance/existing"
            output.mkdir(parents=True)
            with self.assertRaises(FileExistsError):
                prepare("cantos-studio", output)

    def test_inherited_trunk_overrides_are_rejected_before_output_or_build(self):
        for key in ("TRUNK_BUILD_DIST", "TRUNK_CONFIG", "TRUNK_BUILD_FEATURES",
                    "TRUNK_TOOLS_WASM_OPT", "TRUNK_BUILD_MINIFY"):
            with self.subTest(key=key), patch.dict("os.environ", {key: "synthetic-override"}):
                with self.assertRaisesRegex(ValueError, "inherited Trunk overrides"):
                    prepare("cantos-studio", self.dist / "new-output")
                self.assertFalse((self.dist / "new-output").exists())

    def test_inventory_uses_only_the_command_owned_dist(self):
        metadata = {"app": "cantos-studio", "source_before_build": {"revision": "current"}}
        (self.dist / "provenance.json").write_text(json.dumps(metadata))
        self.bundle()  # An unrelated old index/WASM exists but output/dist was never built.
        with patch("web_performance.source_snapshot", return_value={"revision": "current"}), \
                patch("web_performance.brotli_adapter", return_value=(None, {"status": "UNSUPPORTED"})):
            with self.assertRaisesRegex(ValueError, "real directory"):
                measure("cantos-studio", self.dist, None)
        self.assertFalse((self.dist / "inventory.json").exists())


if __name__ == "__main__":
    unittest.main()
