"""Offline checks for the Theatre seed primitive: target safety, scoped reset, record handling.

No PostgreSQL, Cargo or network: psql and the seed test are replaced by recorded fakes.
The actual seed runs only in `cargo xtask seed theatre` against a retained disposable cluster.
"""
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import seed_theatre  # noqa: E402
import serve_studio  # noqa: E402

DATABASE = "cantos_test_" + "1" * 32
PREVIOUS = "cantos_test_" + "2" * 32


def seed_result(database=DATABASE):
    return {"format": 1, "database": database, "rights_valid_until_ms": 1, "studio_actor": "alice",
            "works": [{"work_id": "5eed0001-0000-4000-8000-000000000000", "kind": "novel",
                       "title": "Mùa gió chướng", "language": "vi-VN", "fate": "public",
                       "units": [{"unit_id": "5eed0001-0001-4000-8000-000000000000", "title": "Chương một",
                                  "studio_position": 0, "publication_id": "p", "pointer_version": 1,
                                  "withdrawn": False}]},
                      {"work_id": "5eed0005-0000-4000-8000-000000000000", "kind": "podcast",
                       "title": "Tín hiệu đã tắt", "language": "vi-VN", "fate": "withdrawn",
                       "units": [{"unit_id": "5eed0005-0001-4000-8000-000000000000", "title": "Tập duy nhất",
                                  "studio_position": 0, "publication_id": "q", "pointer_version": 2,
                                  "withdrawn": True}]}]}


class Workspace:
    """A synthetic checkout with a retained cluster record; nothing here is a real cluster."""

    def __init__(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.evidence = self.root / "target" / "revision-evidence"
        self.cluster = self.evidence / "cluster-1"
        (self.cluster / "pgdata").mkdir(parents=True)
        (self.cluster / "pgdata" / "PG_VERSION").write_text("17\n")
        self.config = {"retained": True, "cluster": str(self.cluster), "port": 54321,
                       "app_url": "postgresql://cantos_app@127.0.0.1:54321/cantos_test_editorial",
                       "audio_app_url": "postgresql://cantos_app@127.0.0.1:54321/cantos_test_audio",
                       "theatre_app_url": "postgresql://cantos_app@127.0.0.1:54321/cantos_test_audio"}
        self.run_record = self.evidence / "run.json"
        self.run_record.write_text(json.dumps(self.config))
        self.seed_record = self.evidence / "theatre-seed.json"
        self.patches = [patch.object(seed_theatre, "ROOT", self.root),
                        patch.object(seed_theatre, "EVIDENCE", self.evidence),
                        patch.object(seed_theatre, "RUN_RECORD", self.run_record),
                        patch.object(seed_theatre, "SEED_RECORD", self.seed_record)]

    def __enter__(self):
        for active in self.patches:
            active.start()
        return self

    def __exit__(self, *_):
        for active in reversed(self.patches):
            active.stop()
        self.directory.cleanup()


class FakeProcesses:
    """psql answers the retained data directory; cargo writes the seed test's record."""

    def __init__(self, workspace, data_directory=None, cargo_status=0, result=None):
        self.workspace = workspace
        self.data_directory = data_directory or str(workspace.cluster / "pgdata")
        self.cargo_status = cargo_status
        self.result = seed_result() if result is None else result
        self.calls = []

    def __call__(self, command, **kwargs):
        self.calls.append((command, kwargs))
        if command[0] == "psql":
            sql = command[-1]
            if sql == "SHOW data_directory":
                return subprocess.CompletedProcess(command, 0, self.data_directory + "\n", "")
            return subprocess.CompletedProcess(command, 0, "DROP DATABASE\n", "")
        if command[0] == "cargo":
            if self.cargo_status == 0:
                Path(kwargs["env"]["CANTOS_THEATRE_SEED_RECORD"]).write_text(json.dumps(self.result))
            return subprocess.CompletedProcess(command, self.cargo_status)
        raise AssertionError(f"unexpected command {command}")

    def sql(self):
        return [command[-1] for command, _ in self.calls if command[0] == "psql"]


class RetainedClusterTest(unittest.TestCase):
    def test_only_a_retained_cluster_created_by_this_checkout_is_a_target(self):
        with Workspace() as w:
            self.assertEqual(seed_theatre.retained_cluster(w.config), (54321, w.cluster / "pgdata"))
            outside = Path(tempfile.mkdtemp()) / "cluster-1"
            (outside / "pgdata").mkdir(parents=True)
            (outside / "pgdata" / "PG_VERSION").write_text("17\n")
            refusals = [
                dict(w.config, retained=False), dict(w.config, retained=1),
                {key: value for key, value in w.config.items() if key != "theatre_app_url"},
                dict(w.config, port=True), dict(w.config, port="54321"), dict(w.config, port=80),
                dict(w.config, cluster=str(outside)),
                dict(w.config, cluster=str(w.evidence / "production")),
                dict(w.config, cluster=str(w.evidence / "cluster-2")),
            ]
            for config in refusals:
                with self.subTest(config=config), self.assertRaises(seed_theatre.Refusal):
                    seed_theatre.retained_cluster(config)

    def test_the_port_must_still_be_served_by_the_retained_data_directory(self):
        with Workspace() as w, patch.object(subprocess, "run", FakeProcesses(w, data_directory="/var/lib/postgresql/data")):
            with self.assertRaisesRegex(seed_theatre.Refusal, "another PostgreSQL data directory"):
                seed_theatre.confirm_cluster({}, 54321, w.cluster / "pgdata")
        with Workspace() as w, patch.object(subprocess, "run",
                                            return_value=subprocess.CompletedProcess([], 2, "", "refused")):
            with self.assertRaisesRegex(seed_theatre.Refusal, "not reachable"):
                seed_theatre.confirm_cluster({}, 54321, w.cluster / "pgdata")

    def test_inherited_connection_settings_never_reach_the_seed(self):
        inherited = {"PATH": "/bin", "DATABASE_URL": "postgresql://prod", "PGHOST": "prod",
                     "PGSERVICE": "prod", "CANTOS_TEST_CLUSTER_URL": "postgresql://other",
                     "RUSTUP_TOOLCHAIN": "1.98.1"}
        self.assertEqual(seed_theatre.isolated_environment(inherited),
                         {"PATH": "/bin", "RUSTUP_TOOLCHAIN": "1.98.1"})


class SeedRecordTest(unittest.TestCase):
    def test_a_previous_seed_is_owned_only_for_the_same_cluster_and_a_safe_name(self):
        with Workspace() as w:
            own = {"cluster": w.config["cluster"], "port": 54321, "database": PREVIOUS}
            self.assertEqual(seed_theatre.owned_seed_database(w.config, own), PREVIOUS)
            for other in (dict(own, cluster="/elsewhere/cluster-9"), dict(own, port=1), None, []):
                self.assertIsNone(seed_theatre.owned_seed_database(w.config, other))
            for unsafe in ("cantos_test_audio", 'cantos_test_x";DROP DATABASE postgres;', None, "production"):
                with self.subTest(unsafe=unsafe), self.assertRaises(seed_theatre.Refusal):
                    seed_theatre.owned_seed_database(w.config, dict(own, database=unsafe))

    def test_the_seed_test_result_is_checked_before_it_is_recorded(self):
        with Workspace() as w:
            self.assertEqual(seed_theatre.validated_result(seed_result(), w.config)["database"], DATABASE)
            broken = [[], dict(seed_result(), format=2), dict(seed_result(), database="cantos_test_audio"),
                      dict(seed_result(), database="production"), dict(seed_result(), works=[]),
                      dict(seed_result(), works=[{"fate": "leaked", "units": [{}]}]),
                      dict(seed_result(), works=[{"fate": "public", "units": []}]),
                      dict(seed_result(), rights_valid_until_ms=None),
                      dict(seed_result(), rights_valid_until_ms=True)]
            for result in broken:
                with self.subTest(result=result), self.assertRaises(seed_theatre.Refusal):
                    seed_theatre.validated_result(result, w.config)

    def test_drop_never_targets_a_verification_fixture(self):
        with Workspace() as w, patch.object(subprocess, "run") as run:
            for name in ("cantos_test_audio", "cantos_test_editorial", "postgres"):
                with self.subTest(name=name), self.assertRaises(seed_theatre.Refusal):
                    seed_theatre.drop({}, 54321, name, w.config)
            run.assert_not_called()


class SeedFlowTest(unittest.TestCase):
    def test_seed_runs_the_exact_test_against_the_admin_url_then_replaces_the_previous_seed(self):
        with Workspace() as w:
            w.seed_record.write_text(json.dumps({"cluster": w.config["cluster"], "port": 54321,
                                                 "database": PREVIOUS}))
            fake = FakeProcesses(w)
            with patch.object(subprocess, "run", fake), patch.object(sys, "stdout", io.StringIO()) as out:
                seed_theatre.seed(w.config, {"PATH": "/bin"})
            cargo = [(command, kwargs) for command, kwargs in fake.calls if command[0] == "cargo"]
            self.assertEqual(len(cargo), 1)
            command, kwargs = cargo[0]
            self.assertEqual(command, ["cargo", "test", "-p", "cantos-api", "--test", "catalog_http_postgres",
                                       "--locked", "--offline", "--", "--ignored", "--exact",
                                       seed_theatre.SEED_TEST, "--test-threads=1"])
            self.assertEqual(kwargs["env"]["CANTOS_TEST_CLUSTER_URL"],
                             "postgresql://cantos_test_admin@127.0.0.1:54321/postgres")
            self.assertNotIn("DATABASE_URL", kwargs["env"])
            record = json.loads(w.seed_record.read_text())
            self.assertEqual((record["database"], record["cluster"], record["port"], record["app_url"]),
                             (DATABASE, w.config["cluster"], 54321,
                              f"postgresql://cantos_app@127.0.0.1:54321/{DATABASE}"))
            self.assertEqual(fake.sql(), ["SHOW data_directory",
                                          f'DROP DATABASE IF EXISTS "{PREVIOUS}" WITH (FORCE)'])
            self.assertIn("public      novel   5eed0001-0000-4000-8000-000000000000  1/1 public units",
                          out.getvalue())
            self.assertIn("withdrawn   podcast 5eed0005-0000-4000-8000-000000000000  0/1 public units",
                          out.getvalue())
            self.assertEqual([p.name for p in w.evidence.iterdir() if p.name.startswith("tmp")], [])

    def test_a_failed_seed_writes_no_record_and_keeps_the_previous_seed(self):
        with Workspace() as w:
            previous = {"cluster": w.config["cluster"], "port": 54321, "database": PREVIOUS}
            w.seed_record.write_text(json.dumps(previous))
            for fake in (FakeProcesses(w, cargo_status=101),
                         FakeProcesses(w, result=dict(seed_result(), database="cantos_test_audio"))):
                with self.subTest(status=fake.cargo_status), patch.object(subprocess, "run", fake), \
                        self.assertRaises(seed_theatre.Refusal):
                    seed_theatre.seed(w.config, {})
                self.assertEqual(json.loads(w.seed_record.read_text()), previous)
                self.assertEqual(fake.sql(), ["SHOW data_directory"])

    def test_seed_refuses_a_port_owned_by_another_server_before_running_cargo(self):
        with Workspace() as w:
            fake = FakeProcesses(w, data_directory="/srv/production/pgdata")
            with patch.object(subprocess, "run", fake), self.assertRaises(seed_theatre.Refusal):
                seed_theatre.seed(w.config, {})
            self.assertEqual([command[0] for command, _ in fake.calls], ["psql"])
            self.assertFalse(w.seed_record.exists())

    def test_reset_drops_only_the_recorded_seed_and_removes_its_record(self):
        with Workspace() as w:
            w.seed_record.write_text(json.dumps({"cluster": w.config["cluster"], "port": 54321,
                                                 "database": PREVIOUS}))
            fake = FakeProcesses(w)
            with patch.object(subprocess, "run", fake), patch.object(sys, "stdout", io.StringIO()):
                seed_theatre.reset(w.config, {})
            self.assertEqual(fake.sql(), ["SHOW data_directory",
                                          f'DROP DATABASE IF EXISTS "{PREVIOUS}" WITH (FORCE)'])
            self.assertFalse(w.seed_record.exists())
            # A record from another cluster is removed without touching any database.
            w.seed_record.write_text(json.dumps({"cluster": "/other/cluster-3", "port": 1, "database": PREVIOUS}))
            fake = FakeProcesses(w)
            with patch.object(subprocess, "run", fake), patch.object(sys, "stdout", io.StringIO()) as out:
                seed_theatre.reset(w.config, {})
            self.assertEqual(fake.sql(), ["SHOW data_directory"])
            self.assertIn("only the record was removed", out.getvalue())
            self.assertFalse(w.seed_record.exists())

    def test_unreadable_records_are_refusals_that_change_nothing(self):
        with Workspace() as w:
            w.seed_record.write_text("{not json")
            fake = FakeProcesses(w)
            with patch.object(subprocess, "run", fake), self.assertRaisesRegex(seed_theatre.Refusal, "unreadable"):
                seed_theatre.seed(w.config, {})
            self.assertEqual([command[0] for command, _ in fake.calls], ["psql"])
            self.assertEqual(w.seed_record.read_text(), "{not json")
            w.run_record.write_text("[")
            with patch.object(subprocess, "run") as run, patch.object(sys, "stderr", io.StringIO()) as err:
                self.assertEqual(seed_theatre.main([]), 1)
            run.assert_not_called()
            self.assertIn("run.json is unreadable", err.getvalue())

    def test_main_refuses_without_a_retained_cluster_record(self):
        with Workspace() as w:
            w.run_record.unlink()
            with patch.object(subprocess, "run") as run, patch.object(sys, "stderr", io.StringIO()) as err:
                self.assertEqual(seed_theatre.main([]), 1)
                self.assertEqual(seed_theatre.main(["--reset"]), 1)
            run.assert_not_called()
            self.assertIn("cargo xtask verify postgres --keep", err.getvalue())


class ServeSeedTest(unittest.TestCase):
    def test_theatre_serves_the_recorded_seed_and_announces_it(self):
        with Workspace() as w:
            record = dict(seed_result(), cluster=w.config["cluster"], port=54321)
            w.seed_record.write_text(json.dumps(record))
            output = io.StringIO()
            with patch.object(serve_studio, "ROOT", w.root), \
                    patch.dict(os.environ, {"CANTOS_TEST_CONTEXT": "theatre"}, clear=True), \
                    patch.object(os, "chdir"), patch.object(os, "execve") as execute, \
                    patch.object(sys, "stdout", output):
                serve_studio.main()
            env = execute.call_args.args[2]
            self.assertEqual(env["DATABASE_URL"], f"postgresql://cantos_app@127.0.0.1:54321/{DATABASE}")
            self.assertIn("Theatre seed catalog: 1 public works", output.getvalue())


if __name__ == "__main__":
    unittest.main()
