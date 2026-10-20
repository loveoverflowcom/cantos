"""Deterministic guard regressions, without infrastructure, credentials or production binaries."""

import contextlib
import io
import json
import os
from pathlib import Path
import signal
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import time
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import verify_native_isolation as guard


TEST = "qualification::owned_fixture"
PASS = ("running 1 test\ntest " + TEST + " ... ok\n\n"
        "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s\n").encode()


class PrivateFixture(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="cantos-isolation-regression-", dir="/tmp")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)

    def executable(self, name, code):
        path = self.directory / name
        path.write_text("#!" + sys.executable + "\n" + code, encoding="utf-8")
        path.chmod(0o700)
        return path

    def environment(self, name="run"):
        directory = self.directory / name
        directory.mkdir(mode=0o700)
        return directory, guard.private_environment(directory)


class EnvironmentTests(PrivateFixture):
    def test_private_environment_is_allowlisted_and_owns_private_directories(self):
        with mock.patch.dict(os.environ, {"AWS_SECRET_ACCESS_KEY": "do-not-inherit",
                                         "DATABASE_URL": "do-not-inherit",
                                         "DYLD_INSERT_LIBRARIES": "do-not-inherit"}):
            directory, environment = self.environment()
        self.assertEqual(set(environment), guard.ENVIRONMENT_KEYS)
        self.assertNotIn("do-not-inherit", environment.values())
        self.assertEqual(list(Path(environment["PATH"]).iterdir()), [])
        for key in ("HOME", "TMPDIR", "XDG_CONFIG_HOME", "XDG_CACHE_HOME", "XDG_DATA_HOME"):
            path = Path(environment[key])
            self.assertEqual(path.parent, directory)
            self.assertEqual(path.stat().st_mode & 0o777, 0o700)

    def test_credentials_proxies_compiler_and_application_inputs_are_refused(self):
        _, environment = self.environment()
        for key in ("AWS_SECRET_ACCESS_KEY", "CANTOS_DB_URL", "PGPASSWORD", "HTTP_PROXY",
                    "GOOGLE_APPLICATION_CREDENTIALS", "FIREBASE_TOKEN", "RUSTFLAGS", "DYLD_LIBRARY_PATH"):
            with self.subTest(key=key):
                with self.assertRaisesRegex(guard.GuardFailure, "credential or unapproved"):
                    guard.audit_environment(dict(environment, **{key: "private-do-not-print"}))

    def test_executable_and_credential_fault_controls_actually_run(self):
        directory, environment = self.environment()
        self.assertEqual(guard.negative_controls(environment, directory),
                         {"executable": "PASS", "credential": "PASS", "network": "NOT_RUN"})
        self.assertEqual(list(Path(environment["PATH"]).iterdir()), [])

    def test_versioned_and_wildcard_forbidden_executables_are_detected(self):
        _, environment = self.environment()
        for name in ("podman", "docker-26", "psql17", "postgres-17", "kanidmd", "kanidm-unixd",
                     "firebase", "redis-server", "redis-cli", "feldera"):
            with self.subTest(name=name):
                path = Path(environment["PATH"]) / name
                path.write_text("synthetic canary", encoding="utf-8")
                path.chmod(0o700)
                with self.assertRaisesRegex(guard.GuardFailure, "forbidden infrastructure"):
                    guard.audit_path(environment)
                path.unlink()

    def test_empty_or_populated_path_cannot_pass(self):
        _, environment = self.environment()
        with self.assertRaisesRegex(guard.GuardFailure, "private tool-free"):
            guard.audit_path(dict(environment, PATH=""))
        path = Path(environment["PATH"]) / "benign-tool"
        path.write_text("unexpected file", encoding="utf-8")
        with self.assertRaisesRegex(guard.GuardFailure, "unexpected executable or file"):
            guard.audit_path(environment)


class ParsingTests(unittest.TestCase):
    def test_exact_one_executed_test_is_required(self):
        self.assertEqual(guard.passing_test_count(PASS, TEST), 1)
        for output in (PASS.replace(b"1 passed", b"0 passed"),
                       PASS.replace(TEST.encode(), b"wrong_test"),
                       PASS.replace(b"0 ignored", b"1 ignored"),
                       PASS + PASS, PASS.replace(b" ... ok", b" ... ignored"), b"private\xff"):
            with self.subTest(output_sha=guard.hashlib.sha256(output).hexdigest()):
                with self.assertRaises(guard.GuardFailure):
                    guard.passing_test_count(output, TEST)


@unittest.skipUnless(sys.platform in ("darwin", "linux"), "requires POSIX group supervision")
class CaptureTests(PrivateFixture):
    def test_capture_has_no_raw_output_in_failure(self):
        binary = self.executable("too-much", "print('private-do-not-print' * 1000)\n")
        with self.assertRaisesRegex(guard.GuardFailure, "output bound") as caught:
            guard.captured([str(binary)], {}, self.directory, 2, 100)
        self.assertNotIn("private-do-not-print", str(caught.exception))

    def test_timeout_is_bounded_and_leader_is_reaped(self):
        pid_file = self.directory / "pid"
        command = ["/bin/sh", "-c", "printf '%s' \"$$\" > '" + str(pid_file) + "'; exec /bin/sleep 20"]
        started = time.monotonic()
        with self.assertRaisesRegex(guard.GuardFailure, "time bound"):
            guard.captured(command, {}, self.directory, 0.2)
        self.assertLess(time.monotonic() - started, 3)
        pid = int(pid_file.read_text())
        with self.assertRaises(ProcessLookupError):
            os.kill(pid, 0)

    def test_exited_leader_does_not_leave_inherited_output_child_alive(self):
        marker = self.directory / "survived"
        binary = self.executable("descendant", "import os, time\nfrom pathlib import Path\n"
                                 "if os.fork() == 0:\n"
                                 "    time.sleep(0.7)\n"
                                 "    Path(" + repr(str(marker)) + ").write_text('escaped')\n"
                                 "    os._exit(0)\n"
                                 "print('leader complete')\n")
        completed = guard.captured([str(binary)], {}, self.directory, 2)
        self.assertEqual(completed.returncode, 0)
        time.sleep(0.8)
        self.assertFalse(marker.exists())

    def test_repeated_cancellation_cannot_interrupt_group_reap_or_descriptor_cleanup(self):
        marker = self.directory / "descendant-survived"
        binary = self.executable("double-cancel", "import os, time\nfrom pathlib import Path\n"
                                 "if os.fork() == 0:\n"
                                 "    time.sleep(0.7)\n"
                                 "    Path(" + repr(str(marker)) + ").write_text('escaped')\n"
                                 "    os._exit(0)\n"
                                 "print('leader complete')\n")
        signalled = []
        children = []
        observers = []
        original_signal_group = guard.signal_group
        original_popen = guard.subprocess.Popen
        original_observer = guard.ExitObserver

        def cancel_at_group_boundary(pid, number):
            signalled.append(pid)
            if len(signalled) <= 2:
                os.kill(os.getpid(), signal.SIGTERM)
            original_signal_group(pid, number)

        def retain_child(*args, **kwargs):
            child = original_popen(*args, **kwargs)
            children.append(child)
            return child

        def retain_observer(pid):
            observer = original_observer(pid)
            observers.append(observer)
            return observer

        previous_handler = signal.signal(signal.SIGTERM, guard.interrupted)
        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, set())
        try:
            with mock.patch.object(guard, "signal_group", side_effect=cancel_at_group_boundary), \
                    mock.patch.object(guard.subprocess, "Popen", side_effect=retain_child), \
                    mock.patch.object(guard, "ExitObserver", side_effect=retain_observer):
                with self.assertRaisesRegex(guard.GuardFailure, "interrupted"):
                    guard.captured([str(binary)], {}, self.directory, 2)
            self.assertEqual(len(signalled), 2)
            self.assertEqual(len(set(signalled)), 1)
            self.assertEqual(signal.getsignal(signal.SIGTERM), guard.interrupted)
            self.assertEqual(signal.pthread_sigmask(signal.SIG_BLOCK, set()), previous_mask)
            self.assertEqual(len(children), 1)
            self.assertIsNotNone(children[0].returncode)
            self.assertTrue(children[0].stdout.closed)
            self.assertTrue(children[0].stderr.closed)
            for observer in observers:
                if observer.queue is not None:
                    self.assertTrue(observer.queue.closed)
            time.sleep(0.8)
            self.assertFalse(marker.exists(), "owned descendant must not survive repeated cancellation")
        finally:
            signal.signal(signal.SIGTERM, previous_handler)
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)
            # Keep a failing regression from leaking its exact synthetic group.
            for child in children:
                if child.returncode is None:
                    original_signal_group(child.pid, signal.SIGKILL)
                    child.wait(timeout=3)
                child.stdout.close()
                child.stderr.close()

    def test_private_environment_is_used_for_real_child(self):
        directory, environment = self.environment()
        binary = self.executable("environment", "import json, os\nprint(json.dumps(dict(os.environ)))\n")
        completed = guard.captured([str(binary)], environment, directory, 2)
        observed = json.loads(completed.stdout)
        # CPython can add LC_CTYPE during locale coercion; Rust targets do not.
        observed.pop("LC_CTYPE", None)
        observed.pop("__CF_USER_TEXT_ENCODING", None)
        self.assertEqual(observed, environment)


@unittest.skipUnless(sys.platform in ("darwin", "linux"), "requires POSIX signal masking")
class DirectoryCleanupTests(PrivateFixture):
    def test_repeated_cancellation_during_directory_removal_cannot_leave_owned_files(self):
        binary = self.executable("cleanup-target", "pass\n")
        cancellations = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
        previous_handlers = {number: signal.getsignal(number) for number in cancellations}
        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, set())
        original_rmtree = guard.tempfile.TemporaryDirectory._rmtree
        source = {"sha": "a" * 40, "dirty": False, "diff_sha256": "b" * 64,
                  "guard_sha256": "c" * 64}
        try:
            for cancellation in cancellations:
                for cancelled_in_body in (False, True):
                    with self.subTest(signal=cancellation, cancelled_in_body=cancelled_in_body):
                        owned = []

                        def run_target(_args, directory, result):
                            owned.append(directory)
                            (directory / "nested").mkdir()
                            (directory / "nested" / "fixture").write_text("synthetic owned file")
                            if cancelled_in_body:
                                os.kill(os.getpid(), cancellation)
                            result.update(status="UNSUPPORTED", reason="synthetic target completed")

                        def cancel_at_removal(path, *args, **kwargs):
                            os.kill(os.getpid(), cancellation)
                            os.kill(os.getpid(), cancellation)
                            return original_rmtree(path, *args, **kwargs)

                        try:
                            # The target/source doubles isolate main's real directory lifecycle.
                            # No child, kernel namespace, external network or real data is used.
                            with mock.patch.object(guard.sys, "platform", "darwin"), \
                                    mock.patch.object(guard, "source_identity", return_value=source), \
                                    mock.patch.object(guard, "run_target", side_effect=run_target), \
                                    mock.patch.object(guard.tempfile.TemporaryDirectory, "_rmtree",
                                                      side_effect=cancel_at_removal), \
                                    contextlib.redirect_stdout(io.StringIO()) as output:
                                code = guard.main(["--test-binary", str(binary), "--test", TEST])
                            result = json.loads(output.getvalue())
                            self.assertEqual(code, 1)
                            self.assertEqual(result["status"], "FAIL")
                            self.assertEqual(result["reason"], "owned guard invocation was interrupted")
                            self.assertEqual(len(owned), 1)
                            self.assertFalse(owned[0].exists(), "cancellation left the owned run directory")
                            self.assertEqual(signal.pthread_sigmask(signal.SIG_BLOCK, set()), previous_mask)
                            for number in cancellations:
                                self.assertEqual(signal.getsignal(number), guard.interrupted)
                        finally:
                            # Preserve the failure result, but remove only this test's own fixture
                            # if running the regression against the old leaking implementation.
                            for path in owned:
                                if path.exists():
                                    original_rmtree(str(path))
        finally:
            for number, handler in previous_handlers.items():
                signal.signal(number, handler)
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)


class NetworkTests(unittest.TestCase):
    def test_real_reachable_loopback_connection_is_a_guard_failure(self):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen(1)
            with self.assertRaisesRegex(guard.GuardFailure, "reachable or unclassified"):
                guard.assert_unreachable(listener.getsockname())

    def test_only_kernel_unreachable_errors_are_accepted(self):
        for error in (guard.errno.ENETUNREACH, guard.errno.EHOSTUNREACH):
            with mock.patch.object(socket.socket, "connect_ex", return_value=error):
                guard.assert_unreachable(("192.0.2.1", 443))
        for error in (0, guard.errno.ECONNREFUSED, guard.errno.ETIMEDOUT):
            with mock.patch.object(socket.socket, "connect_ex", return_value=error):
                with self.assertRaises(guard.GuardFailure):
                    guard.assert_unreachable(("192.0.2.1", 443))

    def test_same_namespace_and_non_init_process_are_refused_before_connections(self):
        with mock.patch.object(os, "getpid", return_value=2):
            with self.assertRaisesRegex(guard.GuardFailure, "owned network and PID"):
                guard.network_controls("anything", "/nonexistent", {}, Path("/tmp"))
        with mock.patch.object(os, "getpid", return_value=1), \
                mock.patch.object(os, "readlink", return_value="same"):
            with self.assertRaisesRegex(guard.GuardFailure, "owned network and PID"):
                guard.network_controls("same", "/nonexistent", {}, Path("/tmp"))


@unittest.skipUnless(shutil.which("git"), "requires installed Git for isolated source fixtures")
class SourceTests(PrivateFixture):
    def setUp(self):
        super().setUp()
        self.repository = self.directory / "repository"
        self.repository.mkdir()
        self.git_environment = {"PATH": "/usr/bin:/bin", "HOME": str(self.directory),
                                "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull}
        self.git("init", "--quiet", "--template=")
        (self.repository / "tracked.txt").write_text("baseline\n", encoding="utf-8")
        self.git("add", "tracked.txt")
        self.git("-c", "user.name=Native guard fixture", "-c", "user.email=native-guard@example.invalid",
                 "commit", "--quiet", "--no-gpg-sign", "-m", "synthetic fixture")

    def git(self, *arguments):
        subprocess.run([shutil.which("git"), *arguments], cwd=self.repository,
                       env=self.git_environment, stdin=subprocess.DEVNULL,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)

    def identity(self):
        with mock.patch.object(guard, "ROOT", self.repository):
            return guard.source_identity(self.directory)

    def test_untracked_source_bytes_are_bound_without_exposing_their_contents(self):
        initial = self.identity()
        source = self.repository / "new-source.rs"
        source.write_text("private-fixture-first", encoding="utf-8")
        first = self.identity()
        source.write_text("private-fixture-second", encoding="utf-8")
        second = self.identity()
        self.assertFalse(initial["dirty"])
        self.assertTrue(first["dirty"])
        self.assertTrue(second["dirty"])
        self.assertNotEqual(initial["diff_sha256"], first["diff_sha256"])
        self.assertNotEqual(first["diff_sha256"], second["diff_sha256"])
        self.assertNotIn("private-fixture", json.dumps(second))

    def test_staged_deleted_and_mode_changes_are_part_of_the_raw_snapshot(self):
        initial = self.identity()
        tracked = self.repository / "tracked.txt"
        tracked.write_text("staged source\n", encoding="utf-8")
        unstaged = self.identity()
        self.git("add", "tracked.txt")
        staged = self.identity()
        tracked.chmod(tracked.stat().st_mode | stat.S_IXUSR)
        executable = self.identity()
        tracked.unlink()
        missing = self.identity()
        self.git("rm", "--quiet", "--cached", "tracked.txt")
        deleted = self.identity()
        snapshots = [initial, unstaged, staged, executable, missing, deleted]
        self.assertFalse(initial["dirty"])
        self.assertTrue(all(value["dirty"] for value in snapshots[1:]))
        self.assertEqual(len({value["diff_sha256"] for value in snapshots}), len(snapshots))

    def test_symlink_targets_are_bound_without_reading_the_linked_contents(self):
        outside = self.directory / "private-target"
        outside.write_text("do-not-read-first", encoding="utf-8")
        linked = self.repository / "source-link"
        linked.symlink_to(outside)
        initial = self.identity()
        outside.write_text("do-not-read-second", encoding="utf-8")
        target_content_changed = self.identity()
        self.assertEqual(initial, target_content_changed)
        linked.unlink()
        linked.symlink_to(self.directory / "different-private-target")
        self.assertNotEqual(initial["diff_sha256"], self.identity()["diff_sha256"])

    def test_local_clean_filter_cannot_run_or_hide_changed_raw_source(self):
        attributes = self.repository / ".gitattributes"
        attributes.write_text("tracked.txt filter=constant\n", encoding="utf-8")
        self.git("add", ".gitattributes")
        self.git("-c", "user.name=Native guard fixture", "-c", "user.email=native-guard@example.invalid",
                 "commit", "--quiet", "--no-gpg-sign", "-m", "synthetic attributes")
        marker = self.directory / "clean-filter-invoked"
        driver = self.directory / "constant-clean"
        driver.write_text("#!/bin/sh\nprintf 'invoked' > '" + str(marker) + "'\nprintf 'constant'\n",
                          encoding="utf-8")
        driver.chmod(0o700)
        self.git("config", "filter.constant.clean", str(driver))
        tracked = self.repository / "tracked.txt"
        tracked.write_text("first raw Rust source\n", encoding="utf-8")
        first = self.identity()
        tracked.write_text("second raw Rust source\n", encoding="utf-8")
        second = self.identity()
        self.assertFalse(marker.exists(), "source attribution must never execute a configured clean filter")
        self.assertNotEqual(first["diff_sha256"], second["diff_sha256"])
        self.assertTrue(first["dirty"])
        self.assertTrue(second["dirty"])

    def test_local_external_diff_cannot_run_or_hide_changed_tracked_bytes(self):
        marker = self.directory / "driver-invoked"
        driver = self.directory / "constant-diff"
        driver.write_text("#!/bin/sh\nprintf 'invoked' > '" + str(marker) + "'\nprintf 'constant'\n",
                          encoding="utf-8")
        driver.chmod(0o700)
        self.git("config", "diff.external", str(driver))
        tracked = self.repository / "tracked.txt"
        tracked.write_text("first changed source\n", encoding="utf-8")
        first = self.identity()
        tracked.write_text("second changed source\n", encoding="utf-8")
        second = self.identity()
        self.assertFalse(marker.exists(), "source attribution must never execute a configured diff driver")
        self.assertNotEqual(first["diff_sha256"], second["diff_sha256"])


class MainTests(PrivateFixture):
    def invoke(self, args):
        with contextlib.redirect_stdout(io.StringIO()) as output:
            code = guard.main(args)
        return code, json.loads(output.getvalue())

    def test_relative_directory_symlink_and_nonexecutable_targets_are_refused(self):
        plain = self.directory / "plain"
        plain.write_text("no executable", encoding="utf-8")
        real = self.executable("real", "pass\n")
        alias = self.directory / "alias"
        alias.symlink_to(real)
        for path in ("relative", self.directory, plain, alias):
            with self.subTest(path=str(path)):
                code, result = self.invoke(["--test-binary", str(path), "--test", TEST])
                self.assertEqual(code, 1)
                self.assertEqual(result["status"], "FAIL")
                self.assertEqual(result["reason"], "test binary must be an absolute regular executable file")
                self.assertNotIn(str(self.directory), json.dumps(result))

    @unittest.skipUnless(sys.platform == "darwin", "macOS best-effort path")
    def test_macos_runs_controls_and_exact_test_but_never_reports_full_pass(self):
        binary = self.executable("libtest", "import sys\n"
                                 "assert sys.argv[1:] == ['--exact', " + repr(TEST) + ", '--test-threads=1']\n"
                                 "sys.stdout.buffer.write(" + repr(PASS) + ")\n")
        code, result = self.invoke(["--test-binary", str(binary), "--test", TEST])
        self.assertEqual(code, 3)
        self.assertEqual(result["status"], "UNSUPPORTED")
        self.assertEqual(result["tests_passed"], 1)
        self.assertEqual(result["test_name"], TEST)
        self.assertEqual(result["timeout_seconds"], 60)
        self.assertEqual(result["binary_sha256"], guard.sha256(binary))
        self.assertEqual(result["controls"], {"executable": "PASS", "credential": "PASS", "network": "UNSUPPORTED"})
        self.assertRegex(result["source"]["sha"], r"^[0-9a-f]{40}$")

    @unittest.skipUnless(sys.platform == "darwin", "macOS best-effort path")
    def test_wrong_named_test_and_secret_output_fail_without_leak(self):
        binary = self.executable("wrong", "import sys\nprint('private-do-not-print')\n"
                                 "sys.stdout.buffer.write(" + repr(PASS.replace(TEST.encode(), b"wrong_test")) + ")\n")
        code, result = self.invoke(["--test-binary", str(binary), "--test", TEST])
        self.assertEqual(code, 1)
        self.assertEqual(result["status"], "FAIL")
        self.assertNotIn("private-do-not-print", json.dumps(result))

    @unittest.skipUnless(sys.platform == "darwin", "macOS best-effort path")
    def test_mutated_executable_never_retains_pass(self):
        binary = self.executable("mutate", "import sys\nfrom pathlib import Path\n"
                                 "Path(sys.argv[0]).write_text('modified')\n"
                                 "sys.stdout.buffer.write(" + repr(PASS) + ")\n")
        code, result = self.invoke(["--test-binary", str(binary), "--test", TEST])
        self.assertEqual(code, 1)
        self.assertEqual(result["reason"], "selected test binary changed during execution")


    @unittest.skipUnless(sys.platform == "darwin", "macOS best-effort path")
    def test_source_change_during_execution_never_retains_pass(self):
        binary = self.executable("source-change", "import sys\nsys.stdout.buffer.write(" + repr(PASS) + ")\n")
        first = {"sha": "a" * 40, "dirty": True, "diff_sha256": "a" * 64, "guard_sha256": "c" * 64}
        second = dict(first, diff_sha256="b" * 64)
        with mock.patch.object(guard, "source_identity", side_effect=[first, second]):
            code, result = self.invoke(["--test-binary", str(binary), "--test", TEST])
        self.assertEqual(code, 1)
        self.assertEqual(result["reason"], "source inputs changed during guard execution")


if __name__ == "__main__":
    unittest.main()
