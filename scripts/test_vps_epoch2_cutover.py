"""Fault-inject the production cutover helper with inert process/database boundaries.

These assertions exercise the real recovery/commit decisions. They are not PostgreSQL or
VPS acceptance; the separate restored-database rehearsal owns those claims.
"""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
HELPER = (ROOT / "infra/vps/bin/cantos-epoch2-cutover").read_text()
TARGET = "a" * 40
PREVIOUS = "883e188095227f04fb92cf4dcd29c659ac9de1fe"
JOURNAL = "aaaaaaaaaaaa-123-456"


class Epoch2CutoverTests(unittest.TestCase):
    def shell(self, body, *, phase="prepared", old_name="cantos", new_name="cantos_e2_stage_test", extra_env=None):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            journal = root / "state/epoch2-cutovers" / JOURNAL
            journal.mkdir(parents=True)
            (root / "state/epoch2-cutover.pending").write_text(JOURNAL + "\n")
            (root / "state/current").write_text(PREVIOUS + "\n")
            (journal / "phase").write_text(phase + "\n")
            (journal / "meta").write_text(
                f"target={TARGET}\nprevious={PREVIOUS}\nstaging=cantos_e2_stage_test\n"
                "retained=cantos_e1_retained_test\nold_oid=100\nnew_oid=200\nold_limit=-1\n")
            (root / "database-100").write_text(old_name)
            (root / "database-200").write_text(new_name)
            (journal / "rows-frozen.jsonl").write_text("preserved rows\n")
            incoming = root / "incoming" / TARGET
            (incoming / "tree").mkdir(parents=True)
            (incoming / "cutover-backup.tar.sha256").write_text("verified archive\n")
            (incoming / "cutover-offhost.sha256").write_text("verified archive\n")
            prelude = r'''
set -Eeuo pipefail
readonly ROOT=$FIXTURE_ROOT
readonly STATE=$ROOT/state CONFIG=$ROOT/config RUNTIME=$ROOT/runtime EDGE_DIR=$ROOT/edge
readonly QUADLET_DIR=$ROOT/quadlet SYSTEMD_DIR=$ROOT/systemd
readonly API_UNITS=(cantos-api-theatre cantos-api-studio)
event() { printf '%s\n' "$*" >>"$ROOT/events"; }
die() { printf '%s\n' "$1" >&2; exit "${2:-1}"; }
log() { event "log $*"; }
require_root() { :; }
with_lock() { :; }
config_value() { awk -F= -v key="$2" '$1==key {sub(/^[^=]*=/, ""); print; found=1} END {exit !found}' "$1"; }
'''
            doubles = r'''
systemctl() { event "systemctl $*"; }
epoch2_database_by_oid() { cat "$ROOT/database-$1"; }
epoch2_database_oid() {
  local oid
  for oid in 100 200; do [[ $(cat "$ROOT/database-$oid") != "$1" ]] || printf '%s\n' "$oid"; done
  return 0
}
epoch2_query() {
  event "sql $2"
  case "$2" in
    'ALTER DATABASE cantos RENAME TO cantos_e2_stage_test;') printf cantos_e2_stage_test >"$ROOT/database-200" ;;
    'ALTER DATABASE cantos_e1_retained_test RENAME TO cantos;') printf cantos >"$ROOT/database-100" ;;
    'ALTER DATABASE cantos RENAME TO cantos_e1_retained_test;') printf cantos_e1_retained_test >"$ROOT/database-100" ;;
    'ALTER DATABASE cantos_e2_stage_test RENAME TO cantos;') printf cantos >"$ROOT/database-200" ;;
  esac
}
epoch2_no_clients() { event "no-clients $1"; }
epoch2_restore_browser() { event provider-restore; }
epoch2_restore_host() { event host-restore; printf '%s\n' "$PREVIOUS" >"$STATE/current"; }
switch_to() { event "switch $1"; [[ ${FAIL_TARGET_HEALTH:-0} != 1 || $1 != "$TARGET" ]]; }
check_schema() { event "schema $1"; }
epoch2_guard_legacy() { event guard; }
epoch2_rows() { printf 'preserved rows\n' >"$2"; }
epoch2_reconcile_browser() { event provider-reconcile; }
install_bundle() { event install; }
validate_runtime_browser_configuration() { event browser-validation; }
apply_grants() { event grants; }
edge_probe() { event edge-probe; }
sha256sum() { [[ ${FAIL_ARCHIVE_HASH:-0} == 0 ]]; }
'''
            env = dict(os.environ, FIXTURE_ROOT=directory, TARGET=TARGET, PREVIOUS=PREVIOUS)
            env.update(extra_env or {})
            result = subprocess.run(["bash"], input=prelude + HELPER + doubles + body,
                                    capture_output=True, text=True, env=env, check=False)
            events = (root / "events").read_text().splitlines() if (root / "events").exists() else []
            return result, events, {
                "phase": (journal / "phase").read_text().strip(),
                "old_name": (root / "database-100").read_text(),
                "new_name": (root / "database-200").read_text(),
                "pending": (root / "state/epoch2-cutover.pending").exists(),
            }

    def test_health_failure_restores_database_provider_runtime_and_old_image_as_one_pair(self):
        result, events, state = self.shell(
            f'FAIL_TARGET_HEALTH=1\nepoch2_commit "$TARGET" "$ROOT/incoming/$TARGET/tree"\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(state["phase"], "rolled-back", result.stderr)
        self.assertEqual(state["old_name"], "cantos")
        self.assertEqual(state["new_name"], "cantos_e2_stage_test")
        self.assertLess(events.index("provider-restore"), events.index("host-restore"))
        self.assertIn(f"switch {PREVIOUS}", events)
        self.assertFalse(state["pending"])

    def test_crash_between_database_renames_recovers_by_recorded_oids(self):
        result, events, state = self.shell(
            'epoch2_recover "$TARGET" "$ROOT/incoming/$TARGET/tree"\n', phase="rename-intent",
            old_name="cantos_e1_retained_test")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state["old_name"], "cantos")
        self.assertEqual(state["phase"], "rolled-back")
        self.assertIn("sql ALTER DATABASE cantos_e1_retained_test RENAME TO cantos;", events)

    def test_accepted_crash_finalizes_without_restoring_legacy_database_or_identity(self):
        result, events, state = self.shell(
            'printf "%s\\n" "$TARGET" >"$STATE/current"\nepoch2_recover "$TARGET" "$ROOT/incoming/$TARGET/tree"\n',
            phase="accepted", old_name="cantos_e1_retained_test", new_name="cantos")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state["phase"], "complete")
        self.assertEqual(state["old_name"], "cantos_e1_retained_test")
        self.assertEqual(state["new_name"], "cantos")
        self.assertNotIn("provider-restore", events)
        self.assertNotIn("host-restore", events)
        self.assertFalse(any("RENAME" in event for event in events))

    def test_unknown_database_identity_keeps_apis_stopped_and_refuses_recovery(self):
        result, events, state = self.shell(
            'epoch2_recover "$TARGET" "$ROOT/incoming/$TARGET/tree"\n',
            phase="rename-intent", old_name="foreign")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertTrue(state["pending"])
        self.assertNotIn("provider-restore", events)
        self.assertFalse(any(event.startswith("switch ") for event in events))

    def test_missing_or_mismatched_offhost_receipt_never_installs_or_renames(self):
        result, events, state = self.shell(
            'printf mismatch >"$ROOT/incoming/$TARGET/cutover-offhost.sha256"\n'
            'epoch2_commit "$TARGET" "$ROOT/incoming/$TARGET/tree"\n')
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertNotIn("install", events)
        self.assertFalse(any("RENAME" in event for event in events))
        self.assertEqual(state["old_name"], "cantos")

    def test_checksum_failure_caller_recovery_resumes_the_old_api(self):
        result, events, state = self.shell(r'''
set +e
FAIL_ARCHIVE_HASH=1
epoch2_commit "$TARGET" "$ROOT/incoming/$TARGET/tree"
failure=$?
set -e
epoch2_recover "$TARGET" "$ROOT/incoming/$TARGET/tree"
exit "$failure"
''')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(state["phase"], "rolled-back", result.stderr)
        self.assertIn(f"switch {PREVIOUS}", events)
        self.assertNotIn("install", events)
        self.assertNotIn("provider-restore", events)
        self.assertFalse(state["pending"])

    def test_failed_provider_snapshot_does_not_attempt_to_restore_partial_provider_state(self):
        result, events, state = self.shell(r'''
rm "$STATE/epoch2-cutover.pending"
check_revision() { :; }
require_schema_image() { :; }
epoch2_snapshot_host() { event snapshot-host; }
epoch2_install_pending_guard() { event pending-guard; }
epoch2_snapshot_browser() { event provider-read-failed; return 70; }
epoch2_restore_browser() { event invalid-partial-provider-restore; return 71; }
epoch2_prepare "$TARGET" "$ROOT/incoming/$TARGET/tree"
''')
        self.assertEqual(result.returncode, 70, result.stderr)
        self.assertIn("provider-read-failed", events)
        self.assertNotIn("invalid-partial-provider-restore", events)
        self.assertIn(f"switch {PREVIOUS}", events)
        self.assertFalse(state["pending"])

    def test_ordinary_mutations_refuse_while_a_journal_is_pending(self):
        result, events, _ = self.shell('epoch2_refuse_pending\n')
        self.assertEqual(result.returncode, 75, result.stderr)
        self.assertEqual(events, [])

    def test_legacy_schema_mismatch_refuses_before_registry_or_data_import(self):
        actual_guard = HELPER[HELPER.index("epoch2_guard_legacy() {"):
                              HELPER.index("\nepoch2_dump()")]
        result, events, _ = self.shell(actual_guard + r'''
epoch2_schema() { event schema-dump; printf unknown >"$2"; }
epoch2_registry() { event registry; }
printf expected >"$STATE/epoch2-cutovers/aaaaaaaaaaaa-123-456/schema-oracle.sql"
epoch2_guard_legacy cantos "$STATE/epoch2-cutovers/aaaaaaaaaaaa-123-456" "$ROOT/incoming/$TARGET/tree"
''')
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("unknown objects/columns", result.stderr)
        self.assertNotIn("registry", events)

    def test_restored_schema_uses_an_independently_restored_oracle_without_rewriting_checks(self):
        fixture = json.loads((ROOT / "scripts/fixtures/epoch2-pg17-restore-checks.json").read_text())
        raw = "\n".join(row["fresh_source"] for row in fixture["constraints"])
        restored = "\n".join(row["restored_source"] for row in fixture["constraints"])
        self.assertNotEqual(raw, restored)
        actual_guard = HELPER[HELPER.index("epoch2_guard_legacy() {"):
                              HELPER.index("\nepoch2_dump()")]
        setup = r'''
directory=$STATE/epoch2-cutovers/aaaaaaaaaaaa-123-456
printf '%s' "$FRESH_SCHEMA" >"$directory/schema-oracle.sql"
printf '%s' "$RESTORED_SCHEMA" >"$directory/schema-oracle-restored.sql"
printf checked >"$directory/registry-expected.tsv"
mkdir -p "$ROOT/incoming/$TARGET/tree/infra/vps/postgres"
: >"$ROOT/incoming/$TARGET/tree/infra/vps/postgres/epoch2-empty-business-guard.sql"
epoch2_schema() { printf '%s' "$ACTUAL_SCHEMA" >"$2"; }
epoch2_registry() { printf checked >"$2"; }
psql_superuser() { event data-guard; cat >/dev/null; }
'''
        cases = (
            ("fresh-live", "cantos", "schema-oracle.sql", raw, 0),
            ("restored", "cantos_e1_restore_test", "schema-oracle-restored.sql", restored, 0),
            ("changed-check", "cantos_e1_restore_test", "schema-oracle-restored.sql",
             restored.replace("octet_length(issuer) <= 1024", "octet_length(issuer) <= 1023"), 65),
            ("no-text-normalizer", "cantos_e1_restore_test", "schema-oracle.sql", restored, 65),
            ("never-normalize-live", "cantos", "schema-oracle-restored.sql", restored, 65),
        )
        for name, database, expected_schema, observed, status in cases:
            with self.subTest(name=name):
                body = (actual_guard + setup +
                        f'epoch2_guard_legacy {database} "$directory" "$ROOT/incoming/$TARGET/tree" {expected_schema}\n')
                result, events, _ = self.shell(body, extra_env={
                    "FRESH_SCHEMA": raw, "RESTORED_SCHEMA": restored, "ACTUAL_SCHEMA": observed})
                self.assertEqual(result.returncode, status, result.stderr)
                self.assertEqual("data-guard" in events, status == 0)

    def test_transfer_never_disables_constraints_or_triggers(self):
        self.assertNotIn("session_replication_role", HELPER)
        self.assertNotIn("DISABLE TRIGGER", HELPER)
        self.assertNotIn("DROP DATABASE", HELPER)

    def test_failed_host_archive_restore_retains_pending_and_never_switches_old_image(self):
        actual_restore = HELPER[HELPER.index("epoch2_restore_host() {"):
                                HELPER.index("\nepoch2_database_oid()")]
        result, events, state = self.shell(actual_restore + r'''
tar() { event archive-restore-failed; return 70; }
epoch2_recover "$TARGET" "$ROOT/incoming/$TARGET/tree"
''', phase="health-check", old_name="cantos_e1_retained_test", new_name="cantos")
        self.assertEqual(result.returncode, 70, result.stderr)
        self.assertTrue(state["pending"])
        self.assertEqual(state["phase"], "health-check")
        self.assertIn("archive-restore-failed", events)
        self.assertFalse(any(event.startswith("switch ") for event in events))


if __name__ == "__main__":
    unittest.main()
