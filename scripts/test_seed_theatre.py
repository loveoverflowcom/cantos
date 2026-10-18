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

    def test_independent_failed_narrow_ownership_can_seed_a_new_database(self):
        with Workspace() as w:
            cluster=w.evidence/"schema"/"cluster-2"
            (cluster/"pgdata").mkdir(parents=True)
            (cluster/"pgdata"/"PG_VERSION").write_text("18\n")
            proof={"format":1,"purpose":"retained-disposable-cluster","scope":"schema",
                   "cluster":str(cluster),"port":54321,"retained":True,"runner_pid":99999999,
                   "runner_identity":None,"runner_active":False,"gate_status":"FAIL","full_gate_status":"NOT_RUN"}
            self.assertEqual(seed_theatre.retained_cluster(proof),(54321,cluster/"pgdata"))
            with self.assertRaisesRegex(seed_theatre.Refusal,"still active"):
                seed_theatre.retained_cluster(dict(proof,runner_active=True))
            with self.assertRaisesRegex(seed_theatre.Refusal,"scope/directory"):
                seed_theatre.retained_cluster(dict(proof,scope="full",full_gate_status="FAIL"))

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

    def test_ownership_reset_preserves_matching_complete_runner_fixture_blacklist(self):
        with Workspace() as w,patch.object(subprocess,"run") as execute:
            ownership={"purpose":"retained-disposable-cluster","cluster":w.config["cluster"],"port":54321}
            fixture=PREVIOUS
            w.config["audio_app_url"]=f"postgresql://cantos_app@127.0.0.1:54321/{fixture}"
            w.run_record.write_text(json.dumps(w.config))
            seed={"cluster":w.config["cluster"],"port":54321,"database":fixture}
            with self.assertRaisesRegex(seed_theatre.Refusal,"verification fixture") :
                seed_theatre.owned_seed_database(ownership,seed)
            with self.assertRaises(seed_theatre.Refusal):
                seed_theatre.drop({},54321,fixture,ownership)
            self.assertIn(fixture,seed_theatre.protected_databases(ownership))
            self.assertNotIn("audio_app_url",ownership)
            execute.assert_not_called()

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


class PersistentSeedTest(unittest.TestCase):
    def arguments(self, root, *extra):
        return ["--environment", "catalog-local", "--api-origin", "https://api.example.org",
                "--web-origin", "https://studio.example.org", "--database-url-env", "SEED_DB",
                "--studio-token-env", "SEED_TOKEN", "--rights-ref", "synthetic-owned",
                "--private-root", str(root / "private"), "--publication-root", str(root / "publication"),
                "--record", str(root / "receipt.json"), *extra]

    def test_persistent_lane_uses_only_explicit_credentials_and_never_runs_database_admin(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            (root / "private").mkdir(mode=0o700)
            (root / "publication").mkdir(mode=0o700)
            calls=[]
            def run(command, **kwargs):
                calls.append((command,kwargs))
                self.assertEqual(command[0],"cargo")
                env=kwargs["env"]
                self.assertNotIn("DATABASE_URL",env)
                self.assertNotIn("PGHOST",env)
                self.assertNotIn("CANTOS_TEST_CLUSTER_URL",env)
                self.assertEqual(env["CANTOS_THEATRE_SEED_DATABASE_URL"],"postgresql://chosen")
                self.assertEqual(env["CANTOS_THEATRE_SEED_STUDIO_TOKEN"],"chosen-secret")
                self.assertEqual(env["CANTOS_THEATRE_SEED_STUDIO_COOKIE_NAME"],"__Secure-cantos_studio")
                receipt=dict(seed_result(database="existing"),dataset="theatre-catalog-v2",environment="catalog-local")
                Path(env["CANTOS_THEATRE_SEED_RECORD"]).write_text(json.dumps(receipt))
                return subprocess.CompletedProcess(command,0)
            inherited={"SEED_DB":"postgresql://chosen","SEED_TOKEN":"chosen-secret",
                       "DATABASE_URL":"postgresql://wrong","PGHOST":"wrong","CANTOS_TEST_CLUSTER_URL":"wrong"}
            with patch.dict(os.environ,inherited,clear=True), patch.object(subprocess,"run",run), patch.object(sys,"stdout",io.StringIO()):
                self.assertEqual(seed_theatre.main(self.arguments(root)),0)
            self.assertEqual(len(calls),1)
            self.assertEqual((root / "receipt.lock").stat().st_mode & 0o777,0o600)

    def test_persistent_reset_and_missing_inputs_never_run_a_process(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(subprocess,"run") as run, patch.object(sys,"stderr",io.StringIO()):
            root=Path(directory)
            self.assertEqual(seed_theatre.main(self.arguments(root,"--reset")),1)
            self.assertEqual(seed_theatre.main(["--environment","catalog-local"]),1)
            self.assertEqual(seed_theatre.main(["--record",str(root / "receipt.json")]),1)
            run.assert_not_called()

    def test_unknown_or_injected_cookie_name_is_refused_before_any_process(self):
        with patch.object(subprocess,"run") as run, patch.object(sys,"stderr",io.StringIO()):
            for name in ("arbitrary","cantos_session;evil=x","cantos_session\r\nX-Evil: x"):
                with self.assertRaises(SystemExit) as stopped:
                    seed_theatre.main(["--environment","catalog-local","--studio-cookie-name",name])
                self.assertEqual(stopped.exception.code,2)
            run.assert_not_called()

    def test_persistent_lane_refuses_plaintext_remote_or_credential_origins(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(subprocess,"run") as run, patch.object(sys,"stderr",io.StringIO()):
            for origin in ("http://api.example.org", "https://user:secret@api.example.org", "https://api.example.org/path"):
                args=self.arguments(Path(directory))
                args[args.index("--api-origin")+1]=origin
                self.assertEqual(seed_theatre.main(args),1)
            run.assert_not_called()

    def test_persistent_lane_refuses_overlapping_storage_roots(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(subprocess,"run") as run, patch.object(sys,"stderr",io.StringIO()), patch.dict(os.environ,{"SEED_DB":"chosen","SEED_TOKEN":"secret"},clear=True):
            root=Path(directory)
            args=self.arguments(root)
            args[args.index("--publication-root")+1]=str(root / "private" / "public")
            self.assertEqual(seed_theatre.main(args),1)
            run.assert_not_called()


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
