"""Exercise the real capture adapter across a mocked Android CLI process boundary."""

import base64
import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

import render_compose_preview as preview


PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aXioAAAAASUVORK5CYII="
)
TREE = {"nodes": [{"text": "Phát: Ánh đèn cuối sân khấu", "enabled": True}]}


class CaptureTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        for app in preview.MOBILE_DIRS:
            mobile = self.root / preview.MOBILE_DIRS[app]
            mobile.mkdir(parents=True)
            (mobile / "settings.gradle.kts").write_text("// mock Gradle project\n")
            (mobile / "Screen.kt").write_text("@Preview @Composable fun ScreenPreview() = ProductionScreen()\n")
        self.cli = self.root / "tools with spaces" / "android"
        self.cli.parent.mkdir()
        self.cli.write_text(
            f"#!{sys.executable}\n" + f"ROOT = {str(self.root)!r}\n" + f"PNG = {PNG!r}\n"
            + f"TREE = {TREE!r}\n" + '''
import json
import sys
import time
from pathlib import Path
root = Path(ROOT)
args = sys.argv[1:]
with (root / "calls.jsonl").open("a") as log:
    log.write(json.dumps(args) + "\\n")
mode_file = root / "mode.txt"
mode = mode_file.read_text() if mode_file.exists() else "ok"
if args == ["--version"]:
    print("Android CLI test double 1")
elif "--help" in args:
    print("legacy SDK manager" if mode == "legacy" else "--print-semantics --output-image-file --project")
elif args == ["studio", "check"]:
    if mode == "no-ide":
        print("no running Studio", file=sys.stderr)
        sys.exit(5)
    print("pid: 123\\nversion: test Quail 2\\nProjects: READY " + ROOT)
else:
    if mode == "timeout":
        print("partial render output", flush=True)
        time.sleep(5)
    if mode == "exit":
        print("preview compilation failed", file=sys.stderr)
        sys.exit(7)
    image = Path(next(x.split("=", 1)[1] for x in args if x.startswith("--output-image-file=")))
    if mode != "missing-png":
        image.write_bytes(b"broken image" if mode == "bad-png" else PNG)
    if mode == "source-change":
        Path(args[-2]).write_text("// changed during capture\\n")
    print("render complete")
    if mode == "bad-semantics":
        print("no tree")
    elif mode == "ambiguous":
        print(json.dumps({"log": "json log"}))
        print(json.dumps(TREE))
    else:
        print(json.dumps(TREE, ensure_ascii=False, indent=2))
''', encoding="utf-8",
        )
        self.cli.chmod(0o755)

    def args(self, app="cantos", *extra):
        return preview.parser().parse_args([
            "--app", app, "--file", preview.MOBILE_DIRS[app] + "/Screen.kt",
            "--composable", "ScreenPreview", "--android-cli", str(self.cli), *extra,
        ])

    def capture(self, args):
        with contextlib.redirect_stdout(io.StringIO()):
            return preview.capture(args, self.root)

    def bundles(self, app="cantos"):
        return sorted((self.root / "artifacts/ui-evidence/compose-preview" / app).glob("*"))

    def calls(self):
        path = self.root / "calls.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def test_capture_keeps_real_command_selection_and_opaque_tree_for_both_products(self):
        for app in preview.MOBILE_DIRS:
            with self.subTest(app=app):
                self.assertEqual(self.capture(self.args(app, "--pid", "123", "--label", "dark vi-VN large")), 0)
                bundle, = self.bundles(app)
                metadata = json.loads((bundle / "capture.json").read_text())
                self.assertEqual(metadata["status"], "PASS")
                self.assertEqual(metadata["dimensions"], {"width_px": 1, "height_px": 1})
                self.assertFalse(metadata["image_inspected"])
                self.assertFalse(metadata["semantics_asserted"])
                self.assertEqual(metadata["image_sha256"], preview.sha256(bundle / "preview.png"))
                self.assertEqual(json.loads((bundle / "semantics.json").read_text()), TREE)
                self.assertIn("--pid=123", metadata["command"])
                self.assertIn(f"--project={self.root / preview.MOBILE_DIRS[app]}", metadata["command"])
                self.assertEqual(metadata["command"][-1], "ScreenPreview")
                self.assertEqual((bundle / "preview.png").read_bytes(), PNG)
        self.assertEqual(len(self.calls()), 8)

    def test_each_run_has_fresh_output_and_cannot_reuse_an_old_png_on_failure(self):
        self.capture(self.args())
        old, = self.bundles()
        (self.root / "mode.txt").write_text("missing-png")
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(self.args())
        self.assertEqual(failure.exception.code, 1)
        self.assertEqual(len(self.bundles()), 2)
        new, = [bundle for bundle in self.bundles() if bundle != old]
        self.assertFalse((new / "preview.png").exists())
        self.assertEqual(json.loads((new / "capture.json").read_text())["status"], "FAIL")
        self.assertEqual((old / "preview.png").read_bytes(), PNG)

    def test_render_errors_invalid_outputs_and_source_changes_never_pass(self):
        for mode in ("exit", "bad-png", "bad-semantics", "ambiguous", "source-change"):
            with self.subTest(mode=mode):
                (self.root / "mode.txt").write_text(mode)
                before = set(self.bundles())
                with self.assertRaises(preview.PreviewError) as failure:
                    self.capture(self.args())
                self.assertEqual(failure.exception.code, 1)
                new, = set(self.bundles()) - before
                self.assertEqual(json.loads((new / "capture.json").read_text())["status"], "FAIL")
                self.assertTrue((new / "render.stdout.log").is_file())
                self.assertTrue((new / "render.stderr.log").is_file())
                self.assertFalse((new / "semantics.json").exists())

    def test_legacy_cli_and_unavailable_ide_are_unsupported_before_render(self):
        for mode in ("legacy", "no-ide"):
            with self.subTest(mode=mode):
                (self.root / "mode.txt").write_text(mode)
                before = set(self.bundles())
                with self.assertRaises(preview.PreviewError) as failure:
                    self.capture(self.args())
                self.assertEqual(failure.exception.code, preview.UNSUPPORTED)
                new, = set(self.bundles()) - before
                self.assertEqual(json.loads((new / "capture.json").read_text())["status"], "UNSUPPORTED")
                self.assertFalse((new / "render.stdout.log").exists())

    def test_timeout_is_bounded_and_partial_logs_are_retained(self):
        (self.root / "mode.txt").write_text("timeout")
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(self.args("cantos", "--timeout-seconds", "1"))
        self.assertEqual(failure.exception.code, 1)
        self.assertIn("timed out", str(failure.exception))
        bundle, = self.bundles()
        self.assertIn("partial render output", (bundle / "render.stdout.log").read_text())
        self.assertEqual(json.loads((bundle / "capture.json").read_text())["status"], "FAIL")

    def test_dry_run_neither_probes_cli_nor_writes_evidence(self):
        args = self.args("cantos", "--dry-run")
        args.android_cli = "cantos-no-such-cli"
        self.assertEqual(self.capture(args), 0)
        self.assertEqual(self.calls(), [])
        self.assertFalse((self.root / "artifacts").exists())

    def test_missing_project_and_cli_are_unsupported(self):
        args = self.args()
        (self.root / preview.MOBILE_DIRS["cantos"] / "settings.gradle.kts").unlink()
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(args)
        self.assertEqual(failure.exception.code, preview.UNSUPPORTED)
        self.assertEqual(self.calls(), [])
        (self.root / preview.MOBILE_DIRS["cantos"] / "settings.gradle").write_text("// project\n")
        args.android_cli = "cantos-no-such-cli"
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(args)
        self.assertEqual(failure.exception.code, preview.UNSUPPORTED)
        self.assertEqual(self.calls(), [])

    def test_cross_product_path_traversal_and_option_like_names_are_usage_errors(self):
        for source, composable in (
            ("apps/cantos-studio/mobile/Screen.kt", "ScreenPreview"),
            ("../Screen.kt", "ScreenPreview"),
            ("apps/cantos/mobile/Screen.kt", "--option"),
        ):
            args = self.args()
            args.file = Path(source)
            args.composable = composable
            with self.assertRaises(preview.PreviewError) as failure:
                self.capture(args)
            self.assertEqual(failure.exception.code, 2)
        self.assertEqual(self.calls(), [])

    @unittest.skipUnless(os.name == "posix", "symlink support")
    def test_symlinked_source_and_evidence_cannot_escape_selected_locations(self):
        mobile = self.root / preview.MOBILE_DIRS["cantos"]
        source = mobile / "Screen.kt"
        source.unlink()
        source.symlink_to(self.root / preview.MOBILE_DIRS["cantos-studio"] / "Screen.kt")
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(self.args())
        self.assertEqual(failure.exception.code, 2)
        source.unlink()
        source.write_text("// preview\n")
        (self.root / "artifacts").symlink_to(self.root / "elsewhere")
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(self.args())
        self.assertEqual(failure.exception.code, 2)
        self.assertEqual(self.calls(), [])

    def test_semantics_parser_preserves_json_and_refuses_no_tree_or_multiple_trees(self):
        for tree in (TREE, [TREE], []):
            self.assertEqual(preview.semantics_json(json.dumps(tree, indent=2)), tree)
        for text in ("", "{broken}", "{}\n[]\n", "{} []"):
            with self.assertRaises(preview.PreviewError) as failure:
                preview.semantics_json(text)
            self.assertEqual(failure.exception.code, 1)


if __name__ == "__main__":
    unittest.main()
