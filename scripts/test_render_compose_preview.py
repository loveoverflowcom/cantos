"""Exercise the real capture adapter across a mocked Android CLI process boundary."""

import base64
import contextlib
import io
import json
import os
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

import render_compose_preview as preview


PNG = base64.b64decode(
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aXioAAAAASUVORK5CYII="
)
TREE = {"nodes": [{"text": "Phát: Ánh đèn cuối sân khấu", "enabled": True}]}
TEXT_TREE = r'''ComposeViewAdapter (left: 0, top: 0, right: 977, bottom: 2205)
  ComposeView (left: 0, top: 0, right: 977, bottom: 2205)
    View (left: 0, top: 0, right: 977, bottom: 2205)
      View (left: 0, top: 0, right: 977, bottom: 2205)
        ScrollView (left: 0, top: 0, right: 977, bottom: 2205)
          TextView (left: 63, top: 63, right: 676, bottom: 120) text="Trường nhập · Người dẫn chuyện · Ỗ, Ữ"
          EditText (left: 63, top: 162, right: 914, bottom: 315) text="Ánh đèn cuối sân khấu · Ỗ, Ữ"
            TextView (left: 105, top: 183, right: 293, bottom: 231) text="Tên tác phẩm"
            View (left: 105, top: 200, right: 872, bottom: 326)
          TextView (left: 63, top: 336, right: 510, bottom: 373) text="Selection: 0..7; composition: null"
          EditText (left: 63, top: 415, right: 914, bottom: 568) text="1,25"
            TextView (left: 105, top: 436, right: 357, bottom: 484) text="Tốc độ tham khảo"
            View (left: 105, top: 453, right: 872, bottom: 579)
          TextView (left: 63, top: 589, right: 454, bottom: 626) text="Selection: 0..4; raw text: 1,25"
          EditText (left: 63, top: 668, right: 914, bottom: 1062) text="Người dẫn chuyện: Ngày mai, mình có diễn tiếp không?\nỖ, Ữ, Nguyễn Thị Minh Phương"
            TextView (left: 105, top: 689, right: 226, bottom: 737) text="Lời thoại"
            View (left: 105, top: 737, right: 872, bottom: 908)
            TextView (left: 105, top: 940, right: 791, bottom: 977) text="Hãy thử chọn, sao chép và nhập tiếng Việt có dấu."
            TextView (left: 105, top: 977, right: 872, bottom: 1062) text="Tên tác phẩm cần có ít nhất 3 ký tự. Nội dung chưa bị xóa."
'''


class CaptureTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        for app in preview.PROJECT_DIRS:
            mobile = self.root / preview.PROJECT_DIRS[app]
            mobile.mkdir(parents=True)
            (mobile / "settings.gradle.kts").write_text("// mock Gradle project\n")
            (mobile / "Screen.kt").write_text("@Preview @Composable fun ScreenPreview() = ProductionScreen()\n")
        self.cli = self.root / "tools with spaces" / "android"
        self.cli.parent.mkdir()
        self.cli.write_text(
            f"#!{sys.executable}\n" + f"ROOT = {str(self.root)!r}\n" + f"PNG = {PNG!r}\n"
            + f"TREE = {TREE!r}\n" + f"TEXT_TREE = {TEXT_TREE!r}\n" + '''
import json
import os
import signal
import subprocess
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
    if mode in ("timeout-child", "exit-child", "ok-child", "interrupt-child"):
        (root / "render-pid").write_text(str(os.getpid()))
        child = subprocess.Popen([sys.executable, "-c", ''' + repr('''
import os
import signal
import sys
import time
from pathlib import Path
root = Path(sys.argv[1])
def terminate(*_):
    (root / "child-stopped").write_text("terminated")
    sys.exit(0)
signal.signal(signal.SIGTERM, terminate)
(root / "child-pid").write_text(str(os.getpid()))
while True:
    time.sleep(0.02)
''') + ''', ROOT], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        deadline = time.monotonic() + 2
        while not (root / "child-pid").exists() and child.poll() is None and time.monotonic() < deadline:
            time.sleep(0.01)
        if not (root / "child-pid").exists():
            child.kill()
            child.wait(timeout=2)
            sys.exit(8)
        if mode in ("timeout-child", "interrupt-child"):
            print("partial render output", flush=True)
            time.sleep(10)
        if mode == "exit-child":
            sys.exit(7)
    if mode == "timeout":
        print("partial render output", flush=True)
        time.sleep(5)
    if mode == "exit":
        print("preview compilation failed", file=sys.stderr)
        sys.exit(7)
    image = Path(next(x.split("=", 1)[1] for x in args if x.startswith("--output-image-file=")))
    if mode == "symlink-png":
        image.symlink_to(root / "outside.png")
    elif mode == "fifo-png":
        os.mkfifo(image)
    elif mode == "directory-png":
        image.mkdir()
    elif mode == "oversize-png":
        with image.open("wb") as output:
            output.write(PNG)
            output.truncate(4097)
    elif mode != "missing-png":
        image.write_bytes(b"broken image" if mode == "bad-png" else PNG)
    if mode in ("symlink-semantics", "hardlink-semantics", "symlink-metadata",
                "text-symlink-semantics", "text-hardlink-semantics"):
        name = "capture.json" if mode == "symlink-metadata" else "semantics.txt" if mode.startswith("text-") else "semantics.json"
        if mode in ("hardlink-semantics", "text-hardlink-semantics"):
            os.link(root / "outside.txt", image.parent / name)
        else:
            (image.parent / name).symlink_to(root / "outside.txt")
    if mode in ("stdout-limit", "stderr-limit"):
        stream = sys.stdout if mode == "stdout-limit" else sys.stderr
        stream.buffer.write(b"x" * 4097)
        stream.flush()
    if mode == "source-change":
        Path(args[-2]).write_text("// changed during capture\\n")
    if mode.startswith("text-"):
        sys.stdout.write(TEXT_TREE)
        print("\\nWrote rendering to " + str(image))
    elif mode == "bad-semantics":
        print("no tree")
    elif mode == "ambiguous":
        print(json.dumps({"log": "json log"}))
        print(json.dumps(TREE))
    else:
        print("render complete")
        print(json.dumps(TREE, ensure_ascii=False, indent=2))
''', encoding="utf-8",
        )
        self.cli.chmod(0o755)

    def args(self, app="cantos-theatre", *extra):
        return preview.parser().parse_args([
            "--app", app, "--file", preview.PROJECT_DIRS[app] + "/Screen.kt",
            "--composable", "ScreenPreview", "--android-cli", str(self.cli), *extra,
        ])

    def capture(self, args):
        with contextlib.redirect_stdout(io.StringIO()):
            return preview.capture(args, self.root)

    def bundles(self, app="cantos-theatre"):
        return sorted((self.root / "artifacts/ui-evidence/compose-preview" / app).glob("*"))

    def calls(self):
        path = self.root / "calls.jsonl"
        return [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []

    def process_alive(self, pid):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        # A zombie cannot continue producing output; Linux init may reap it later.
        facts = Path(f"/proc/{pid}/stat")
        try:
            return facts.read_text().rsplit(")", 1)[1].split()[0] != "Z"
        except FileNotFoundError:
            return os.name != "posix" or not Path("/proc").exists()

    def assert_test_child_exited(self):
        pid = int((self.root / "child-pid").read_text())
        deadline = time.monotonic() + 2
        while self.process_alive(pid) and time.monotonic() < deadline:
            time.sleep(0.01)
        self.assertFalse(self.process_alive(pid), "CLI descendant survived capture")

    def stop_test_processes(self):
        for name in ("render-pid", "child-pid"):
            pid_path = self.root / name
            if not pid_path.exists():
                continue
            pid = int(pid_path.read_text())
            try:
                if self.process_alive(pid):
                    if os.getpgid(pid) == pid:
                        os.killpg(pid, signal.SIGKILL)
                    else:
                        os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass

    def test_capture_keeps_real_command_selection_and_opaque_tree_for_each_project(self):
        for app in preview.PROJECT_DIRS:
            with self.subTest(app=app):
                self.assertEqual(self.capture(self.args(app, "--pid", "123", "--label", "dark vi-VN large")), 0)
                bundle, = self.bundles(app)
                metadata = json.loads((bundle / "capture.json").read_text())
                self.assertEqual(metadata["status"], "PASS")
                self.assertEqual(metadata["dimensions"], {"width_px": 1, "height_px": 1})
                self.assertFalse(metadata["image_inspected"])
                self.assertFalse(metadata["semantics_asserted"])
                self.assertEqual(metadata["image_sha256"], preview.sha256(bundle / "preview.png"))
                self.assertEqual(metadata["semantics_format"], "json")
                self.assertEqual(metadata["semantics_path"], "semantics.json")
                self.assertEqual(json.loads((bundle / "semantics.json").read_text()), TREE)
                self.assertIn("--pid=123", metadata["command"])
                self.assertIn(f"--project={self.root / preview.PROJECT_DIRS[app]}", metadata["command"])
                self.assertEqual(metadata["command"][-1], "ScreenPreview")
                self.assertEqual((bundle / "preview.png").read_bytes(), PNG)
        self.assertEqual(len(self.calls()), 4 * len(preview.PROJECT_DIRS))

    def test_capture_preserves_real_cli_text_tree_bytes_without_inventing_json(self):
        (self.root / "mode.txt").write_text("text-tree")
        self.assertEqual(self.capture(self.args("ui-system-cmp")), 0)
        bundle, = self.bundles("ui-system-cmp")
        metadata = json.loads((bundle / "capture.json").read_text())
        self.assertEqual(metadata["status"], "PASS")
        self.assertEqual(metadata["semantics_format"], "text-tree")
        self.assertEqual(metadata["semantics_path"], "semantics.txt")
        self.assertEqual((bundle / "semantics.txt").read_bytes(), TEXT_TREE.encode("utf-8"))
        self.assertEqual(metadata["semantics_sha256"], preview.sha256(bundle / "semantics.txt"))
        self.assertFalse((bundle / "semantics.json").exists())
        self.assertEqual((bundle / "render.stdout.log").read_bytes(),
                         (TEXT_TREE + f"\nWrote rendering to {bundle / 'preview.png'}\n").encode("utf-8"))
        self.assertFalse(metadata["semantics_asserted"])

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
            self.capture(self.args("cantos-theatre", "--timeout-seconds", "1"))
        self.assertEqual(failure.exception.code, 1)
        self.assertIn("timed out", str(failure.exception))
        bundle, = self.bundles()
        self.assertIn("partial render output", (bundle / "render.stdout.log").read_text())
        self.assertEqual(json.loads((bundle / "capture.json").read_text())["status"], "FAIL")

    @unittest.skipUnless(os.name == "posix", "owned process groups require POSIX")
    def test_cli_children_are_stopped_after_timeout_error_and_normal_exit(self):
        for mode in ("timeout-child", "exit-child", "ok-child"):
            with self.subTest(mode=mode):
                (self.root / "mode.txt").write_text(mode)
                before = set(self.bundles())
                try:
                    if mode == "ok-child":
                        self.assertEqual(self.capture(self.args()), 0)
                    else:
                        with self.assertRaises(preview.PreviewError):
                            self.capture(self.args("cantos-theatre", "--timeout-seconds", "1"))
                    self.assertTrue((self.root / "child-stopped").exists(), "CLI descendant survived capture")
                    self.assert_test_child_exited()
                    bundle, = set(self.bundles()) - before
                    if mode == "timeout-child":
                        self.assertIn("partial render output", (bundle / "render.stdout.log").read_text())
                finally:
                    self.stop_test_processes()
                    for name in ("render-pid", "child-pid", "child-stopped"):
                        (self.root / name).unlink(missing_ok=True)

    @unittest.skipUnless(os.name == "posix", "POSIX group cleanup")
    def test_transient_group_denial_waits_for_exit_and_preserves_timeout(self):
        (self.root / "mode.txt").write_text("timeout-child")
        original_signal = os.killpg
        denied = []
        def exiting_group(group, signum):
            leader = self.root / "render-pid"
            if signum == 0 and leader.exists() and group == int(leader.read_text()) and len(denied) < 2:
                denied.append(group)
                raise PermissionError("test-owned exiting group is not yet reaped")
            return original_signal(group, signum)
        try:
            with mock.patch.object(os, "killpg", side_effect=exiting_group):
                with self.assertRaises(preview.PreviewError) as failure:
                    self.capture(self.args("cantos-theatre", "--timeout-seconds", "1"))
            self.assertIn("timed out", str(failure.exception))
            self.assertEqual(len(denied), 2)
            self.assertEqual(denied[0], denied[1])
            self.assertTrue((self.root / "child-stopped").exists())
            self.assert_test_child_exited()
            bundle, = self.bundles()
            self.assertIn("partial render output", (bundle / "render.stdout.log").read_text())
            self.assertEqual(json.loads((bundle / "capture.json").read_text())["status"], "FAIL")
        finally:
            self.stop_test_processes()

    @unittest.skipUnless(os.name == "posix", "POSIX signals and process groups")
    def test_signals_stop_cli_children_and_record_failure(self):
        (self.root / "mode.txt").write_text("interrupt-child")
        # Invoke an unchanged copy of the production script so ROOT points at the fixture.
        adapter = self.root / "scripts/render_compose_preview.py"
        adapter.parent.mkdir()
        shutil.copyfile(preview.__file__, adapter)
        arguments = [
            "--app", "cantos-theatre", "--file", preview.PROJECT_DIRS["cantos-theatre"] + "/Screen.kt",
            "--composable", "ScreenPreview", "--android-cli", str(self.cli),
        ]
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            with self.subTest(signum=signum):
                before = set(self.bundles())
                runner = subprocess.Popen(
                    [sys.executable, str(adapter), *arguments], stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE, start_new_session=True,
                )
                try:
                    deadline = time.monotonic() + 5
                    while not (self.root / "child-pid").exists() and runner.poll() is None and time.monotonic() < deadline:
                        time.sleep(0.01)
                    self.assertTrue((self.root / "child-pid").exists(), "test CLI never started its child")
                    os.kill(runner.pid, signum)
                    runner.communicate(timeout=5)
                    self.assertEqual(runner.returncode, 128 + signum)
                    self.assertTrue((self.root / "child-stopped").exists(), "CLI descendant survived interruption")
                    self.assert_test_child_exited()
                    bundle, = set(self.bundles()) - before
                    self.assertEqual(json.loads((bundle / "capture.json").read_text())["status"], "FAIL")
                finally:
                    if runner.poll() is None:
                        os.killpg(runner.pid, signal.SIGKILL)
                        runner.communicate(timeout=5)
                    self.stop_test_processes()
                    for name in ("render-pid", "child-pid", "child-stopped"):
                        (self.root / name).unlink(missing_ok=True)

    def test_command_output_is_bounded_and_oversize_never_passes(self):
        for mode, stream in (("stdout-limit", "stdout"), ("stderr-limit", "stderr")):
            with self.subTest(mode=mode), mock.patch.object(preview, "MAX_COMMAND_LOG_BYTES", 4096, create=True):
                (self.root / "mode.txt").write_text(mode)
                before = set(self.bundles())
                with self.assertRaises(preview.PreviewError) as failure:
                    self.capture(self.args())
                self.assertIn(f"{stream} exceeded", str(failure.exception))
                bundle, = set(self.bundles()) - before
                self.assertLessEqual((bundle / f"render.{stream}.log").stat().st_size, 4096)
                self.assertEqual(json.loads((bundle / "capture.json").read_text())["status"], "FAIL")

    @unittest.skipUnless(os.name == "posix", "POSIX group cleanup")
    def test_git_cleanup_failure_blocks_capture_instead_of_becoming_unknown_provenance(self):
        original_signal = os.killpg
        denied_group = None
        def deny_first_group(group, signum):
            nonlocal denied_group
            if denied_group is None:
                denied_group = group
            if group == denied_group:
                raise PermissionError("test-owned group signal denied")
            return original_signal(group, signum)
        with mock.patch.object(os, "killpg", side_effect=deny_first_group):
            with self.assertRaises(preview.PreviewError) as failure:
                self.capture(self.args())
        self.assertIn("could not clean up owned command group", str(failure.exception))
        self.assertIn("test-owned group signal denied", str(failure.exception))
        self.assertEqual(self.calls(), [])
        bundle, = self.bundles()
        self.assertEqual(json.loads((bundle / "capture.json").read_text())["status"], "FAIL")
        self.assertTrue((bundle / "git-status.stderr.log").is_file())

    @unittest.skipUnless(os.name == "posix", "POSIX filesystem types")
    def test_png_must_be_bounded_regular_file_without_symlinks(self):
        (self.root / "outside.png").write_bytes(PNG)
        original_read = Path.read_bytes
        def read_without_blocking_fifo(path):
            if stat.S_ISFIFO(path.lstat().st_mode):
                self.fail("adapter tried to read a FIFO instead of rejecting it")
            return original_read(path)
        for mode in ("symlink-png", "fifo-png", "directory-png", "oversize-png"):
            with self.subTest(mode=mode), mock.patch.object(preview, "MAX_PNG_BYTES", 4096, create=True), \
                    mock.patch.object(Path, "read_bytes", read_without_blocking_fifo):
                (self.root / "mode.txt").write_text(mode)
                before = set(self.bundles())
                with self.assertRaises(preview.PreviewError) as failure:
                    self.capture(self.args())
                self.assertRegex(str(failure.exception), "regular file|symlink|byte limit")
                bundle, = set(self.bundles()) - before
                self.assertEqual(json.loads((bundle / "capture.json").read_text())["status"], "FAIL")
        self.assertEqual((self.root / "outside.png").read_bytes(), PNG)

    @unittest.skipUnless(os.name == "posix", "POSIX link support")
    def test_adapter_writes_never_follow_cli_created_symlinks_or_hardlinks(self):
        sentinel = self.root / "outside.txt"
        sentinel.write_text("private sentinel")
        for mode in ("symlink-semantics", "hardlink-semantics", "symlink-metadata"):
            with self.subTest(mode=mode):
                (self.root / "mode.txt").write_text(mode)
                with self.assertRaises(preview.PreviewError):
                    self.capture(self.args())
                self.assertEqual(sentinel.read_text(), "private sentinel")

    @unittest.skipUnless(os.name == "posix", "POSIX link support")
    def test_text_tree_artifact_writes_never_follow_renderer_links(self):
        sentinel = self.root / "outside.txt"
        sentinel.write_text("private sentinel")
        for mode in ("text-symlink-semantics", "text-hardlink-semantics"):
            with self.subTest(mode=mode):
                (self.root / "mode.txt").write_text(mode)
                with self.assertRaisesRegex(preview.PreviewError, "private artifact semantics.txt"):
                    self.capture(self.args())
                self.assertEqual(sentinel.read_text(), "private sentinel")

    @unittest.skipUnless(os.name == "posix", "POSIX directory permissions")
    def test_capture_bundle_is_private_even_with_permissive_umask(self):
        previous = os.umask(0)
        try:
            self.capture(self.args())
        finally:
            os.umask(previous)
        bundle, = self.bundles()
        self.assertEqual(stat.S_IMODE(bundle.stat().st_mode), 0o700)
        for artifact in bundle.iterdir():
            self.assertEqual(stat.S_IMODE(artifact.stat().st_mode) & 0o077, 0)

    def test_dry_run_neither_probes_cli_nor_writes_evidence(self):
        args = self.args("cantos-theatre", "--dry-run")
        args.android_cli = "cantos-no-such-cli"
        self.assertEqual(self.capture(args), 0)
        self.assertEqual(self.calls(), [])
        self.assertFalse((self.root / "artifacts").exists())

    def test_cmp_target_selects_only_its_isolated_android_preview_project(self):
        self.assertEqual(self.capture(self.args("ui-system-cmp")), 0)
        bundle, = self.bundles("ui-system-cmp")
        metadata = json.loads((bundle / "capture.json").read_text())
        project = self.root / "packages/ui-system/cmp/android-preview"
        self.assertEqual(metadata["project"], str(project))
        self.assertIn(f"--project={project}", metadata["command"])
        self.assertEqual(metadata["source"], "packages/ui-system/cmp/android-preview/Screen.kt")
        self.assertEqual(json.loads((bundle / "semantics.json").read_text()), TREE)

    def test_studio_is_not_a_preview_target(self):
        with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as failure:
            preview.parser().parse_args([
                "--app", "cantos-studio", "--file", "Screen.kt", "--composable", "ScreenPreview",
            ])
        self.assertEqual(failure.exception.code, 2)
        self.assertEqual(self.calls(), [])

    def test_cmp_target_refuses_parent_package_and_product_sources_before_cli_calls(self):
        parent = self.root / "packages/ui-system/cmp/Outside.kt"
        parent.write_text("@Preview @Composable fun ScreenPreview() {}\n")
        for path in (
            "packages/ui-system/cmp/Outside.kt",
            "packages/ui-system/cmp/android-preview/../Outside.kt",
            "apps/cantos-theatre/mobile/Screen.kt",
        ):
            with self.subTest(path=path):
                args = self.args("ui-system-cmp")
                args.file = Path(path)
                with self.assertRaises(preview.PreviewError) as failure:
                    self.capture(args)
                self.assertEqual(failure.exception.code, 2)
        self.assertEqual(self.calls(), [])

    def test_missing_project_and_cli_are_unsupported(self):
        args = self.args()
        (self.root / preview.PROJECT_DIRS["cantos-theatre"] / "settings.gradle.kts").unlink()
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(args)
        self.assertEqual(failure.exception.code, preview.UNSUPPORTED)
        self.assertEqual(self.calls(), [])
        (self.root / preview.PROJECT_DIRS["cantos-theatre"] / "settings.gradle").write_text("// project\n")
        args.android_cli = "cantos-no-such-cli"
        with self.assertRaises(preview.PreviewError) as failure:
            self.capture(args)
        self.assertEqual(failure.exception.code, preview.UNSUPPORTED)
        self.assertEqual(self.calls(), [])

    def test_cross_product_path_traversal_and_option_like_names_are_usage_errors(self):
        for source, composable in (
            ("packages/ui-system/cmp/android-preview/Screen.kt", "ScreenPreview"),
            ("../Screen.kt", "ScreenPreview"),
            ("apps/cantos-theatre/mobile/Screen.kt", "--option"),
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
        mobile = self.root / preview.PROJECT_DIRS["cantos-theatre"]
        source = mobile / "Screen.kt"
        source.unlink()
        source.symlink_to(self.root / preview.PROJECT_DIRS["ui-system-cmp"] / "Screen.kt")
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

    @unittest.skipUnless(os.name == "posix", "symlink support")
    def test_symlinked_mobile_root_cannot_capture_the_shared_preview(self):
        mobile = self.root / preview.PROJECT_DIRS["cantos-theatre"]
        for path in mobile.iterdir():
            path.unlink()
        mobile.rmdir()
        mobile.symlink_to(self.root / preview.PROJECT_DIRS["ui-system-cmp"], target_is_directory=True)
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

    def test_semantics_output_rejects_garbage_malformed_or_ambiguous_text_trees(self):
        def rendered(tree):
            return tree + "\nWrote rendering to /tmp/preview.png\n"
        for output in (
            "", "render complete\n", "Wrote rendering to /tmp/preview.png\n",
            rendered("TextView (left: 0, top: 0, right: 1, bottom: 1) text=\"hello\"\n"),
            rendered(TEXT_TREE.splitlines(keepends=True)[0]),
            rendered(TEXT_TREE.replace("right: 977", "right: -1", 1)),
            rendered(TEXT_TREE.replace("  ComposeView", "    ComposeView", 1)),
            rendered(TEXT_TREE.replace("text=\"1,25\"", "text=\"1,25\"\x00", 1)),
            rendered(TEXT_TREE.replace("    View (", "    arbitrary log\n    View (", 1)),
            rendered("arbitrary log\n" + TEXT_TREE),
            rendered(TEXT_TREE + "arbitrary log\n"),
            rendered(TEXT_TREE + "\n" + TEXT_TREE),
            rendered(TEXT_TREE + "\n{}\n"),
            rendered("{}\n" + TEXT_TREE),
            TEXT_TREE + "\nWrote rendering to /tmp/other.png\n",
            TEXT_TREE + "\nWrote rendering to /tmp/preview.png\nWrote rendering to /tmp/preview.png\n",
        ):
            with self.subTest(output=output):
                with self.assertRaises(preview.PreviewError):
                    preview.semantics_output(output, Path("/tmp/preview.png"))

    def test_semantics_output_keeps_text_line_endings_and_legacy_json(self):
        for ending in ("\n", "\r\n"):
            tree = TEXT_TREE.replace("\n", ending)
            self.assertEqual(preview.semantics_output(tree + ending + "Wrote rendering to /tmp/preview.png" + ending,
                                                     Path("/tmp/preview.png")),
                             ("text-tree", "semantics.txt", tree))
        self.assertEqual(preview.semantics_output("render complete\n" + json.dumps(TREE), Path("/tmp/preview.png")),
                         ("json", "semantics.json", json.dumps(TREE, ensure_ascii=False, separators=(",", ":")) + "\n"))


if __name__ == "__main__":
    unittest.main()
