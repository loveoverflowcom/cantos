"""Retained media-host safety; process doubles are not PostgreSQL or audio evidence."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
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
