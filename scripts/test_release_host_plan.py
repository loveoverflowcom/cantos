"""Offline host plans exercise the production pure decision, never a real host."""
import copy
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_oci as oci
from test_release_oci import fixture


def inventory(manifest):
    return {"schema_version": 1, "host_id": "fixture-host", "observed_at": 1000,
            "state": "NATIVE_READY", "platform": "linux/amd64", "cgroup_version": 2,
            "journal": "IDLE", "lock": "AVAILABLE", "current_manifest_sha256": "a" * 64,
            "previous_manifest_sha256": "b" * 64, "compatibility": manifest["compatibility"],
            "socket_directory": "/run/cantos/identity-rpc",
            "services": {service: {"runtime_mode": "rootful", "principal": "root",
                                   "image_store": "existing-root-store", "quadlet_scope": "system",
                                   "database_directory": "/var/lib/" + service}
                         for service in oci.SERVICES}}


class HostPlan(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.source, self.args, self.manifest, _ = fixture(Path(self.temporary.name))
        self.inventory = inventory(self.manifest)

    def plan(self, **changes):
        return oci.host_plan({**self.inventory, **changes}, self.manifest, now=1001)

    def test_exact_pair_preserves_declared_owners_but_never_grants_activation(self):
        result = self.plan()
        self.assertEqual(result["readiness"], "REVIEW_REQUIRED")
        self.assertEqual(result["current_manifest_sha256"], "a" * 64)
        self.assertEqual(result["previous_manifest_sha256"], "b" * 64)
        self.assertFalse(result["release_eligible"])
        self.assertEqual(result["activation"], "NOT_RUN")
        self.assertEqual(result["cleanup"], "NOT_RUN")
        self.assertEqual({s: result["services"][s]["image"] for s in oci.SERVICES},
                         {s: self.manifest["images"][s]["ref"] for s in oci.SERVICES})

    def test_unknown_legacy_and_new_require_distinct_operator_work(self):
        for state, reason in (("UNKNOWN", "HOST_UNKNOWN"),
                              ("LEGACY", "EXPLICIT_CUTOVER_AND_NATIVE_RECOVERY_ACCEPTANCE_REQUIRED"),
                              ("NEW", "EXPLICIT_BOOTSTRAP_AND_DATA_TIER_APPROVAL_REQUIRED")):
            result = self.plan(state=state)
            self.assertEqual(result["readiness"], "BLOCKED")
            self.assertIn(reason, result["blockers"])

    def test_platform_schema_cgroup_and_interrupted_operation_block_independently(self):
        for change, reason in (({"platform": "linux/arm64"}, "PLATFORM_MISMATCH"),
                               ({"compatibility": {}}, "SCHEMA_RPC_SECURITY_COMPATIBILITY_NOT_CONFIRMED"),
                               ({"cgroup_version": True}, "CGROUP_V2_REQUIRED"),
                               ({"journal": "RECOVERY_REQUIRED"}, "RECONCILE_OPERATION_BEFORE_PREPARATION"),
                               ({"lock": "HELD"}, "RECONCILE_OPERATION_BEFORE_PREPARATION"),
                               ({"current_manifest_sha256": None}, "CURRENT_RELEASE_UNKNOWN")):
            self.assertIn(reason, self.plan(**change)["blockers"])

    def test_unsafe_or_shared_database_socket_and_runtime_ownership_are_refused(self):
        for path in ("/var/lib/cantos-server", "/var/lib", "/run/cantos/identity-rpc/db", "/var/../private", "/"):
            value = copy.deepcopy(self.inventory)
            value["services"]["cantos-identity"]["database_directory"] = path
            with self.subTest(path=path), self.assertRaisesRegex(ValueError, "directory|directories"):
                oci.host_plan(value, self.manifest, now=1001)
        value = copy.deepcopy(self.inventory)
        item = value["services"]["cantos-server"]
        item.update(runtime_mode="rootless", principal="cantos", quadlet_scope="user")
        self.assertEqual(oci.host_plan(value, self.manifest, now=1001)["services"]["cantos-server"],
                         {**item, "image": self.manifest["images"]["cantos-server"]["ref"]})
        item["quadlet_scope"] = "system"
        with self.assertRaisesRegex(ValueError, "scope mismatch"):
            oci.host_plan(value, self.manifest, now=1001)

    def test_expired_future_or_malformed_inventory_refuses(self):
        for change in ({"observed_at": -100000}, {"observed_at": 2000}, {"observed_at": True},
                       {"state": "READY"}, {"host_id": "private\nvalue"}, {"unknown": True}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                self.plan(**change)

    def test_pinned_inventory_and_qualification_records_precede_any_registry_effect(self):
        self.args.host_inventory = Path(self.temporary.name) / "inventory.json"
        contract.write_new(self.args.host_inventory, self.inventory)
        self.args.host_inventory_digest = contract.file_record(self.args.host_inventory, self.args.host_inventory.parent)["sha256"]
        self.args.dry_run = True
        self.args.registry_readback = True
        self.args.authfile = Path("never-read-auth")
        with patch("release_oci_registry.observe", side_effect=AssertionError("unexpected network/auth")), patch.object(oci.time, "time", return_value=1001):
            self.assertEqual(oci.execute(self.source, self.args)["host_plan"]["readiness"], "REVIEW_REQUIRED")
            self.args.host_inventory_digest = "0" * 64
            with self.assertRaisesRegex(ValueError, "manifest digest mismatch"):
                oci.execute(self.source, self.args)
            self.args.host_inventory = None
            (self.args.manifest.parent / "gate-records/native-e2e.json").unlink()
            with self.assertRaises(FileNotFoundError):
                oci.execute(self.source, self.args)

    def test_rehashed_but_foreign_qualification_record_is_not_admitted(self):
        evidence_path = self.args.manifest.parent / "evidence.json"
        evidence = contract.read_json(evidence_path)
        record_path = evidence_path.parent / "gate-records/image-pair.json"
        record = contract.read_json(record_path)
        record["images"]["cantos-identity"]["ref"] = "ghcr.io/loveoverflowcom/cantos-identity@sha256:" + "f" * 64
        record_path.write_bytes(contract.canonical(record))
        evidence["gates"]["image-pair"]["record_sha256"] = contract.file_record(record_path, record_path.parent)["sha256"]
        evidence_path.write_bytes(contract.canonical(evidence))
        self.manifest["evidence"] = contract.file_record(evidence_path, evidence_path.parent)
        with self.assertRaisesRegex(ValueError, "qualification record release identity mismatch: image-pair"):
            oci.verify_evidence(evidence_path, self.manifest)


if __name__ == "__main__":
    unittest.main()
