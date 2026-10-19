"""Retained media-host safety; process doubles are not PostgreSQL or audio evidence."""
import contextlib
import http.client
import http.server
import io
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import tempfile
import threading
import unittest
from unittest.mock import patch

import retained_postgres
import serve_studio


class RetainedMediaEnvironmentTest(unittest.TestCase):
    def test_only_the_current_clusters_complete_seed_layout_enables_delivery(self):
        with tempfile.TemporaryDirectory() as directory:
            cluster = Path(directory) / "cluster-1"
            media = cluster / "theatre-seed-owned"
            media.mkdir(parents=True)
            (media / "private").mkdir()
            (media / "publication").mkdir()
            (media / "playback.key").write_bytes(b"k" * 32)
            config = {"cluster": str(cluster)}
            seed = dict(zip(("private_root", "publication_root", "playback_key_file"),
                            (str(media / "private"), str(media / "publication"), str(media / "playback.key"))))
            env = serve_studio.theatre_seed_media(config, seed)
            self.assertEqual(env["CANTOS_PUBLICATION_ROOT"], str(media / "publication"))
            self.assertEqual(env["CANTOS_PLAYBACK_KEY_FILE"], str(media / "playback.key"))
            self.assertEqual(serve_studio.theatre_seed_media(config, {}), {})
            for field in seed:
                partial = {key: value for key, value in seed.items() if key != field}
                with self.assertRaisesRegex(ValueError, "incomplete"):
                    serve_studio.theatre_seed_media(config, partial)
            with self.assertRaisesRegex(ValueError, "retained cluster"):
                serve_studio.theatre_seed_media({"cluster": str(cluster / "other")}, seed)
            (media / "publication").rmdir()
            (media / "publication").symlink_to(media / "private", target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "retained cluster"):
                serve_studio.theatre_seed_media(config, seed)


class LocalMediaOriginTest(unittest.TestCase):
    """The advertised origin must be one a loopback browser can open; no validation is relaxed."""

    def test_default_is_a_tls_port_that_is_not_the_http_api_port(self):
        origin = serve_studio.local_media_origin({})
        self.assertEqual(origin, "https://localhost:8443")
        self.assertNotEqual(origin, "https://localhost:8080")

    def test_override_is_exactly_a_loopback_https_origin(self):
        for accepted in ("https://localhost:9443", "https://127.0.0.1:8443"):
            self.assertEqual(serve_studio.local_media_origin({"CANTOS_LOCAL_MEDIA_ORIGIN": accepted}), accepted)
        for refused in ("http://localhost:8443", "https://localhost", "https://localhost:8080",
                        "https://localhost:8443/", "https://localhost:8443/api", "https://example.test:8443",
                        "https://user@localhost:8443", "https://localhost:99999", "localhost:8443", "",
                        "https://localhost:8443?x=1", "https://LOCALHOST:8443 "):
            with self.subTest(origin=refused), self.assertRaisesRegex(ValueError, "CANTOS_LOCAL_MEDIA_ORIGIN"):
                serve_studio.local_media_origin({"CANTOS_LOCAL_MEDIA_ORIGIN": refused})

    def test_the_seeded_host_advertises_that_origin_and_never_an_inherited_one(self):
        with tempfile.TemporaryDirectory() as directory:
            cluster = Path(directory) / "cluster-1"
            media = cluster / "theatre-seed-owned"
            for name in ("private", "publication"):
                (media / name).mkdir(parents=True)
            (media / "playback.key").write_bytes(b"k" * 32)
            config = {"cluster": str(cluster), "retained": True, "purpose": "retained-disposable-cluster"}
            seed = {"cluster": str(cluster), "port": 1, "database": "cantos_test_" + "a" * 32,
                    "private_root": str(media / "private"), "publication_root": str(media / "publication"),
                    "playback_key_file": str(media / "playback.key")}
            config["port"] = 1
            inherited = {"CANTOS_TEST_CONTEXT": "theatre", "CANTOS_PUBLIC_MEDIA_ORIGIN": "https://elsewhere.test"}
            env = serve_studio.studio_environment(config, inherited, seed)
            self.assertEqual(env["CANTOS_PUBLIC_MEDIA_ORIGIN"], "https://localhost:8443")
            env = serve_studio.studio_environment(
                config, dict(inherited, CANTOS_LOCAL_MEDIA_ORIGIN="https://127.0.0.1:9443"), seed)
            self.assertEqual(env["CANTOS_PUBLIC_MEDIA_ORIGIN"], "https://127.0.0.1:9443")
            with self.assertRaisesRegex(ValueError, "CANTOS_LOCAL_MEDIA_ORIGIN"):
                serve_studio.studio_environment(
                    config, dict(inherited, CANTOS_LOCAL_MEDIA_ORIGIN="https://localhost:8080"), seed)


class TlsFrontTest(unittest.TestCase):
    def test_a_grant_is_never_logged(self):
        self.assertEqual(serve_studio.redact("/api/v1/theatre/media/pg1_abc?x=1"),
                         "/api/v1/theatre/media/<redacted>?x=1")
        self.assertEqual(serve_studio.redact("/api/v1/theatre/works"), "/api/v1/theatre/works")

    def test_missing_certificate_is_a_clear_refusal_without_generating_one_silently(self):
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(serve_studio, "ROOT", Path(directory)), \
                    self.assertRaisesRegex(ValueError, "--generate"):
                serve_studio.tls_front([], {})

    @unittest.skipUnless(shutil.which("openssl"), "openssl is required to mint the test certificate")
    def test_range_and_status_pass_through_tls_and_only_api_paths_are_served(self):
        calls = []

        class Upstream(http.server.BaseHTTPRequestHandler):
            def do_GET(self):
                calls.append((self.path, self.headers.get("Range"), self.headers.get("Host")))
                body = bytes(range(100))
                if self.path.endswith("/gone"):
                    payload = b'{"code":"not_found","retryable":false}'
                    self.send_response(404)
                    self.send_header("Content-Type", "application/json")
                    self.send_header("Content-Length", str(len(payload)))
                    self.end_headers()
                    self.wfile.write(payload)
                    return
                if self.headers.get("Range") == "bytes=10-19":
                    self.send_response(206)
                    self.send_header("Content-Range", "bytes 10-19/100")
                    self.send_header("Content-Length", "10")
                    self.send_header("Accept-Ranges", "bytes")
                    self.end_headers()
                    self.wfile.write(body[10:20])
                else:
                    self.send_response(200)
                    self.send_header("Content-Length", "100")
                    self.end_headers()
                    self.wfile.write(body)

            def log_message(self, *args):
                pass

        upstream = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Upstream)
        threading.Thread(target=upstream.serve_forever, daemon=True).start()
        self.addCleanup(upstream.shutdown)
        self.addCleanup(upstream.server_close)
        with tempfile.TemporaryDirectory() as directory:
            certificate, key = serve_studio.generate_certificate(Path(directory))
            self.assertEqual(oct(key.stat().st_mode & 0o777), "0o600")
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(certificate, key)
            front = serve_studio.TlsFrontServer(("127.0.0.1", 0), context)
            self.addCleanup(front.server_close)
            threading.Thread(target=front.serve_forever, daemon=True).start()
            self.addCleanup(front.shutdown)
            with patch.object(serve_studio.TlsFront, "upstream", ("127.0.0.1", upstream.server_address[1])):
                trust = ssl.create_default_context(cafile=str(certificate))
                port = front.server_address[1]

                def fetch(path, **headers):
                    connection = http.client.HTTPSConnection("localhost", port, context=trust, timeout=10)
                    connection.request("GET", path, headers=headers)
                    response = connection.getresponse()
                    return response.status, dict(response.getheaders()), response.read()

                status, headers, body = fetch("/api/v1/theatre/media/pg1_x", Range="bytes=10-19")
                self.assertEqual((status, body), (206, bytes(range(10, 20))))
                self.assertEqual(headers["Content-Range"], "bytes 10-19/100")
                status, _, body = fetch("/api/v1/theatre/media/pg1_x")
                self.assertEqual((status, len(body)), (200, 100))
                status, _, body = fetch("/api/v1/theatre/media/gone")
                self.assertEqual((status, json.loads(body)["code"]), (404, "not_found"))
                status, _, _ = fetch("/not-api")
                self.assertEqual(status, 404)
                # The upstream sees the original range and a Host of its own, never the front's.
                self.assertEqual(calls[0][1], "bytes=10-19")
                self.assertTrue(all(host.startswith("127.0.0.1:") for _, _, host in calls))
                self.assertEqual(len(calls), 3, "a non-API path is refused before it reaches the API")
            # The certificate names localhost and 127.0.0.1 only: another name does not verify.
            with socket.create_connection(("127.0.0.1", port), timeout=5) as raw, \
                    self.assertRaises(ssl.SSLCertVerificationError):
                trust.wrap_socket(raw, server_hostname="example.test")


class RetainedOwnershipServingTest(unittest.TestCase):
    def fixture(self, directory):
        root = Path(directory)
        evidence = root / "target/revision-evidence"
        cluster = evidence / "schema/cluster-1"
        data = cluster / "pgdata"
        data.mkdir(parents=True)
        (data / "PG_VERSION").write_text("17\n")
        ownership = {"format": 1, "purpose": "retained-disposable-cluster", "scope": "schema",
                     "cluster": str(cluster), "port": 12345, "retained": True,
                     "runner_pid": 123, "runner_identity": "synthetic-start", "runner_active": False,
                     "gate_status": "PASS", "full_gate_status": "NOT_RUN"}
        seed = {"cluster": str(cluster), "port": 12345, "database": "cantos_test_" + "a" * 32,
                "works": [{"fate": "public"}], "rights_valid_until_ms": 123456789}
        self.write_record(root, retained_postgres.OWNERSHIP_RECORD, ownership)
        self.write_record(root, serve_studio.SEED_RECORD, seed)
        return root, ownership, seed, data

    def write_record(self, root, relative, record):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(record))

    def test_ownership_environment_requires_theatre_and_an_exact_seed_without_fixture_urls(self):
        with tempfile.TemporaryDirectory() as directory:
            _, ownership, seed, _ = self.fixture(directory)
            inherited = {"CANTOS_TEST_CONTEXT": "theatre", "DATABASE_URL": "production"}
            environment = serve_studio.studio_environment(ownership, inherited, seed)
            self.assertEqual(environment["DATABASE_URL"],
                             "postgresql://cantos_app@127.0.0.1:12345/" + seed["database"])
            self.assertNotIn("theatre_app_url", ownership)
            self.assertNotIn("CANTOS_AUDIO_ROOT", environment)
            for context in ("editorial", "catalog", "audio"):
                with self.subTest(context=context), self.assertRaisesRegex(ValueError, "matching Theatre seed"):
                    serve_studio.studio_environment(ownership, {"CANTOS_TEST_CONTEXT": context}, seed)
            for other in (None, {}, dict(seed, port=12346), dict(seed, cluster="/other/cluster-1")):
                with self.subTest(seed=other), self.assertRaisesRegex(ValueError, "matching Theatre seed"):
                    serve_studio.studio_environment(ownership, inherited, other)

    def test_only_seeded_theatre_can_select_ownership_without_a_full_run_record(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ownership, seed, _ = self.fixture(directory)
            with patch.object(serve_studio, "ROOT", root), \
                    patch.object(retained_postgres, "runner_alive", return_value=False):
                self.assertEqual(serve_studio.serving_config("theatre", seed), ownership)
                for context, receipt in (("theatre", None), ("editorial", seed), ("catalog", seed),
                                         ("audio", seed), ("unsupported", seed)):
                    with self.subTest(context=context, seed=receipt), self.assertRaises(FileNotFoundError):
                        serve_studio.serving_config(context, receipt)
            self.assertFalse((root / "target/revision-evidence/run.json").exists())

    def test_existing_full_record_still_serves_other_contexts_and_unseeded_theatre(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ownership, seed, _ = self.fixture(directory)
            legacy = {"retained": True, "script": "fixture", "app_url": "editorial-fixture",
                      "theatre_app_url": "theatre-fixture"}
            self.write_record(root, "target/revision-evidence/run.json", legacy)
            with patch.object(serve_studio, "ROOT", root), \
                    patch.object(retained_postgres, "runner_alive", return_value=False):
                self.assertEqual(serve_studio.serving_config("editorial", seed), legacy)
                self.assertEqual(serve_studio.serving_config("theatre", None), legacy)
                self.assertEqual(serve_studio.serving_config("theatre", seed), ownership)

    def test_stale_seed_never_falls_back_to_an_older_full_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root, _, seed, _ = self.fixture(directory)
            legacy = {"retained": True, "cluster": str(root / "target/revision-evidence/cluster-2"),
                      "port": 12346, "theatre_app_url":
                      "postgresql://cantos_app@127.0.0.1:12346/cantos_test_old"}
            self.write_record(root, "target/revision-evidence/run.json", legacy)
            for stale in (dict(seed, port=12346), dict(seed, cluster=legacy["cluster"]), [], {}):
                with self.subTest(seed=stale), patch.object(serve_studio, "ROOT", root), \
                        self.assertRaisesRegex(ValueError, "does not match retained cluster ownership"):
                    serve_studio.serving_config("theatre", stale)
            for malformed in (None, [], False):
                self.write_record(root, serve_studio.SEED_RECORD, malformed)
                with self.subTest(receipt=malformed), patch.object(serve_studio, "ROOT", root), \
                        patch.dict(os.environ, {"CANTOS_TEST_CONTEXT": "theatre"}, clear=True), \
                        patch.object(os, "execve") as execute, \
                        self.assertRaisesRegex(ValueError, "invalid Theatre seed receipt"):
                    serve_studio.main()
                execute.assert_not_called()

    def test_main_checks_the_actual_owned_cluster_before_exec_without_a_full_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root, _, seed, data = self.fixture(directory)
            inherited = {"CANTOS_TEST_CONTEXT": "theatre", "DATABASE_URL": "production",
                         "PGHOST": "foreign.invalid", "PATH": "/synthetic/bin"}
            result = subprocess.CompletedProcess([], 0, str(data) + "\n", "")
            with patch.object(serve_studio, "ROOT", root), \
                    patch.object(retained_postgres, "runner_alive", return_value=False), \
                    patch.dict(os.environ, inherited, clear=True), \
                    patch.object(retained_postgres.subprocess, "run", return_value=result) as query, \
                    patch.object(os, "chdir") as chdir, \
                    patch.object(os, "execve") as execute, \
                    contextlib.redirect_stdout(io.StringIO()) as output:
                serve_studio.main()
            self.assertEqual(query.call_args.args[0][2],
                             "postgresql://cantos_test_admin@127.0.0.1:12345/postgres")
            self.assertEqual(query.call_args.kwargs["env"], {"PATH": "/synthetic/bin"})
            chdir.assert_called_once_with(root)
            self.assertEqual(execute.call_args.args[2]["DATABASE_URL"],
                             "postgresql://cantos_app@127.0.0.1:12345/" + seed["database"])
            self.assertIn("Theatre seed catalog: 1 public works", output.getvalue())
            self.assertNotIn("empty public catalog", output.getvalue())
            self.assertFalse((root / "target/revision-evidence/run.json").exists())

    def test_stopped_or_replaced_cluster_is_refused_before_chdir_or_exec(self):
        with tempfile.TemporaryDirectory() as directory:
            root, _, _, _ = self.fixture(directory)
            results = ((subprocess.CompletedProcess([], 2, "", "stopped"), "stopped or unreachable"),
                       (subprocess.CompletedProcess([], 0, str(root / "other/pgdata") + "\n", ""),
                        "another data directory"))
            for result, error in results:
                with self.subTest(error=error), patch.object(serve_studio, "ROOT", root), \
                        patch.object(retained_postgres, "runner_alive", return_value=False), \
                        patch.dict(os.environ, {"CANTOS_TEST_CONTEXT": "theatre"}, clear=True), \
                        patch.object(retained_postgres.subprocess, "run", return_value=result), \
                        patch.object(os, "chdir") as chdir, patch.object(os, "execve") as execute, \
                        self.assertRaisesRegex(ValueError, error):
                    serve_studio.main()
                chdir.assert_not_called()
                execute.assert_not_called()

    def test_active_or_swapped_ownership_cannot_reach_the_cluster_or_exec(self):
        with tempfile.TemporaryDirectory() as directory:
            root, ownership, _, _ = self.fixture(directory)
            for altered in (dict(ownership, runner_active=True), dict(ownership, scope="full"),
                            dict(ownership, retained=False), dict(ownership, purpose="full-gate-proof")):
                self.write_record(root, retained_postgres.OWNERSHIP_RECORD, altered)
                with self.subTest(ownership=altered), patch.object(serve_studio, "ROOT", root), \
                        patch.object(retained_postgres, "runner_alive", return_value=False), \
                        patch.dict(os.environ, {"CANTOS_TEST_CONTEXT": "theatre"}, clear=True), \
                        patch.object(retained_postgres.subprocess, "run") as query, \
                        patch.object(os, "execve") as execute, self.assertRaises(ValueError):
                    serve_studio.main()
                query.assert_not_called()
                execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
