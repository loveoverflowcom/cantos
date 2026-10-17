"""Exercise the real VPS shell boundary with inert local command doubles.

The repository gate discovers these tests. No Podman, systemd or PostgreSQL process runs.
Real schema/grant semantics belong to the disposable PostgreSQL gate.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / "infra/vps/bin/cantos-vps").read_text().removesuffix('\nmain "$@"\n')
CURRENT = "a" * 40
PREVIOUS = "b" * 40
BACKUP_CHECKSUM = "0" * 64 + "  backup.dump\n" + "1" * 64 + "  backup.dump.schema-epoch\n"


class VpsSchemaTests(unittest.TestCase):
    def run_shell(self, body, *, files=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, content in (files or {}).items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content)
            env = dict(os.environ, CANTOS_VPS_ROOT=directory, CANTOS_TEST_EVENTS=str(root / "events"))
            doubles = """
event() { printf '%s\\n' "$*" >>"$CANTOS_TEST_EVENTS"; }
require_root() { :; }
with_lock() { :; }
unit_active() { return 0; }
check_revision() { event "revision $1"; }
podman() { event "podman $*"; printf '%s\\n' "${TEST_IMAGE_EPOCH:-2}"; }
check_schema() { event "check $1"; return "${TEST_SCHEMA_EXIT:-0}"; }
database_is_empty() { event empty-probe; return "${TEST_EMPTY_EXIT:-1}"; }
backup() { event backup; printf 'test.dump\\n'; }
migrate() { event migrate; }
apply_grants() { event grants; }
switch_to() { event "switch $1"; return 0; }
edge_probe() { event edge-probe; }
prune_releases() { event prune; }
systemctl() { event "systemctl $*"; }
"""
            result = subprocess.run(["bash"], input=SCRIPT + doubles + body,
                                    text=True, capture_output=True, env=env, check=False)
            events = (root / "events").read_text().splitlines() if (root / "events").exists() else []
            return result, events

    def test_populated_legacy_database_refuses_deploy_before_backup_or_migration(self):
        result, events = self.run_shell(f"TEST_SCHEMA_EXIT=1\ndeploy {CURRENT}\n")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("inventory and approved cutover required", result.stderr)
        self.assertFalse(any(event in ("backup", "migrate", "grants") or event.startswith("switch ")
                             for event in events), events)

    def test_current_schema_checks_target_and_previous_before_writes(self):
        result, events = self.run_shell(f"deploy {CURRENT}\n", files={"state/current": PREVIOUS + "\n"})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertLess(events.index(f"check {CURRENT}"), events.index("backup"))
        self.assertLess(events.index(f"check {PREVIOUS}"), events.index("backup"))
        self.assertLess(events.index("backup"), events.index("migrate"))
        self.assertLess(events.index("migrate"), events.index(f"switch {CURRENT}"))

    def test_empty_fresh_database_allows_bootstrap_without_current_schema_check(self):
        result, events = self.run_shell(f"TEST_EMPTY_EXIT=0\ndeploy {CURRENT}\n",
                                        files={"state/.keep": ""})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("migrate", events)
        self.assertFalse(any(event.startswith("check ") for event in events), events)

    def test_image_without_epoch_label_refuses_before_database_probe(self):
        result, events = self.run_shell(f"TEST_IMAGE_EPOCH='<no value>'\ndeploy {CURRENT}\n")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertNotIn("empty-probe", events)
        self.assertNotIn("backup", events)

    def test_rollback_refuses_old_image_before_switch(self):
        result, events = self.run_shell("TEST_IMAGE_EPOCH=1\nrollback\n",
                                       files={"state/current": CURRENT, "state/previous": PREVIOUS})
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertFalse(any(event.startswith("switch ") for event in events), events)

    def test_rollback_refuses_incompatible_current_schema_before_switch(self):
        result, events = self.run_shell("TEST_SCHEMA_EXIT=1\nrollback\n",
                                       files={"state/current": CURRENT, "state/previous": PREVIOUS})
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn(f"check {PREVIOUS}", events)
        self.assertFalse(any(event.startswith("switch ") for event in events), events)

    def test_historical_backup_without_epoch_metadata_refuses_before_any_write(self):
        result, events = self.run_shell('restore "$ROOT/backup.dump" --yes-replace-database\n',
                                       files={"backup.dump": "historical", "backup.dump.sha256": "historical"})
        self.assertEqual(result.returncode, 66, result.stderr)
        self.assertIn("reviewed legacy bundle", result.stderr)
        self.assertEqual(events, [])

    def test_foreign_epoch_backup_refuses_before_stopping_apis(self):
        body = """
sha256sum() { return 0; }
restore "$ROOT/backup.dump" --yes-replace-database
"""
        result, events = self.run_shell(body, files={"backup.dump": "historical",
                                                    "backup.dump.sha256": BACKUP_CHECKSUM,
                                                    "backup.dump.schema-epoch": "1\n"})
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertNotIn("backup", events)
        self.assertFalse(any(event.startswith("systemctl ") for event in events), events)

    def test_epoch_metadata_must_be_bound_to_the_backup_checksum(self):
        body = """
sha256sum() { return 0; }
restore "$ROOT/backup.dump" --yes-replace-database
"""
        result, events = self.run_shell(body, files={"backup.dump": "current",
                                                    "backup.dump.sha256": "0" * 64 + "  backup.dump\n",
                                                    "backup.dump.schema-epoch": "2\n"})
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("not covered by the backup checksum", result.stderr)
        self.assertEqual(events, [])

    def test_backup_preserves_dump_when_schema_metadata_is_unavailable(self):
        backup_function = SCRIPT[SCRIPT.index("backup() {"):SCRIPT.index("\nrestore() {")]
        body = backup_function + """
database_schema_epoch() { return 1; }
sha256sum() { event "checksum $*"; }
fixture_dump=$(backup test)
[[ -s $fixture_dump && $(<"$fixture_dump.schema-epoch") == unrecognized ]]
"""
        result, events = self.run_shell(body)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("preserving backup with unrecognized metadata", result.stderr)
        checksum = next(event for event in events if event.startswith("checksum "))
        self.assertIn(".dump.schema-epoch", checksum)

    def test_restored_schema_failure_keeps_apis_stopped_and_does_not_grant(self):
        body = """
sha256sum() { return 0; }
checks=0
check_schema() { checks=$((checks + 1)); event "check $checks"; [[ $checks -eq 1 ]]; }
restore "$ROOT/backup.dump" --yes-replace-database
"""
        result, events = self.run_shell(body, files={"backup.dump": "current",
                                                    "backup.dump.sha256": BACKUP_CHECKSUM,
                                                    "backup.dump.schema-epoch": "2\n",
                                                    "state/current": CURRENT})
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("APIs remain stopped", result.stderr)
        self.assertIn("systemctl stop cantos-api-theatre.service cantos-api-studio.service", events)
        self.assertNotIn("grants", events)
        self.assertFalse(any(event.startswith("systemctl start ") for event in events), events)


if __name__ == "__main__":
    unittest.main()
