"""Runner safety and fail-closed guards; process/HTTP doubles are not PG evidence."""
import contextlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import MagicMock, patch

import test_postgres as runner
import serve_studio


class PostgresRunnerTests(unittest.TestCase):
    def test_scoped_sql_selection_refuses_retention_and_unknown_suites_before_tools(self):
        self.assertEqual(runner.parse_args([]).suite, "full")
        self.assertTrue(runner.parse_args(["--keep"]).keep)
        self.assertEqual(runner.parse_args(["--suite", "theatre-firebase"]).suite,
                         "theatre-firebase")
        self.assertEqual(runner.parse_args(["--suite", "schema"]).suite, "schema")
        for arguments in (["--suite", "theatre-firebase", "--keep"],
                          ["--suite", "schema", "--keep"],
                          ["--suite", "legacy-dbsp"],
                          ["--suite", "unregistered"]):
            with self.subTest(arguments=arguments), contextlib.redirect_stderr(io.StringIO()), \
                    patch.object(runner.shutil, "which") as probe:
                with patch.object(sys, "argv", ["test_postgres.py", *arguments]), \
                        self.assertRaises(SystemExit):
                    runner.main()
                probe.assert_not_called()

    def test_scoped_sql_owns_fresh_cluster_environment_filters_and_stop(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            commands = []

            def execute(command, env=None, output=None):
                commands.append((command, dict(env or {}), output))
                return "test result: ok. 3 passed; 0 failed; 0 ignored; 17 filtered out;\n"

            inherited = {"PATH": "/installed", "DATABASE_URL": "postgresql://private.invalid/live",
                         "PGHOST": "private.invalid", "CANTOS_TEST_CLUSTER_URL": "private"}
            with patch.object(runner, "EVIDENCE", evidence), \
                    patch.object(sys, "argv", ["test_postgres.py", "--suite", "theatre-firebase"]), \
                    patch.dict(os.environ, inherited, clear=True), \
                    patch.object(runner.shutil, "which", return_value="/installed/tool"), \
                    patch.object(runner, "free_port", return_value=12345), \
                    patch.object(runner, "run", side_effect=execute), \
                    patch.object(runner.subprocess, "Popen") as host, \
                    contextlib.redirect_stdout(io.StringIO()) as output:
                runner.main()
            self.assertEqual([command[0] for command, _, _ in commands],
                             ["initdb", "pg_ctl", "psql", "cargo", "cargo", "pg_ctl"])
            cluster = Path(commands[0][0][2]).parent
            self.assertEqual(cluster.parent, evidence / "theatre-firebase")
            self.assertTrue(cluster.is_dir())
            self.assertEqual(commands[-1][0],
                             ["pg_ctl", "-D", str(cluster / "pgdata"), "-m", "fast", "-w", "stop"])
            for _, environment, _ in commands:
                self.assertNotIn("DATABASE_URL", environment)
                self.assertNotIn("PGHOST", environment)
                self.assertEqual(environment["CANTOS_TEST_CLUSTER_URL"],
                                 "postgresql://cantos_test_admin@127.0.0.1:12345/postgres")
            sql_commands = [command for command, _, _ in commands if command[0] == "cargo"]
            self.assertEqual([command[7] for command in sql_commands], [
                "platform::theatre_registration::firebase_enrollment::postgres::tests",
                "platform::theatre_registration::postgres::tests"])
            for command in sql_commands:
                self.assertIn("--offline", command)
                self.assertEqual(command[-4:], ["--", "--ignored", "--test-threads=1", "--nocapture"])
            record = json.loads((evidence / "theatre-firebase" / "run.json").read_text())
            self.assertEqual(record["suite"], "theatre-firebase")
            self.assertEqual(record["full_suite"], "NOT_RUN")
            self.assertFalse(record["retained"])
            self.assertFalse((evidence / "run.json").exists())
            self.assertIn("full PostgreSQL/media/HTTP/restart campaign NOT_RUN", output.getvalue())
            host.assert_not_called()

    def test_schema_scope_runs_current_sql_targets_in_an_isolated_owned_cluster(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            commands = []

            def execute(command, env=None, output=None):
                commands.append((command, dict(env or {}), output))
                return "test result: ok. 2 passed; 0 failed; 0 ignored; 0 filtered out;\n"

            inherited = {"PATH": "/installed", "DATABASE_URL": "postgresql://private.invalid/live",
                         "PGHOSTADDR": "192.0.2.1", "PGSERVICE": "live",
                         "CANTOS_TEST_CLUSTER_URL": "private", "CANTOS_ENV": "production"}
            with patch.object(runner, "EVIDENCE", evidence), \
                    patch.object(sys, "argv", ["test_postgres.py", "--suite", "schema"]), \
                    patch.dict(os.environ, inherited, clear=True), \
                    patch.object(runner.shutil, "which", return_value="/installed/tool") as probe, \
                    patch.object(runner, "free_port", return_value=12345), \
                    patch.object(runner, "run", side_effect=execute), \
                    patch.object(runner, "reap_completed_databases") as reap, \
                    patch.object(runner.subprocess, "Popen") as host, \
                    contextlib.redirect_stdout(io.StringIO()) as output:
                runner.main()
            self.assertEqual([call.args[0] for call in probe.call_args_list],
                             ["initdb", "pg_ctl", "psql", "cargo"])
            self.assertEqual([command[0] for command, _, _ in commands],
                             ["initdb", "pg_ctl", "psql"] + ["cargo"] * 14 + ["pg_ctl"])
            cluster = Path(commands[0][0][2]).parent
            self.assertEqual(cluster.parent, evidence / "schema")
            self.assertEqual(commands[-1][0],
                             ["pg_ctl", "-D", str(cluster / "pgdata"), "-m", "fast", "-w", "stop"])
            for _, environment, _ in commands:
                self.assertEqual(environment, {
                    "PATH": "/installed",
                    "CANTOS_TEST_CLUSTER_URL": "postgresql://cantos_test_admin@127.0.0.1:12345/postgres"})
            targets = ["schema_postgres", "production_grants_postgres", "adaptations_postgres",
                       "imports_postgres", "revisions_postgres", "catalog_http_postgres",
                       "catalog_postgres", "delegation_http_postgres", "identity_postgres", "theatre_identity_postgres",
                       "theatre_native_postgres", "studio_oidc_postgres"]
            logs = ["schema-postgres-tests", "production-grants-postgres-tests",
                    "adaptation-postgres-tests", "import-postgres-tests", "revision-postgres-tests",
                    "catalog-http-postgres-tests", "catalog-postgres-tests", "delegation-http-postgres-tests", "identity-postgres-tests",
                    "theatre-identity-postgres-tests", "theatre-native-postgres-tests",
                    "studio-oidc-postgres-tests", "theatre-firebase-postgres-tests",
                    "theatre-registration-postgres-tests"]
            sql_commands = commands[3:-1]
            self.assertEqual([command[5] for command, _, _ in sql_commands[:12]], targets)
            selectors = ["platform::theatre_registration::firebase_enrollment::postgres::tests",
                         "platform::theatre_registration::postgres::tests"]
            self.assertEqual([command[7] for command, _, _ in sql_commands[12:]], selectors)
            for (command, _, path), name in zip(sql_commands, logs):
                self.assertIn("--offline", command)
                self.assertIn("--ignored", command)
                self.assertIn("--test-threads=1", command)
                self.assertEqual(path, evidence / "schema" / f"{name}.log")
            skipped = {
                "adaptations_postgres": ["studio_editor_import_proposal_edit_accept_review_save_and_reopen_preserve_history"],
                "imports_postgres": ["import_restart_fixture_preserves_exact_bytes_receipt_and_same_operation"],
                "revisions_postgres": ["restart_fixture_for_process_and_database_recovery"],
                "catalog_http_postgres": ["catalog_http_recovery_after_server_restart",
                                          "committed_http_catalog_emits_exact_postgres_restart_oracle",
                                          "audio_lifecycle_http::"],
                "catalog_postgres": ["catalog_recovery_after_server_restart",
                                     "committed_catalog_fixture_survives_new_store_and_emits_server_restart_oracle"],
            }
            for command, _, _ in sql_commands[:12]:
                actual_skips = [command[index + 1] for index, arg in enumerate(command) if arg == "--skip"]
                self.assertEqual(actual_skips, skipped.get(command[5], []))
            record = json.loads((evidence / "schema" / "run.json").read_text())
            self.assertEqual(record["suite"], "schema")
            self.assertEqual(record["checks"], logs)
            self.assertEqual(record["integration_targets"], targets)
            self.assertEqual(record["registration_selectors"], selectors)
            self.assertEqual(record["skipped_tests"], skipped)
            self.assertEqual(record["full_suite"], "NOT_RUN")
            self.assertFalse(record["retained"])
            self.assertFalse((evidence / "run.json").exists())
            self.assertEqual(reap.call_count, 13)
            for call in reap.call_args_list:
                self.assertEqual(call.args[1], "postgresql://cantos_test_admin@127.0.0.1:12345/postgres")
                self.assertEqual(call.args[2], set())
            self.assertIn("full PostgreSQL/media/HTTP/restart campaign NOT_RUN", output.getvalue())
            host.assert_not_called()

    def test_scoped_sql_failure_or_interrupt_still_stops_its_owned_cluster(self):
        for failure in (RuntimeError("synthetic SQL failure"), KeyboardInterrupt()):
            with self.subTest(failure=type(failure).__name__), tempfile.TemporaryDirectory() as directory:
                commands = []

                def execute(command, env=None, output=None):
                    commands.append(command)
                    if command[0] == "cargo":
                        raise failure
                    return ""

                with patch.object(runner, "EVIDENCE", Path(directory)), \
                        patch.object(sys, "argv", ["test_postgres.py", "--suite", "theatre-firebase"]), \
                        patch.object(runner.shutil, "which", return_value="/installed/tool"), \
                        patch.object(runner, "free_port", return_value=12345), \
                        patch.object(runner, "run", side_effect=execute), \
                        self.assertRaises(type(failure)):
                    runner.main()
                self.assertEqual(commands[-1][0], "pg_ctl")
                self.assertEqual(commands[-1][-4:], ["-m", "fast", "-w", "stop"])
                self.assertFalse((Path(directory) / "theatre-firebase" / "run.json").exists())

    def test_registration_sql_selection_refuses_zero_selected_or_missing_summary(self):
        for output in ("test result: ok. 0 passed; 0 failed; 0 ignored;\n", "compiled only\n"):
            with self.subTest(output=output), patch.object(runner, "run", return_value=output), \
                    self.assertRaisesRegex(RuntimeError, "selected no passing SQL tests"):
                runner.registration_postgres_tests({}, Path("/synthetic/evidence"))

    def test_schema_scope_stops_owned_cluster_on_error_or_interrupt(self):
        for failure in (RuntimeError("synthetic SQL failure"), KeyboardInterrupt()):
            with self.subTest(failure=type(failure).__name__), tempfile.TemporaryDirectory() as directory:
                commands = []

                def execute(command, env=None, output=None):
                    commands.append(command)
                    if command[0] == "cargo":
                        raise failure
                    return ""

                with patch.object(runner, "EVIDENCE", Path(directory)), \
                        patch.object(sys, "argv", ["test_postgres.py", "--suite", "schema"]), \
                        patch.object(runner.shutil, "which", return_value="/installed/tool"), \
                        patch.object(runner, "free_port", return_value=12345), \
                        patch.object(runner, "run", side_effect=execute), \
                        self.assertRaises(type(failure)):
                    runner.main()
                self.assertEqual(commands[-1][0], "pg_ctl")
                self.assertEqual(commands[-1][-4:], ["-m", "fast", "-w", "stop"])
                self.assertFalse((Path(directory) / "schema" / "run.json").exists())

    def test_schema_selection_refuses_zero_missing_or_failed_test_summary_for_every_target(self):
        valid = "test result: ok. 2 passed; 0 failed; 0 ignored;\n"
        invalid = ("test result: ok. 0 passed; 0 failed; 0 ignored;\n", "compiled only\n",
                   "test result: ok. 2 passed; 1 failed;\n")
        targets = ["schema-postgres-tests", "production-grants-postgres-tests",
                   "adaptation-postgres-tests", "import-postgres-tests", "revision-postgres-tests",
                   "catalog-http-postgres-tests", "catalog-postgres-tests", "delegation-http-postgres-tests", "identity-postgres-tests",
                   "theatre-identity-postgres-tests", "theatre-native-postgres-tests",
                   "studio-oidc-postgres-tests", "theatre-firebase-postgres-tests",
                   "theatre-registration-postgres-tests"]
        for index, target in enumerate(targets):
            for output in invalid:
                with self.subTest(target=target, output=output), \
                        patch.object(runner, "run", side_effect=[valid] * index + [output]), \
                        patch.object(runner, "reap_completed_databases"), \
                        self.assertRaisesRegex(RuntimeError, f"{target} selected no passing SQL tests"):
                    runner.schema_postgres_tests({"CANTOS_TEST_CLUSTER_URL": "owned"},
                                                 Path("/synthetic/evidence"), include_current=True)

    def test_full_postgres_gate_includes_schema_before_existing_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            commands = []

            def execute(command, env=None, output=None):
                commands.append(command)
                if "admin_postgres" in command:
                    raise RuntimeError("stop synthetic full suite after schema target")
                return "test result: ok. 2 passed; 0 failed; 0 ignored;\n"

            with patch.object(runner, "EVIDENCE", Path(directory)), \
                    patch.object(sys, "argv", ["test_postgres.py"]), \
                    patch.object(runner.shutil, "which", return_value="/installed/tool"), \
                    patch.object(runner, "free_port", return_value=12345), \
                    patch.object(runner, "run", side_effect=execute), \
                    patch.object(runner, "reap_completed_databases"), \
                    self.assertRaisesRegex(RuntimeError, "stop synthetic full suite"):
                runner.main()
            self.assertEqual([command[5] for command in commands if command[0] == "cargo"],
                             ["schema_postgres", "production_grants_postgres", "admin_postgres"])
            self.assertEqual(commands[-1][-4:], ["-m", "fast", "-w", "stop"])

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

    def test_theatre_fixture_adds_only_required_discovery_reads_to_the_retained_audio_database(self):
        environment = {"PATH": "/synthetic/bin"}
        with patch.object(runner, "run", return_value="GRANT\nGRANT\n0\n") as execute:
            url = runner.prepare_theatre_fixture(environment, 12345, {"database": "cantos_test_audio"})
        self.assertEqual(url, "postgresql://cantos_app@127.0.0.1:12345/cantos_test_audio")
        command = execute.call_args.args[0]
        self.assertEqual(command[:3], ["psql", "-X", "postgresql://cantos_test_admin@127.0.0.1:12345/cantos_test_audio"])
        self.assertIn("ON_ERROR_STOP=1", command)
        self.assertEqual(execute.call_args.kwargs["env"], environment)
        self.assertEqual(command[-1].split("; ")[:2], [
            "GRANT SELECT ON audio_distribution_clearances,audio_publication_approvals,"
            "audio_releases,audio_release_withdrawals,audio_publication_pointers,"
            "audio_listener_works,audio_listener_units,audio_listener_releases TO cantos_app",
            "GRANT EXECUTE ON FUNCTION catalog_listener_discoverable(text,text),"
            "listener_actor_active(text) TO cantos_app",
        ])
        self.assertIn("SELECT count(*) FROM audio_releases", command[-1])
        self.assertIn("SELECT count(*) FROM audio_listener_releases", command[-1])
        for mutation in ("INSERT", "UPDATE", "DELETE", "CREATE", "DROP"):
            self.assertNotIn(mutation, command[-1])

    def test_theatre_fixture_refuses_unsafe_database_names_and_existing_publications(self):
        for database in ("production", 'cantos_test_audio";DROP DATABASE postgres;', None):
            with self.subTest(database=database), patch.object(runner, "run") as execute, \
                    self.assertRaisesRegex(ValueError, "disposable test database name"):
                runner.prepare_theatre_fixture({}, 12345, {"database": database})
            execute.assert_not_called()
        for observed in ("GRANT\nGRANT\n1\n", "", "GRANT\nGRANT\n"):
            with self.subTest(observed=observed), patch.object(runner, "run", return_value=observed), \
                    self.assertRaisesRegex(RuntimeError, "no publications"):
                runner.prepare_theatre_fixture({}, 12345, {"database": "cantos_test_audio"})

    def test_theatre_probe_requires_exact_anonymous_empty_json_not_an_outage_or_html(self):
        response = MagicMock()
        response.__enter__.return_value = response
        response.status = 200
        response.headers.get_content_type.return_value = "application/json"
        response.headers.__getitem__.return_value = "no-store"
        response.read.return_value = b'{"items":[],"next_cursor":null}'
        opener = MagicMock()
        opener.open.return_value = response
        with patch.object(runner.urllib.request, "build_opener", return_value=opener):
            runner.theatre_probe(12345)
        request = opener.open.call_args.args[0]
        self.assertEqual(request.full_url, "http://127.0.0.1:12345/api/v1/theatre/works?limit=20")
        self.assertEqual(request.get_header("Accept"), "application/json")
        self.assertIsNone(request.get_header("Cookie"))
        self.assertIsNone(request.get_header("Authorization"))
        self.assertEqual(opener.open.call_args.kwargs["timeout"], 3)
        for status, content_type, body in (
            (503, "application/json", b'{"code":"unavailable","retryable":true}'),
            (200, "text/html", b'{"items":[],"next_cursor":null}'),
            (200, "application/json", b'{"items":[{"invented":"sample"}],"next_cursor":null}'),
        ):
            response.status = status
            response.headers.get_content_type.return_value = content_type
            response.read.return_value = body
            with self.subTest(status=status, content_type=content_type, body=body), \
                    patch.object(runner.urllib.request, "build_opener", return_value=opener), \
                    self.assertRaises(AssertionError):
                runner.theatre_probe(12345)

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

    def test_every_actual_restart_database_is_retained_between_suites(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            revision, editor, catalog = (root / name for name in ("revision", "editor", "catalog"))
            for folder in (revision, editor, catalog):
                folder.mkdir()
            paths = (revision / "restart.json", editor / "editor-api-journey.json",
                     catalog / "restart.json", catalog / "http-restart.json", catalog / "audio-restart.json")
            expected = {f"cantos_test_recovery_{index}" for index in range(5)}
            for index, path in enumerate(paths):
                path.write_text(json.dumps({"database": f"cantos_test_recovery_{index}"}))
            with patch.object(runner, "EVIDENCE", revision), patch.object(runner, "EDITOR_EVIDENCE", editor), patch.object(runner, "CATALOG_EVIDENCE", catalog):
                self.assertEqual(runner.retained_restart_databases(), expected)

    def test_completed_database_cleanup_preserves_restart_fixtures_and_other_names(self):
        listed = subprocess.CompletedProcess([], 0, "postgres\ncantos_test_completed\ncantos_test_recovery\nother_database\n", "")
        dropped = subprocess.CompletedProcess([], 0, "", "")
        with patch.object(runner.subprocess, "run", side_effect=[listed, dropped]) as execute:
            runner.reap_completed_databases({}, "postgresql://cantos_test_admin@127.0.0.1:12345/postgres", {"cantos_test_recovery"})
        self.assertEqual(execute.call_count, 2)
        self.assertEqual(execute.call_args.args[0][-1], 'DROP DATABASE "cantos_test_completed"')
        self.assertNotIn("FORCE", execute.call_args.args[0][-1])

    def test_completed_cleanup_refuses_inherited_or_non_cluster_url_and_unsafe_names(self):
        for url in ["postgresql://cantos_app@127.0.0.1:12345/postgres", "postgresql://cantos_test_admin@example.com/postgres", "postgresql://cantos_test_admin@127.0.0.1:12345/production"]:
            with patch.object(runner.subprocess, "run") as execute, self.assertRaises(ValueError):
                runner.reap_completed_databases({}, url, set())
            execute.assert_not_called()
        listed = subprocess.CompletedProcess([], 0, 'cantos_test_unsafe";drop database postgres;\n', "")
        with patch.object(runner.subprocess, "run", return_value=listed) as execute, self.assertRaises(ValueError):
            runner.reap_completed_databases({}, "postgresql://cantos_test_admin@127.0.0.1:12345/postgres", set())
        self.assertEqual(execute.call_count, 1)

    def test_optimized_python_refuses_before_cluster_or_host_start(self):
        result = subprocess.run([sys.executable, "-O", str(Path(runner.__file__)), "--help"],
                                text=True, capture_output=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("PostgreSQL assertions are required; run Python without -O", result.stderr)
        self.assertNotIn("PASS:", result.stdout)

    def test_inherited_pg_and_host_configuration_cannot_escape_the_disposable_cluster(self):
        inherited = {"PGHOSTADDR": "192.0.2.1", "PGSERVICE": "production",
                     "PGOPTIONS": "-c search_path=private", "PGPASSWORD": "synthetic",
                     "DATABASE_URL": "postgresql://example.invalid/production",
                     "CANTOS_WEB_ORIGIN": "https://example.invalid",
                     "CANTOS_WEB_DIST": "/tmp/private-test-content",
                     "CANTOS_SESSION_TOKEN": "inherited-token-must-not-be-used",
                     "CANTOS_TEST_CLUSTER_URL": "postgresql://example.invalid/other"}
        with patch.dict(os.environ, inherited):
            environment = runner.isolated_environment()
        for key in inherited:
            self.assertNotIn(key, environment)
        self.assertIn("PATH", environment)

    def test_readiness_mismatch_stops_and_reaps_the_unreturned_host(self):
        host = MagicMock()
        host.poll.return_value = None
        with patch.object(runner.subprocess, "Popen", return_value=host), \
                patch.object(runner, "http", return_value={"wrong": "response"}):
            with self.assertRaises(AssertionError):
                runner.start_host({}, 12345, io.StringIO(), {"response": {"expected": "response"}})
        host.kill.assert_called_once_with()
        host.wait.assert_called_once_with(timeout=5)

    def test_msgpack_probe_sends_independent_bytes_and_decodes_by_response_type(self):
        revision = {"script_id": "s", "revision": 2, "script_json": "{\"t\":\"Ngày\"}"}
        fixture = {"script": "s", "token": "synthetic-session",
                   "request": {"expected_revision": 1, "operation_id": "o", "script_json": "{}"}}
        response = MagicMock()
        response.__enter__.return_value = response
        response.status = 200
        response.headers = {"Cache-Control": "no-store", "X-Content-Type-Options": "nosniff",
                            "Content-Type": "application/msgpack", "Vary": "accept"}
        response.read.return_value = runner.reference_msgpack.encode(revision)
        opener = MagicMock()
        opener.open.return_value = response
        with patch.object(runner.urllib.request, "build_opener", return_value=opener):
            self.assertEqual(runner.msgpack_http(12345, fixture, "POST"), (200, revision))
        sent = opener.open.call_args.args[0]
        self.assertEqual(sent.get_header("Content-type"), "application/msgpack")
        self.assertEqual(sent.get_header("Accept"), "application/msgpack")
        self.assertEqual(runner.reference_msgpack.decode(sent.data), fixture["request"])
        response.headers = {"Cache-Control": "no-store", "X-Content-Type-Options": "nosniff",
                            "Content-Type": "application/json"}
        response.status = 406
        response.read.return_value = b'{"code":"invalid_request"}'
        with patch.object(runner.urllib.request, "build_opener", return_value=opener):
            self.assertEqual(runner.msgpack_http(12345, fixture, accept="text/html"),
                             (406, {"code": "invalid_request"}))
        self.assertIsNone(opener.open.call_args.args[0].data)

    def test_protobuf_probe_sends_independent_request_and_requires_exact_response_bytes(self):
        revision = {"script_id": "s", "revision": 2, "accepted_by": "actor",
                    "accepted_at": "2026-10-13T00:00:00Z", "content_digest": "c", "export_digest": "e",
                    "script_json": '{"title":"Ngày 🎭"}'}
        fixture = {"script": "s", "token": "synthetic-session", "response": revision,
                   "request": {"expected_revision": 1, "operation_id": "o", "script_json": "{}"}}
        expected = runner.reference_protobuf.encode("RevisionResponse", revision)
        response = MagicMock()
        response.__enter__.return_value = response
        response.status = 200
        response.headers = {"Cache-Control": "no-store", "X-Content-Type-Options": "nosniff",
                            "Content-Type": "application/protobuf", "Vary": "accept"}
        response.read.return_value = expected
        opener = MagicMock()
        opener.open.return_value = response
        with patch.object(runner.urllib.request, "build_opener", return_value=opener):
            self.assertEqual(runner.protobuf_http(12345, fixture, "POST"), (200, expected))
        sent = opener.open.call_args.args[0]
        self.assertEqual(sent.get_header("Content-type"), "application/protobuf")
        self.assertEqual(sent.get_header("Accept"), "application/protobuf")
        self.assertEqual(sent.data, runner.reference_protobuf.encode("SaveRevisionRequest", fixture["request"]))
        response.read.return_value = expected + b'\x00'
        with patch.object(runner.urllib.request, "build_opener", return_value=opener):
            with self.assertRaisesRegex(AssertionError, "independent wire oracle"):
                runner.protobuf_http(12345, fixture)
        response.read.return_value = expected
        response.headers["Content-Type"] = "application/json"
        with patch.object(runner.urllib.request, "build_opener", return_value=opener):
            with self.assertRaises(AssertionError):
                runner.protobuf_http(12345, fixture)
        response.status = 406
        response.read.return_value = b'{"code":"invalid_request"}'
        with patch.object(runner.urllib.request, "build_opener", return_value=opener):
            self.assertEqual(runner.protobuf_http(12345, fixture, accept="text/html"),
                             (406, {"code": "invalid_request"}))
        self.assertIsNone(opener.open.call_args.args[0].data)

    def test_editor_readiness_uses_the_latest_saved_revision_without_replacing_the_fixture(self):
        fixture = {"script": "editor-script", "token": "synthetic-session",
                   "first": {"revision": 1}, "second": {"revision": 2},
                   "save_request": {"expected_revision": 1}}
        original = json.loads(json.dumps(fixture))
        probe = runner.editor_probe(fixture)
        self.assertEqual(probe["script"], "editor-script")
        self.assertEqual(probe["response"], {"revision": 2})
        self.assertEqual(probe["request"], {"expected_revision": 1})
        self.assertEqual(fixture, original)

    def test_editor_row_oracle_uses_declared_local_database_and_fails_closed_on_psql_failure(self):
        local_url = "postgresql://cantos_app@127.0.0.1:12345/cantos_test_studio_editor"
        expected = {"revisions": 2, "acceptances": 1}
        completed = subprocess.CompletedProcess([], 0, json.dumps(expected), "")
        with patch.object(runner.subprocess, "run", return_value=completed) as execute:
            self.assertEqual(runner.editor_snapshot({"PATH": "/synthetic"}, local_url), expected)
        command = execute.call_args.args[0]
        self.assertIn(local_url, command)
        self.assertIn("ON_ERROR_STOP=1", command)
        self.assertIn("count(*) FROM script_reviews", command[-1])
        self.assertIn("count(*) FROM script_revisions", command[-1])
        self.assertNotIn("attempts", command[-1])
        self.assertEqual(execute.call_args.kwargs["timeout"], 10)
        failed = subprocess.CompletedProcess([], 1, "", "database unavailable")
        with patch.object(runner.subprocess, "run", return_value=failed), \
                self.assertRaisesRegex(RuntimeError, "editor row-count oracle failed"):
            runner.editor_snapshot({}, local_url)

    def test_tool_session_is_only_in_child_environment_and_receipt_stays_structured(self):
        token = "a" * 64
        environment = {"PATH": "/synthetic/bin"}
        payload = {"run_id": "synthetic-run"}
        envelope = {"ok": True, "http_status": 200, "result": {"workflow": "caller"}}
        completed = subprocess.CompletedProcess([], 0, json.dumps(envelope), "")
        with patch.object(runner.subprocess, "run", return_value=completed) as execute:
            self.assertEqual(runner.adaptation_tool(environment, 12345, token, "review", payload, 200),
                             envelope)
        command = execute.call_args.args[0]
        options = execute.call_args.kwargs
        self.assertEqual(command, [sys.executable, str(runner.ADAPTATION_TOOL), "--base-url",
                                   "http://127.0.0.1:12345", "review"])
        self.assertNotIn(token, " ".join(command))
        self.assertEqual(options["env"]["CANTOS_SESSION_TOKEN"], token)
        self.assertNotIn("CANTOS_SESSION_TOKEN", environment)
        self.assertEqual(json.loads(options["input"]), payload)
        self.assertEqual(options["timeout"], 15)
        execute.assert_called_once()

    def test_tool_rejects_unstructured_or_inconsistent_receipts_without_printing_secrets(self):
        token = "a" * 64
        good = {"ok": True, "http_status": 200, "result": {"id": "synthetic"}}
        cases = [
            (0, "not JSON", "", "one JSON envelope"),
            (1, json.dumps(good), "", "unexpected status or envelope"),
            (0, json.dumps(dict(good, ok=False)), "", "unexpected status or envelope"),
            (0, json.dumps(dict(good, result=None)), "", "unexpected status or envelope"),
            (0, json.dumps(good), token, "output suppressed"),
            (0, json.dumps(dict(good, result={"secret": token})), "", "output suppressed"),
        ]
        for code, stdout, stderr, message in cases:
            with self.subTest(message=message, code=code):
                completed = subprocess.CompletedProcess([], code, stdout, stderr)
                with patch.object(runner.subprocess, "run", return_value=completed), \
                        self.assertRaisesRegex(RuntimeError, message) as failure:
                    runner.adaptation_tool({}, 12345, token, "context-read", {}, 200)
                self.assertNotIn(token, str(failure.exception))


if __name__ == "__main__":
    unittest.main()
