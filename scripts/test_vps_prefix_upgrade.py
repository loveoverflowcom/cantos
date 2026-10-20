"""Synthetic boundary/fault tests; no PostgreSQL, SSH, provider or container is contacted."""
import os
import hashlib
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
HOST = (ROOT / 'infra/vps/bin/cantos-vps').read_text().removesuffix('\nmain "$@"\n')
COMMON = (ROOT / 'infra/vps/bin/cantos-epoch2-cutover').read_text()
PREFIX = (ROOT / 'infra/vps/bin/cantos-prefix-upgrade').read_text()
TARGET, OLD, ATTEMPT = 'a' * 40, 'b' * 40, '1' * 32
CHECKSUMS = [f'{n:x}' * 64 for n in (1, 2, 3)]
STAGING, RETAINED = f'cantos_e2_prefix_{ATTEMPT}', f'cantos_e2_retained_{ATTEMPT}'


class PrefixUpgradeTests(unittest.TestCase):
    def shell(self, body, *, phase='prepared', old_name='cantos', new_name=STAGING,
              real_prefix=False, real_lock=False, target_rows=2, source_rows=1, extra_env=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = root / 'state'
            state.mkdir()
            journal = state / 'epoch2-prefix-upgrades' / f'{TARGET[:12]}-{ATTEMPT}'
            incoming = root / 'incoming' / TARGET / ATTEMPT
            tree = incoming / 'tree'
            (tree / 'infra/vps/postgres').mkdir(parents=True)
            (tree / 'infra/vps/postgres/epoch2-prefix-registry.tsv').write_text(
                ''.join(f'{n + 1}|{CHECKSUMS[n]}\n' for n in range(target_rows)))
            (tree / 'REVISION').write_text(TARGET + '\n')
            (state / 'current').write_text(OLD + '\n')
            if phase is not None:
                journal.mkdir(parents=True)
                (state / 'epoch2-prefix.pending').write_text(journal.name + '\n')
                (journal / 'phase').write_text(phase + '\n')
                (journal / 'meta').write_text(
                    f'target={TARGET}\nattempt={ATTEMPT}\nprevious={OLD}\n'
                    f'staging={STAGING}\nretained={RETAINED}\nold_oid=100\nnew_oid=200\nold_limit=-1\n')
                for name in ['rows-frozen.tsv', 'schema-target.sql']:
                    (journal / name).write_text('synthetic\n')
                rows = [f'{n + 1}|{CHECKSUMS[n]}\n' for n in range(target_rows)]
                (journal / 'registry-full.tsv').write_text(''.join(rows))
                (journal / 'registry-prefix.tsv').write_text(''.join(rows[:source_rows]))
                (journal / 'installed-host').write_text('synthetic host\n')
                (journal / 'edge-was-active').touch()
                if phase not in ['initializing', 'snapshotted', 'refused', 'rolled-back']:
                    (root / 'edge-stopped').touch()
                archive = b'synthetic archive\n'
                (incoming / 'cutover-backup.tar').write_bytes(archive)
                manifest = hashlib.sha256(archive).hexdigest() + '  cutover-backup.tar\n'
                for name in ['cutover-backup.tar.sha256', 'cutover-offhost.sha256']:
                    (incoming / name).write_text(manifest)
            (root / 'database-100').write_text(old_name)
            (root / 'database-200').write_text(new_name)
            host = HOST.replace('readonly QUADLET_DIR=/etc/containers/systemd', 'readonly QUADLET_DIR=$ROOT/quadlet')
            host = host.replace('readonly SYSTEMD_DIR=/etc/systemd/system', 'readonly SYSTEMD_DIR=$ROOT/systemd')
            host = host.replace('readonly LOCK=/run/lock/cantos-vps.lock', 'readonly LOCK=$ROOT/operation.lock')
            doubles = r'''
event() { printf '%s\n' "$*" >>"$ROOT/events"; }
log() { event "log $*"; }
require_root() { :; }
sync() { if [[ ${2:-} == */phase ]]; then event "durable $(<"$2")"; fi; }
flock() { return 0; }
systemctl() {
  event "systemctl $*"
  case "$*" in
    'stop cantos-edge.service') touch "$ROOT/edge-stopped" ;;
    'start cantos-edge.service') rm -f "$ROOT/edge-stopped" ;;
  esac
}
install() { event "install-file $*"; if [[ $1 == -d ]]; then mkdir -p "${@: -1}"; fi; }
podman() { if [[ $* == *'.Os'* ]]; then printf 'linux/amd64\n'; else printf 'old-image\n'; fi; }
unit_active() { [[ $1 != cantos-edge || ! -e $ROOT/edge-stopped ]]; }
check_revision() { event "revision $1"; }
require_schema_image() { event "epoch $1"; }
check_schema() { event "check $1"; }
switch_to() { event "switch $1"; [[ ${FAIL_HEALTH:-0} != 1 || $1 != "$TARGET" ]]; }
apply_grants() { event grants; }
edge_probe() { event edge-probe; [[ ${FAIL_EDGE:-0} != 1 ]]; }
validate_runtime_browser_configuration() { event browser-validation; }
install_bundle() { event install-bundle; [[ ${FAIL_INSTALL:-0} != 1 ]]; }
epoch2_snapshot_host() { event snapshot-host; printf old-host >"$1/installed-host"; touch "$1/edge-was-active"; }
epoch2_restore_host() { event restore-host; printf '%s\n' "$OLD" >"$STATE/current"; }
epoch2_database_by_oid() { cat "$ROOT/database-$1"; }
epoch2_database_oid() {
  local oid
  for oid in 100 200; do [[ $(cat "$ROOT/database-$oid") != "$1" ]] || printf '%s\n' "$oid"; done
  return 0
}
epoch2_no_clients() { event "no-clients $1"; [[ ${FOREIGN_CLIENT:-0} != 1 ]]; }
epoch2_query() {
  event "sql $2"
  case "$2" in
    "ALTER DATABASE cantos RENAME TO $RETAINED;") printf '%s' "$RETAINED" >"$ROOT/database-100" ;;
    "ALTER DATABASE $STAGING RENAME TO cantos;") printf cantos >"$ROOT/database-200" ;;
    "ALTER DATABASE cantos RENAME TO $STAGING;") printf '%s' "$STAGING" >"$ROOT/database-200" ;;
    "ALTER DATABASE $RETAINED RENAME TO cantos;") printf cantos >"$ROOT/database-100" ;;
    *'datconnlimit'*) printf '%s\n' -1 ;;
    *'SELECT tablename'*) printf 'actors\ncantos_schema\n' ;;
    *"c.relkind='S'"*) printf 'synthetic_seq\n' ;;
  esac
}
epoch2_create_database() { event "create-db $1"; }
epoch2_migrate_database() { event "migrator $*"; [[ $1 != "$TARGET" || ${FAIL_MIGRATION:-0} != 1 ]]; }
epoch2_schema() { printf 'schema\n' >"$2"; }
epoch2_registry() {
  case $1 in *full_*|*prefix_*) cat "$TREE/infra/vps/postgres/epoch2-prefix-registry.tsv" ;;
    *) if ((SOURCE_ROWS > 0)); then head -n "$SOURCE_ROWS" "$TREE/infra/vps/postgres/epoch2-prefix-registry.tsv"; fi ;; esac >"$2"
}
epoch2_dump() { event "dump $1"; printf 'synthetic dump\n' >"$2"; }
epoch2_restore_dump() { event "restore $1"; }
'''
            if not real_lock:
                doubles += 'with_lock() { event "lock ${1:-ordinary}"; }\n'
            if not real_prefix:
                doubles += r'''
prefix_infra_guard() { event infrastructure-guard; }
prefix_objects_guard() { event "objects-guard $1"; }
prefix_install_guard() { event pending-wrapper; }
prefix_fingerprint() {
  event "fingerprint $1"
  if [[ ${DRIFT_SOURCE:-0} == 1 && $1 == cantos ]]; then printf drift; else printf 'synthetic\n'; fi >"$3"
}
prefix_source_guard() { event "source-guard $1"; }
prefix_target_guard() { event "target-guard $1"; }
'''
            env = dict(os.environ, CANTOS_VPS_ROOT=str(root), TARGET=TARGET, OLD=OLD,
                       ATTEMPT=ATTEMPT, STAGING=STAGING, RETAINED=RETAINED,
                       JOURNAL=str(journal), TREE=str(tree), SOURCE_ROWS=str(source_rows),
                       **(extra_env or {}))
            result = subprocess.run(['bash'], input=host + COMMON + PREFIX + doubles + body,
                                    text=True, capture_output=True, env=env, timeout=15)
            events = (root / 'events').read_text().splitlines() if (root / 'events').exists() else []
            state = {'pending': (state / 'epoch2-prefix.pending').exists(),
                     'phase': (journal / 'phase').read_text().strip() if (journal / 'phase').exists() else None,
                     'old': (root / 'database-100').read_text(), 'new': (root / 'database-200').read_text()}
            return result, events, state

    def test_all_phases_have_a_closed_recovery_decision(self):
        for phase, action in [('initializing', 'clear'), ('refused', 'clear'), ('rolled-back', 'clear'),
                              ('snapshotted', 'rollback'), ('freeze-intent', 'rollback'), ('prepared', 'rollback'),
                              ('install-intent', 'rollback'), ('rename-intent', 'rollback'),
                              ('health-check', 'rollback'),
                              ('accepted', 'finalize'), ('complete', 'finalize')]:
            with self.subTest(phase=phase):
                result, _, _ = self.shell(f'prefix_action {phase}\n')
                self.assertEqual((result.returncode, result.stdout.strip()), (0, action))
        result, _, _ = self.shell('prefix_action unrecognized\n')
        self.assertEqual(result.returncode, 65)

    def test_health_failure_recovers_the_database_image_runtime_pair(self):
        result, events, state = self.shell('FAIL_HEALTH=1\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(state, {'pending': False, 'phase': 'rolled-back', 'old': 'cantos', 'new': STAGING})
        self.assertIn('restore-host', events)
        self.assertIn(f'switch {OLD}', events)
        self.assertFalse(any('DROP' in event or 'provider' in event for event in events))

    def test_acceptance_is_durable_before_writes_and_accepted_retry_never_rolls_back(self):
        result, events, state = self.shell('prefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state['phase'], 'complete')
        enable = 'sql ALTER ROLE cantos_app IN DATABASE cantos RESET default_transaction_read_only;'
        self.assertLess(events.index('durable accepted'), events.index(enable))
        result, events, state = self.shell(
            'printf "%s\\n" "$TARGET" >"$STATE/current"\nprefix_recover "$TARGET" "$TREE" "$ATTEMPT"\n',
            phase='accepted', old_name=RETAINED, new_name='cantos')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state['old'], RETAINED)
        self.assertEqual(state['new'], 'cantos')
        self.assertFalse(any('RENAME' in event or 'restore-host' in event or event.startswith('switch ') for event in events))

    def test_crash_between_renames_recovers_from_recorded_oids(self):
        result, events, state = self.shell('prefix_recover "$TARGET" "$TREE" "$ATTEMPT"\n',
                                           phase='rename-intent', old_name=RETAINED)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state['old'], 'cantos')
        self.assertEqual(state['phase'], 'rolled-back')

    def test_unknown_database_or_phase_keeps_pending_and_never_switches(self):
        for phase, old in [('rename-intent', 'foreign'), ('unknown', 'cantos')]:
            with self.subTest(phase=phase):
                result, events, state = self.shell('prefix_recover "$TARGET" "$TREE" "$ATTEMPT"\n',
                                                   phase=phase, old_name=old)
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertTrue(state['pending'])
                self.assertFalse(any('RENAME' in event or event.startswith('switch ') for event in events))

    def test_wrong_attempt_or_duplicate_metadata_never_selects_a_journal(self):
        for body, expected in [
                ('prefix_recover "$TARGET" "$TREE" "22222222222222222222222222222222"\n', 75),
                ('printf "old_oid=999\\n" >>"$JOURNAL/meta"\nprefix_recover "$TARGET" "$TREE" "$ATTEMPT"\n', 65)]:
            result, events, state = self.shell(body)
            self.assertEqual(result.returncode, expected, result.stderr)
            self.assertTrue(state['pending'])
            self.assertFalse(any(event.startswith('systemctl') for event in events))

    def test_missing_offhost_ack_refuses_before_install_or_rename(self):
        result, events, state = self.shell('rm "${TREE%/tree}/cutover-offhost.sha256"\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertEqual(result.returncode, 65)
        self.assertTrue(state['pending'])
        self.assertNotIn('install-bundle', events)
        self.assertFalse(any('RENAME' in event for event in events))

    def test_partial_install_and_frozen_source_drift_both_recover_before_acceptance(self):
        for failure in ['FAIL_INSTALL=1', 'DRIFT_SOURCE=1']:
            result, events, state = self.shell(failure + '\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(state['phase'], 'rolled-back', result.stderr)
            self.assertEqual(state['old'], 'cantos')
            self.assertIn(f'switch {OLD}', events)

    def test_prepare_only_migrates_siblings_and_verifies_the_final_frozen_restore(self):
        result, events, state = self.shell('prefix_prepare "$TARGET" "$TREE" "$ATTEMPT"\n', phase=None)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state['phase'], 'prepared')
        self.assertTrue(state['pending'])
        self.assertFalse(any(event.startswith(f'migrator {TARGET} cantos ') or event == f'migrator {TARGET} cantos' for event in events))
        self.assertLess(events.index('systemctl stop cantos-api-theatre.service cantos-api-studio.service'), events.index('dump cantos'))
        self.assertIn(f'restore {STAGING}', events)
        self.assertIn(f'migrator {OLD} {STAGING} --check', events)
        self.assertIn(f'migrator {TARGET} {STAGING}', events)

    def test_pending_state_blocks_ordinary_mutations_and_other_cutover_under_lock(self):
        for body in ['main install "$TREE"\n', 'with_lock cutover\n']:
            result, _, state = self.shell(body, real_lock=True)
            self.assertEqual(result.returncode, 75, result.stderr)
            self.assertTrue(state['pending'])

    def test_any_retained_prefix_journal_skips_all_release_pruning(self):
        result, events, _ = self.shell('prune_releases\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any('skipping all release pruning' in event for event in events))

    def test_edge_stays_closed_through_private_health_until_durable_acceptance(self):
        result, events, state = self.shell('prefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertLess(events.index('durable accepted'), events.index('systemctl start cantos-edge.service'))
        self.assertEqual(state['phase'], 'complete')
        result, events, state = self.shell('rm "$ROOT/edge-stopped"\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertNotIn('install-bundle', events)
        self.assertEqual(state['old'], 'cantos')

    def test_edge_failure_after_acceptance_retains_new_database_and_pending_recovery(self):
        result, events, state = self.shell('FAIL_EDGE=1\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(state['phase'], 'accepted')
        self.assertTrue(state['pending'])
        self.assertEqual(state['old'], RETAINED)
        self.assertEqual(state['new'], 'cantos')
        self.assertNotIn(f'switch {OLD}', events)
        self.assertNotIn('restore-host', events)

    def test_prepared_target_recheck_opens_only_owner_then_refreezes_before_install(self):
        body = r'''
original_query=$(declare -f epoch2_query)
eval "${original_query/epoch2_query/original_epoch2_query}"
epoch2_query() {
  original_epoch2_query "$@"
  case "$2" in
    *'REVOKE CONNECT'*'CONNECTION LIMIT 1;'*) touch "$ROOT/owner-open" ;;
    "ALTER DATABASE $STAGING CONNECTION LIMIT 0;") rm -f "$ROOT/owner-open" ;;
  esac
}
prefix_target_guard() { [[ -f $ROOT/owner-open ]]; event owner-rechecked; }
prefix_commit "$TARGET" "$TREE" "$ATTEMPT"
'''
        result, events, state = self.shell(body)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(state['phase'], 'complete')
        freeze = f'sql ALTER DATABASE {STAGING} CONNECTION LIMIT 0;'
        self.assertLess(events.index('owner-rechecked'), events.index(freeze))
        self.assertLess(events.index(freeze), events.index('install-bundle'))
        self.assertIn('sql GRANT CONNECT ON DATABASE cantos TO cantos_app;', events)

    def test_registry_contract_is_bound_to_every_real_migration_file(self):
        import hashlib
        schema = ROOT / 'services/cantos-api/schema'
        files = sorted(schema.glob('[0-9][0-9][0-9][0-9]_*.sql'))
        # The migrator numbers versions by position: a gap or reorder would be a different registry.
        self.assertEqual([int(path.name[:4]) for path in files], list(range(1, len(files) + 1)))
        rows = (ROOT / 'infra/vps/postgres/epoch2-prefix-registry.tsv').read_text().splitlines()
        self.assertEqual(rows, [f'{number}|{hashlib.sha256(path.read_bytes()).hexdigest()}'
                               for number, path in enumerate(files, 1)])

    def test_source_prefix_is_the_live_registry_and_must_be_exact_and_proper(self):
        select = 'prefix_select_registries "$TREE" "$JOURNAL"\nawk "END {print NR}" "$JOURNAL/registry-prefix.tsv"\n'
        for source, target in [(1, 2), (1, 3), (2, 3)]:
            with self.subTest(source=source, target=target):
                result, events, _ = self.shell(select, target_rows=target, source_rows=source)
                self.assertEqual((result.returncode, result.stdout.strip()), (0, str(source)), result.stderr)
        for source, target in [(2, 2), (3, 2), (0, 3)]:
            with self.subTest(source=source, target=target):
                result, _, _ = self.shell(select, target_rows=target, source_rows=source)
                self.assertEqual(result.returncode, 65, result.stderr)
        edited = ('epoch2_registry() { { head -n 1 "$TREE/infra/vps/postgres/epoch2-prefix-registry.tsv";'
                  ' printf "2|%s\\n" ' + '0' * 64 + '; } >"$2"; }\n')
        result, _, _ = self.shell(edited + select, target_rows=3, source_rows=2)
        self.assertEqual(result.returncode, 65, result.stderr)
        for registry in [f'1|{CHECKSUMS[0]}\n3|{CHECKSUMS[2]}\n', f'1|{CHECKSUMS[0]}\n2|short\n',
                         f'0|{CHECKSUMS[0]}\n1|{CHECKSUMS[1]}\n', f'1|{CHECKSUMS[0]}\n']:
            with self.subTest(registry=registry):
                body = f'printf %b {registry!r} >"$TREE/infra/vps/postgres/epoch2-prefix-registry.tsv"\n' + select
                result, _, _ = self.shell(body, source_rows=1)
                self.assertEqual(result.returncode, 65, result.stderr)

    def test_prepare_upgrades_a_bearer_registry_to_the_three_migration_target(self):
        result, events, state = self.shell(
            'prefix_prepare "$TARGET" "$TREE" "$ATTEMPT"\nawk "END {print NR}" "$JOURNAL/registry-prefix.tsv"\n',
            phase=None, target_rows=3, source_rows=2)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), '2')
        self.assertEqual(state['phase'], 'prepared')
        # The source stays at the previous image's [1,2]; only the restored sibling earns version 3.
        self.assertIn(f'migrator {OLD} {STAGING} --check', events)
        self.assertIn(f'migrator {TARGET} {STAGING}', events)
        self.assertFalse(any(event == f'migrator {TARGET} cantos' for event in events))
        self.assertLess(events.index('systemctl stop cantos-api-theatre.service cantos-api-studio.service'),
                        events.index('dump cantos'))

    def test_registry_refusals_happen_before_any_freeze_or_database_change(self):
        for source, target in [(2, 2), (3, 2)]:
            with self.subTest(source=source, target=target):
                result, events, state = self.shell('prefix_prepare "$TARGET" "$TREE" "$ATTEMPT"\n',
                                                   phase=None, target_rows=target, source_rows=source)
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertFalse(state['pending'])
                self.assertFalse(any(event.startswith('systemctl stop') or event.startswith('create-db')
                                     or event.startswith('dump ') for event in events))

    def test_every_table_added_by_the_suffix_must_be_empty_before_acceptance(self):
        body = r'''
printf 'actors\nbearer_tokens\ncantos_schema\n' >"$JOURNAL/baseline-tables"
epoch2_query() {
  event "sql $2"
  case "$2" in
    *'SELECT tablename'*) printf 'actors\nbearer_tokens\ncantos_schema\nlistener_bookmarks\n' ;;
    *'public."listener_bookmarks"'*) printf '%s\n' "$BOOKMARKS_EMPTY" ;;
    *) return 1 ;;
  esac
}
prefix_new_tables_empty cantos "$JOURNAL"
'''
        result, events, _ = self.shell(body, extra_env={'BOOKMARKS_EMPTY': 't'})
        self.assertEqual(result.returncode, 0, result.stderr)
        # Baseline tables (including Bearer on a [1,2] source) are fingerprinted, not required empty.
        self.assertFalse(any('bearer_tokens"' in event for event in events))
        self.assertTrue(any('public."listener_bookmarks"' in event for event in events))
        result, _, _ = self.shell(body, extra_env={'BOOKMARKS_EMPTY': 'f'})
        self.assertEqual(result.returncode, 65, result.stderr)

    def test_acceptance_retires_the_image_only_rollback_and_records_the_registry_step(self):
        result, events, state = self.shell(
            'echo previous >"$STATE/previous"\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n'
            '[[ ! -e $STATE/previous ]] || exit 1\nrollback\n', target_rows=3, source_rows=2)
        # No `previous` record remains, so `cantos-vps rollback` has nothing to switch to.
        self.assertEqual(result.returncode, 66, result.stderr)
        self.assertEqual(state['phase'], 'complete')
        self.assertFalse(any(event.startswith(f'switch {OLD}') for event in events))

    def test_corrupt_offhost_archive_never_reaches_installation(self):
        result, events, state = self.shell(
            'printf changed >>"${TREE%/tree}/cutover-backup.tar"\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(state['pending'])
        self.assertNotIn('install-bundle', events)

    def test_archive_failure_after_freeze_recovers_without_dropping_siblings(self):
        result, events, state = self.shell(
            'tar() { return 1; }\nprefix_prepare "$TARGET" "$TREE" "$ATTEMPT"\n', phase=None)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(state['phase'], 'rolled-back', result.stderr)
        self.assertEqual(state['old'], 'cantos')
        self.assertFalse(state['pending'])
        self.assertFalse(any('DROP' in event for event in events))

    def test_unknown_clients_leave_write_freeze_pending_for_operator_reconciliation(self):
        result, events, state = self.shell(
            'FOREIGN_CLIENT=1\nprefix_recover "$TARGET" "$TREE" "$ATTEMPT"\n', phase='freeze-intent')
        self.assertNotEqual(result.returncode, 0)
        self.assertTrue(state['pending'])
        self.assertFalse(any(event.startswith('switch ') for event in events))

    def test_prepare_failure_before_freeze_does_not_stop_the_running_apis(self):
        result, events, state = self.shell(
            'FAIL_MIGRATION=1\nprefix_prepare "$TARGET" "$TREE" "$ATTEMPT"\n', phase=None)
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(state['pending'])
        self.assertEqual(state['old'], 'cantos')
        self.assertFalse(any(event.startswith('systemctl stop') for event in events))

    def test_host_entrypoint_keeps_executable_mode_for_direct_bundle_calls(self):
        import io
        import tarfile
        entry = ROOT / 'infra/vps/bin/cantos-vps'
        self.assertTrue(os.access(entry, os.X_OK))
        staged = subprocess.check_output(['git', 'ls-files', '--stage', 'infra/vps/bin/cantos-vps'], cwd=ROOT)
        self.assertEqual(staged.split()[0], b'100755')
        archive = subprocess.check_output(['git', 'archive', 'HEAD', 'infra/vps/bin/cantos-vps'], cwd=ROOT)
        with tarfile.open(fileobj=io.BytesIO(archive)) as bundle:
            self.assertEqual(bundle.getmember('infra/vps/bin/cantos-vps').mode & 0o111, 0o111)

    def test_paired_host_restore_keeps_the_pending_guard_until_recovery_is_durable(self):
        body = COMMON + r'''
printf '/usr/local/sbin/cantos-vps\n' >"$JOURNAL/all-system-paths"
rm() { event "remove $*"; }
tar() { event "extract $*"; }
epoch2_restore_host "$JOURNAL" prefix
'''
        result, events, _ = self.shell(body)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(any(event.startswith('extract --exclude=usr/local/sbin/cantos-vps ') for event in events))
        self.assertFalse(any(event.startswith('remove ') and event.endswith('/usr/local/sbin/cantos-vps') for event in events))
        result, events, _ = self.shell('FAIL_HEALTH=1\nprefix_commit "$TARGET" "$TREE" "$ATTEMPT"\n')
        self.assertNotEqual(result.returncode, 0)
        self.assertLess(events.index('pending-wrapper'), events.index('restore-host'))
        self.assertLess(events.index('durable rolled-back'),
                        next(i for i, event in enumerate(events) if event.startswith('install-file -m 0750')))


if __name__ == '__main__':
    unittest.main()
