"""The removed historical orchestration has no active safety contract; the refusal does."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch
import test_postgres as runner
import serve_studio

class PostgresRunnerTests(unittest.TestCase):
    def test_retired_entrypoint_refuses_before_any_effect(self):
        with patch.object(Path,"mkdir",side_effect=AssertionError("filesystem effect")), \
             patch.object(subprocess,"Popen",side_effect=AssertionError("child process")), \
             contextlib.redirect_stderr(io.StringIO()) as error:
            with self.assertRaises(SystemExit) as refused:
                runner.main()
            self.assertEqual(refused.exception.code,2)
            self.assertIn("cargo xtask verify turso",error.getvalue())

    def test_real_retired_cli_refuses_even_with_optimized_python_and_inherited_pg_inputs(self):
        arguments = ([], ["--keep"], ["--suite", "schema"],
                     ["--suite", "theatre-firebase", "--keep"], ["--help"])
        for optimized in ([], ["-O"]):
            for argv in arguments:
                with self.subTest(optimized=optimized, argv=argv):
                    result = subprocess.run(
                        [sys.executable, *optimized, str(Path(runner.__file__)), *argv],
                        env={"PATH": "", "DATABASE_URL": "postgresql://private.invalid/live",
                             "CANTOS_TEST_CLUSTER_URL": "private", "PGHOST": "private.invalid",
                             "PYTHONDONTWRITEBYTECODE": "1"},
                        text=True, capture_output=True, timeout=5)
                    self.assertEqual(result.returncode, 2, result.stderr)
                    self.assertEqual(result.stdout, "")
                    self.assertIn("test_postgres.py is retired", result.stderr)
                    self.assertNotIn("private.invalid", result.stderr)
                    self.assertNotIn("Traceback", result.stderr)


class RetainedStudioFixtureTests(unittest.TestCase):
    def test_theatre_main_executes_its_retained_fixture_without_editorial_script_or_token(self):
        root = Path("/synthetic/cantos")
        database = "postgresql://cantos_app@127.0.0.1:12345/cantos_test_theatre"
        config = {"retained": True, "theatre_app_url": database}
        inherited = {"CANTOS_TEST_CONTEXT": "theatre", "DATABASE_URL": "production",
                     "CANTOS_WEB_ORIGIN": "http://127.0.0.1:8082"}
        output = io.StringIO()
        with patch.object(serve_studio, "ROOT", root), \
                patch.object(Path, "read_text", return_value=json.dumps(config)) as read, \
                patch.dict(os.environ, inherited, clear=True), \
                patch.object(os, "chdir") as chdir, \
                patch.object(os, "execve") as execute, \
                patch.object(sys, "stdout", output):
            serve_studio.main()
        read.assert_called_once_with()
        chdir.assert_called_once_with(root)
        binary = str(root / "target/debug/cantos-server")
        execute.assert_called_once_with(binary, [binary], {
            **inherited, "DATABASE_URL": database, "CANTOS_ENV": "development",
            "CANTOS_BIND_ADDRESS": "127.0.0.1:8080"})
        self.assertEqual(output.getvalue(),
                         "Theatre test fixture: empty public catalog; identity and playback remain separate.\n"
                         "Seed synthetic works with `cargo xtask seed theatre`.\n")
        self.assertNotIn("script", output.getvalue())
        self.assertNotIn("alice", output.getvalue())

    def test_editorial_main_preserves_the_existing_script_and_synthetic_token_instructions(self):
        root = Path("/synthetic/cantos")
        database = "postgresql://cantos_app@127.0.0.1:12345/cantos_test_editorial"
        config = {"app_url": database, "script": "synthetic-script"}
        output = io.StringIO()
        with patch.object(serve_studio, "ROOT", root), \
                patch.object(Path, "read_text", return_value=json.dumps(config)), \
                patch.dict(os.environ, {}, clear=True), \
                patch.object(os, "chdir") as chdir, \
                patch.object(os, "execve") as execute, \
                patch.object(sys, "stdout", output):
            serve_studio.main()
        chdir.assert_called_once_with(root)
        binary = str(root / "target/debug/cantos-server")
        execute.assert_called_once_with(binary, [binary], {
            "DATABASE_URL": database, "CANTOS_ENV": "development",
            "CANTOS_BIND_ADDRESS": "127.0.0.1:8080"})
        self.assertEqual(output.getvalue(), "Test script ID: synthetic-script\n"
                         "Sign in with the synthetic alice test token (64 lowercase a characters).\n")

    def test_theatre_main_rejects_unretained_or_foreign_database_before_chdir_or_exec(self):
        for config in ({"retained": False, "theatre_app_url": "unused"},
                       {"retained": True, "theatre_app_url": "postgresql://cantos_app@example.com/cantos_test_wrong"}):
            with self.subTest(config=config), \
                    patch.object(Path, "read_text", return_value=json.dumps(config)), \
                    patch.dict(os.environ, {"CANTOS_TEST_CONTEXT": "theatre"}, clear=True), \
                    patch.object(os, "chdir") as chdir, \
                    patch.object(os, "execve") as execute, \
                    self.assertRaises(ValueError):
                serve_studio.main()
            chdir.assert_not_called()
            execute.assert_not_called()

    def test_theatre_context_uses_only_its_retained_app_role_url_and_needs_no_private_audio_root(self):
        config = {"app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_editorial",
                  "catalog_app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_catalog",
                  "audio_app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio",
                  "theatre_app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio",
                  "retained": True,
                  "audio_root": "/private/tmp/synthetic-owned-audio"}
        inherited = {"DATABASE_URL": "production", "CANTOS_TEST_CONTEXT": "theatre",
                     "CANTOS_WEB_ORIGIN": "http://127.0.0.1:8082"}
        env = serve_studio.studio_environment(config, inherited)
        self.assertEqual(env["DATABASE_URL"], config["theatre_app_url"])
        self.assertEqual(env["CANTOS_WEB_ORIGIN"], inherited["CANTOS_WEB_ORIGIN"])
        self.assertEqual(env["CANTOS_ENV"], "development")
        self.assertNotIn("CANTOS_AUDIO_ROOT", env)
        self.assertEqual(inherited["DATABASE_URL"], "production")
        self.assertEqual(serve_studio.studio_environment(config, {})["DATABASE_URL"], config["app_url"])
        self.assertEqual(serve_studio.studio_environment(config, {"CANTOS_TEST_CONTEXT": "catalog"})["DATABASE_URL"], config["catalog_app_url"])

    def test_theatre_context_serves_only_a_seed_recorded_for_this_retained_cluster(self):
        config = {"retained": True, "cluster": "/repo/target/revision-evidence/cluster-1", "port": 12345,
                  "theatre_app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio"}
        database = "cantos_test_" + "a" * 32
        seed = {"cluster": config["cluster"], "port": 12345, "database": database,
                "app_url": "postgresql://cantos_app@example.com:1/production"}
        inherited = {"CANTOS_TEST_CONTEXT": "theatre", "DATABASE_URL": "production"}
        env = serve_studio.studio_environment(config, inherited, seed)
        # The URL is rebuilt from the retained port and name, never copied from the record.
        self.assertEqual(env["DATABASE_URL"], f"postgresql://cantos_app@127.0.0.1:12345/{database}")
        for stale in (dict(seed, cluster="/repo/target/revision-evidence/cluster-2"), dict(seed, port=1)):
            with self.subTest(stale=stale):
                self.assertEqual(serve_studio.studio_environment(config, inherited, stale)["DATABASE_URL"],
                                 config["theatre_app_url"])
        editorial = dict(config, app_url="postgresql://cantos_app@127.0.0.1:12345/cantos_test_editorial")
        self.assertEqual(serve_studio.studio_environment(editorial, {}, seed)["DATABASE_URL"], editorial["app_url"])
        for unsafe in ("production", 'cantos_test_x";DROP DATABASE postgres;', None, "cantos_test_" + "A" * 32):
            with self.subTest(unsafe=unsafe), self.assertRaisesRegex(ValueError, "seed theatre --reset"):
                serve_studio.studio_environment(config, inherited, dict(seed, database=unsafe))

    def test_theatre_context_refuses_old_records_and_urls_outside_the_disposable_app_role(self):
        inherited = {"CANTOS_TEST_CONTEXT": "theatre"}
        url = "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio"
        for config in ({}, {"retained": True}, {"theatre_app_url": url},
                       {"theatre_app_url": url, "retained": False},
                       {"theatre_app_url": url, "retained": 1}):
            with self.subTest(config=config), self.assertRaisesRegex(ValueError, "cargo xtask verify postgres --keep"):
                serve_studio.studio_environment(config, inherited)
        invalid = [
            "postgresql://cantos_test_admin@127.0.0.1:12345/cantos_test_audio",
            "postgresql://cantos_app@example.com:12345/cantos_test_audio",
            "postgresql://cantos_app:password@127.0.0.1:12345/cantos_test_audio",
            "postgresql://cantos_app@127.0.0.1:12345/production",
            "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio?host=example.com",
            "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio#other",
        ]
        for url in invalid:
            with self.subTest(url=url), self.assertRaisesRegex(ValueError, "expected disposable test database"):
                serve_studio.studio_environment({"theatre_app_url": url, "retained": True}, inherited)

    def test_studio_audio_context_uses_its_retained_private_fixture_without_inherited_database(self):
        config = {"app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_editorial",
                  "catalog_app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_catalog",
                  "audio_app_url": "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio",
                  "audio_root": "/private/tmp/synthetic-owned-audio"}
        env = serve_studio.studio_environment(config, {"DATABASE_URL": "production", "CANTOS_TEST_CONTEXT": "audio"})
        self.assertEqual(env["DATABASE_URL"], config["audio_app_url"])
        self.assertEqual(env["CANTOS_AUDIO_ROOT"], config["audio_root"])
        self.assertEqual(env["CANTOS_ENV"], "development")
        with self.assertRaises(ValueError):
            serve_studio.studio_environment(config, {"CANTOS_TEST_CONTEXT": "production"})
        with self.assertRaises(ValueError):
            serve_studio.studio_environment(dict(config, audio_app_url="postgresql://production@example.com/live"), {"CANTOS_TEST_CONTEXT": "audio"})


if __name__ == "__main__":
    unittest.main()
