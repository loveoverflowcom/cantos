"""Ownership receipt guards; subprocess doubles establish no PostgreSQL acceptance."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import retained_postgres as retained


class RetainedOwnershipTests(unittest.TestCase):
    def fixture(self, root, scope="schema"):
        cluster = root / scope / "cluster-123" if scope != "full" else root / "cluster-123"
        (cluster / "pgdata").mkdir(parents=True)
        (cluster / "pgdata" / "PG_VERSION").write_text("18\n")
        return {"format": 1, "purpose": "retained-disposable-cluster", "scope": scope,
                "cluster": str(cluster), "port": 54321, "retained": True,
                "runner_pid": 99999999, "runner_identity": None, "runner_active": False,
                "gate_status": "FAIL", "full_gate_status": "NOT_RUN"}

    def test_failed_narrow_receipt_proves_only_owned_path_not_full_acceptance(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            record = self.fixture(evidence)
            self.assertEqual(retained.validate_ownership(record, evidence),
                             (54321, Path(record["cluster"]) / "pgdata"))
            self.assertEqual(record["gate_status"], "FAIL")
            self.assertEqual(record["full_gate_status"], "NOT_RUN")
            full=self.fixture(evidence,"full")
            full["full_gate_status"]="FAIL"
            self.assertEqual(retained.validate_ownership(full,evidence),(54321,Path(full["cluster"])/"pgdata"))

    def test_narrow_outcome_cannot_claim_full_pass_or_boolean_format(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence=Path(directory)
            narrow=self.fixture(evidence)
            for record in (dict(narrow,full_gate_status="PASS"),dict(narrow,full_gate_status="FAIL"),dict(narrow,format=True)):
                with self.subTest(record=record),self.assertRaises(ValueError):
                    retained.validate_ownership(record,evidence)
            full=self.fixture(evidence,"full")
            with self.assertRaisesRegex(ValueError,"outcomes are inconsistent"):
                retained.validate_ownership(dict(full,gate_status="FAIL",full_gate_status="PASS"),evidence)

    def test_active_flag_or_matching_live_pid_refuses_before_connection(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            record = self.fixture(evidence)
            for active in (dict(record, runner_active=True),
                           dict(record, runner_pid=os.getpid(), runner_identity=retained.process_identity(os.getpid()))):
                with self.subTest(record=active), patch.object(subprocess,"run") as execute, self.assertRaisesRegex(ValueError,"still active"):
                    retained.confirm_running(active,evidence,{})
                execute.assert_not_called()

    def test_scope_directory_symlink_port_and_unowned_record_are_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence = Path(directory)
            record = self.fixture(evidence)
            invalid = (dict(record, scope="full"), dict(record, scope="unregistered"),
                       dict(record, retained=False), dict(record, port=True), dict(record, port=80),
                       dict(record, purpose="full-verification-pass"), dict(record, cluster=str(evidence / "elsewhere")))
            for value in invalid:
                with self.subTest(value=value), self.assertRaises(ValueError):
                    retained.validate_ownership(value,evidence)
            link=evidence / "schema" / "cluster-456"
            link.symlink_to(Path(record["cluster"]),target_is_directory=True)
            with self.assertRaisesRegex(ValueError,"scope/directory"):
                retained.validate_ownership(dict(record,cluster=str(link)),evidence)

    def test_stopped_or_swapped_cluster_refuses_and_inherited_libpq_cannot_redirect(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence=Path(directory)
            record=self.fixture(evidence)
            for result,message in ((subprocess.CompletedProcess([],2,"","stopped"),"stopped"),
                                   (subprocess.CompletedProcess([],0,"/another/data\n",""),"another data directory")):
                with patch.object(subprocess,"run",return_value=result) as execute, self.assertRaisesRegex(ValueError,message):
                    retained.confirm_running(record,evidence,{"PATH":"/bin","PGHOST":"elsewhere","DATABASE_URL":"private","CANTOS_TEST_CLUSTER_URL":"other"})
                self.assertEqual(execute.call_args.kwargs["env"],{"PATH":"/bin"})
                self.assertIn("postgresql://cantos_test_admin@127.0.0.1:54321/postgres",execute.call_args.args[0])

    def test_runner_write_contains_no_fixture_urls_and_separates_scope_status(self):
        with tempfile.TemporaryDirectory() as directory:
            evidence=Path(directory)
            record=self.fixture(evidence)
            path=evidence/"retained-cluster.json"
            retained.write_ownership(path,Path(record["cluster"]),54321,"schema",False,"FAIL")
            actual=json.loads(path.read_text())
            self.assertEqual((actual["gate_status"],actual["full_gate_status"]),("FAIL","NOT_RUN"))
            self.assertEqual(actual["runner_pid"],os.getpid())
            self.assertFalse(actual["runner_active"])
            self.assertFalse(any("url" in name for name in actual))
            self.assertEqual(path.stat().st_mode & 0o777,0o600)
