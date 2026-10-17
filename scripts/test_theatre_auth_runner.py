"""Deterministic safety regressions; these tests start no emulator, database or browser."""

import contextlib
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

import test_theatre_auth as runner


class TheatreAuthRunnerTests(unittest.TestCase):
    def test_inherited_cloud_database_proxy_and_loader_inputs_are_removed(self):
        inherited = {"PATH": "/tools", "HOME": "/synthetic-home", "LANG": "C.UTF-8",
                     "DATABASE_URL": "private-production", "PGSERVICE": "private-production",
                     "PGHOSTADDR": "203.0.113.1", "HTTP_PROXY": "private-proxy",
                     "https_proxy": "private-proxy", "GOOGLE_APPLICATION_CREDENTIALS": "private-key",
                     "FIREBASE_TOKEN": "private-token", "FIREBASE_AUTH_EMULATOR_HOST": "foreign",
                     "CANTOS_TEST_CLUSTER_URL": "private-production", "CANTOS_WEB_ORIGIN": "foreign",
                     "NODE_OPTIONS": "private-loader", "LD_PRELOAD": "private-loader",
                     "PYTHONPATH": "private-loader", "RUSTFLAGS": "private-config"}
        clean = runner.isolated_environment(inherited)
        self.assertEqual(clean["PATH"], "/tools")
        self.assertEqual(clean["CARGO_HOME"], str(Path("/synthetic-home") / ".cargo"))
        self.assertEqual(clean["RUSTUP_HOME"], str(Path("/synthetic-home") / ".rustup"))
        self.assertEqual(clean["HOME"], "/nonexistent/cantos-no-user-home")
        self.assertEqual(clean["CARGO_NET_OFFLINE"], "true")
        self.assertEqual(clean["RUSTUP_AUTO_INSTALL"], "0")
        for key in inherited.keys() - {"PATH", "HOME", "LANG"}:
            self.assertNotIn(key, clean)

    def test_emulator_module_requires_exact_reviewed_version_without_a_subprocess(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            module = root / "lib/emulator/auth/server.js"
            module.parent.mkdir(parents=True)
            module.write_text("fixture")
            package = root / "package.json"
            with patch.object(runner, "captured") as process:
                for version in ("15.34.0", "14.2.3"):
                    package.write_text(json.dumps({"name": "firebase-tools", "version": version}))
                    with self.assertRaises(runner.Unsupported):
                        runner.emulator_module({"CANTOS_TEST_FIREBASE_AUTH_MODULE": str(module)})
                package.write_text(json.dumps({"name": "firebase-tools", "version": "14.2.2"}))
                self.assertEqual(runner.emulator_module({"CANTOS_TEST_FIREBASE_AUTH_MODULE": str(module)}),
                                 (str(module.resolve()), "14.2.2"))
                process.assert_not_called()

    def test_zero_selected_skipped_and_multiple_test_summaries_cannot_pass(self):
        for output in ("", "test result: ok. 0 passed; 0 failed; 1 ignored;",
                       "test result: ok. 1 passed; 1 failed;",
                       "test result: ok. 1 passed; 0 failed;\ntest result: ok. 0 passed; 0 failed;"):
            with self.subTest(output=output), self.assertRaises(RuntimeError):
                runner.passing_test_count(output)
        self.assertEqual(runner.passing_test_count("test result: ok. 1 passed; 0 failed; 0 ignored;"), 1)

    def test_unimplemented_real_web_journey_is_unsupported_before_tool_or_resource_use(self):
        with patch.object(runner, "captured") as process, \
                patch.object(runner.tempfile, "TemporaryDirectory") as directory:
            with self.assertRaisesRegex(runner.Unsupported, "real disposable Kanidm"):
                runner.prerequisites("firebase-web-e2e", {}, {})
        process.assert_not_called()
        directory.assert_not_called()

    def test_unsupported_host_emits_an_envelope_before_tool_or_resource_use(self):
        output = io.StringIO()
        with patch.object(runner.sys, "platform", "darwin"), \
                patch.object(runner, "captured") as process, \
                patch.object(runner.tempfile, "TemporaryDirectory") as directory, \
                contextlib.redirect_stdout(output):
            self.assertEqual(runner.main(["--suite", "firebase-emulator"]), 3)
        self.assertEqual(json.loads(output.getvalue())["status"], "UNSUPPORTED")
        process.assert_not_called()
        directory.assert_not_called()

    def test_guard_rejects_an_unisolated_host_before_any_network_access(self):
        with patch.object(runner.os, "getpid", return_value=123), \
                patch.object(runner.socket, "socket") as network, \
                patch.object(runner, "checked") as process:
            with self.assertRaisesRegex(RuntimeError, "owned PID namespace"):
                runner.assert_network_guard({})
        network.assert_not_called()
        process.assert_not_called()

    def test_unsupported_summary_retains_source_and_never_prints_private_inputs(self):
        output = io.StringIO()
        with patch.object(runner.sys, "platform", "linux"), \
                patch.object(runner.os, "waitid", create=True), \
                patch.object(runner.os, "WNOWAIT", 0, create=True), \
                patch.object(runner.signal, "SIGHUP", 1, create=True), \
                patch.object(runner, "source_identity", return_value={"sha": "a" * 40, "dirty": True}), \
                patch.object(runner, "prerequisites", side_effect=runner.Unsupported("missing reviewed module")), \
                patch.object(runner.signal, "signal"), \
                patch.dict(os.environ, {"FIREBASE_TOKEN": "do-not-print-this"}), \
                contextlib.redirect_stdout(output):
            self.assertEqual(runner.main(["--suite", "firebase-emulator"]), 3)
        summary = json.loads(output.getvalue())
        self.assertEqual(summary["source"], {"sha": "a" * 40, "dirty": True})
        self.assertEqual(summary["status"], "UNSUPPORTED")
        self.assertEqual(summary["tests_passed"], 0)
        self.assertNotIn("do-not-print-this", output.getvalue())

    def namespace_environment(self):
        if sys.platform != "linux" or not all(shutil.which(tool) for tool in ("unshare", "ip")):
            self.skipTest("owned namespace guard requires Linux unshare and ip")
        environment = runner.isolated_environment(os.environ)
        probe = runner.captured(runner.NAMESPACE + ["ip", "link", "set", "lo", "up"], environment)
        if probe.returncode:
            self.skipTest("host refuses unprivileged user/network/PID namespace fixture")
        environment["CANTOS_TEST_PARENT_NETNS"] = os.readlink("/proc/self/ns/net")
        return environment

    def test_real_kernel_guard_denies_nonlocal_destinations_and_allows_owned_loopback(self):
        environment = self.namespace_environment()
        script = ("import sys,socket;sys.path.insert(0,sys.argv[1]);import test_theatre_auth as r;"
                  "r.assert_network_guard(dict(__import__('os').environ));"
                  "server=socket.socket();server.bind(('127.0.0.1',0));server.listen();"
                  "client=socket.create_connection(server.getsockname(),timeout=1);"
                  "accepted,_=server.accept();accepted.close();client.close();server.close();"
                  "print('owned-loopback-and-forbidden-destinations-PASS')")
        result = runner.captured(runner.NAMESPACE + [sys.executable, "-I", "-c", script,
                                 str(Path(runner.__file__).parent)], environment)
        self.assertEqual(result.returncode, 0, "owned namespace guard must run successfully")
        self.assertEqual(result.stdout, b"owned-loopback-and-forbidden-destinations-PASS\n")

    def test_namespace_timeout_kills_a_detached_grandchild_before_owned_resources_are_deleted(self):
        environment = self.namespace_environment()
        import fcntl
        with tempfile.TemporaryDirectory(prefix="cantos-namespace-regression-") as temporary:
            lock_path = Path(temporary) / "owned.lock"
            marker = Path(temporary) / "ready"
            script = ("import fcntl,os,pathlib,sys,time\n"
                      "child=os.fork()\n"
                      "if child==0:\n"
                      " os.setsid()\n"
                      " lock=open(sys.argv[1],'w')\n"
                      " fcntl.flock(lock,fcntl.LOCK_EX)\n"
                      " pathlib.Path(sys.argv[2]).write_text('detached-and-locked')\n"
                      " time.sleep(60)\n"
                      "else:\n time.sleep(60)\n")
            with self.assertRaises(subprocess.TimeoutExpired):
                runner.captured(runner.NAMESPACE + [sys.executable, "-I", "-c", script,
                                str(lock_path), str(marker)], environment, timeout=1)
            self.assertEqual(marker.read_text(), "detached-and-locked")
            with lock_path.open("r+") as lock:
                # A still-running detached descendant would retain this lock. No global
                # PID lookup or process-name kill is involved in the oracle or cleanup.
                deadline = time.monotonic() + 2
                while True:
                    try:
                        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                        break
                    except BlockingIOError:
                        if time.monotonic() >= deadline:
                            self.fail("detached owned descendant retained its lock after namespace teardown")
                        time.sleep(0.01)

    def test_capture_output_is_bounded_during_collection_and_suppressed(self):
        if os.name != "posix" or not hasattr(os, "waitid") or not hasattr(os, "WNOWAIT"):
            self.skipTest("bounded local collector requires POSIX waitid")
        with patch.object(runner, "MAX_OUTPUT", 1024):
            with self.assertRaisesRegex(RuntimeError, "output suppressed"):
                runner.captured([os.sys.executable, "-I", "-c", "print('x'*2048)"],
                                runner.isolated_environment(os.environ))


if __name__ == "__main__":
    unittest.main()
