"""Production planner + orchestration on real files with a named FakeHost, never DB/host acceptance."""
import copy
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_native as native
import release_native_migration as migration
from test_release_native import FakeHost, pair, request


def releases():
    new = pair('2')
    new['compatibility'] = {'server_schema': migration.SCHEMA_24, 'identity_schema': 'identity-migrations-v4',
                            'rpc': 'identity-rpc-v1', 'security_state': 'identity-security-store-v1', 'turso': '0.8.3'}
    old = pair('1')
    old['compatibility'] = {**new['compatibility'], 'server_schema': migration.SCHEMA_23}
    return old, new


def adoption_request():
    old, _ = releases()
    value = request()
    for name in ('host_state', 'generation', 'current_manifest_sha256', 'previous'):
        del value[name]
    value.update(operation=migration.OPERATION, inventory_sha256='d'*64, baseline=old,
                 peer_policy='trusted_proxy', rehearsal={'path': '/private/historical.json', 'sha256': 'e'*64})
    for service in oci_services():
        value['mounts'][service][0]['source'] = migration.DATABASES[service]
        value['mounts'][service][1]['source'] = migration.SOCKET
    value['databases'] = migration.DATABASES.copy()
    value['socket_directory'] = migration.SOCKET
    value['configuration_files'] = {migration.ROOT+'/native.env': 'c'*64}
    return value


def oci_services():
    return native.SERVICES


class Decisions(unittest.TestCase):
    def test_proxy_policy_is_preserved_only_by_the_explicit_adoption_profile(self):
        request_value = adoption_request()
        service = 'cantos-server'
        old, _ = releases()
        request_value['mounts'][service].append({'source': migration.MEDIA, 'destination': '/private', 'read_only': False})
        container = {'ImageName': old['images'][service]['ref'], 'Image': old['images'][service]['config_digest'],
                     'Path': '/usr/local/bin/cantos-server', 'Args': [],
                     'Config': {'User': '10001:10001', 'Env': ['CANTOS_SERVER_DATABASE=/data/server.db',
                                'CANTOS_IDENTITY_SOCKET=/rpc/identity.sock', 'CANTOS_IDENTITY_PEER_UID=10002',
                                'CANTOS_IDENTITY_PEER_POLICY=trusted_proxy']},
                     'Mounts': [{'Type': 'bind', 'Source': item['source'], 'Destination': item['destination'],
                                 'RW': not item['read_only']} for item in request_value['mounts'][service]]}
        migration.AdoptionSystem().check_container(request_value, service, container, [old['images'][service]])
        with self.assertRaisesRegex(ValueError, 'unbound mount'):
            native.check_container(request_value, service, container, [old['images'][service]])
        container['Config']['Env'][-1] = 'CANTOS_IDENTITY_PEER_POLICY=direct_peer'
        with self.assertRaisesRegex(ValueError, 'peer policy mismatch'):
            migration.AdoptionSystem().check_container(request_value, service, container, [old['images'][service]])

    def test_plan_preserves_known_paths_proxy_and_never_claims_live_readiness(self):
        value = adoption_request()
        decision = migration.decide(value, *releases(), time.time())
        self.assertEqual(decision['activation'], 'NOT_RUN')
        self.assertEqual(decision['peer_policy'], 'trusted_proxy')
        self.assertEqual(decision['data_paths'], migration.DATABASES)

    def test_unknown_layout_schema_engine_identity_policy_and_stale_inventory_refuse(self):
        old, new = releases()
        for change, message in [({'databases': {'cantos-server': '/unknown', 'cantos-identity': '/other'}}, 'unknown existing-host layout'),
                                ({'peer_policy': 'trust_everyone'}, 'peer policy'),
                                ({'observed_at': int(time.time())-3601}, 'fresh adoption'),
                                ({'health_url': 'http://127.0.0.1:65536/health'}, 'loopback health'),
                                ({'configuration_files': {'/etc/passwd': 'c'*64}}, 'configuration path')]:
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, message):
                migration.decide(adoption_request() | change, old, new, time.time())
        for key, value in [('turso', '0.9'), ('identity_schema', 'v5'), ('rpc', 'rpc-v2'), ('server_schema', 'server-turso-epoch-1-v25')]:
            altered = copy.deepcopy(new)
            altered['compatibility'][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'only reviewed v23'):
                migration.transition(old, altered)

    def test_mounts_must_be_complete_and_bound_even_in_known_layout(self):
        for source in ['/root/cantos-native/storage/foreign', migration.DATABASES['cantos-identity'], '/etc/passwd']:
            value = adoption_request()
            value['mounts']['cantos-server'].append({'source': source, 'destination': '/media', 'read_only': True})
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, 'unknown adoption bind|unbound mount'):
                migration.decide(value, *releases(), time.time())

    def test_approval_binds_exact_request_inventory_rehearsal_target_and_write_loss_policy(self):
        request_value = adoption_request()
        value = {'schema_version': 1, 'operation': migration.OPERATION, 'request_sha256': 'f'*64,
                 'inventory_sha256': request_value['inventory_sha256'], 'rehearsal_sha256': 'e'*64,
                 'target_manifest_sha256': '2'*64, 'maintenance': 'BOTH_NATIVE_WRITERS_FENCED',
                 'recovery': 'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START'}
        migration.approval(value, request_value, 'f'*64)
        for field in value:
            altered = {**value, field: 'foreign'}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'operator approval must bind'):
                migration.approval(altered, request_value, 'f'*64)

    def test_fresh_same_schema_restore_cannot_be_relabelled_as_historical_upgrade(self):
        old, new = releases()
        record = {'operation': 'HISTORICAL_V23_V24_REHEARSAL', 'status': 'PASS', 'baseline': old,
                  'target': migration.new_identity(new), 'evidence_level': 'integration-tested',
                  'execution_environment': 'linux-native-exact-images', 'checks': migration.HISTORICAL_CHECKS}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory).resolve()/'report.json'
            for field, value in [('operation', 'migration-restore'), ('baseline', migration.new_identity(new)),
                                 ('checks', ['fresh-current-schema']), ('status', 'NOT_RUN'),
                                 ('evidence_level', 'mock-integration-tested'), ('target', old)]:
                altered = {**record, field: value}
                native.atomic(path, contract.canonical(altered))
                request_value = adoption_request() | {'rehearsal': {'path': str(path), 'sha256': native.digest_file(path)}}
                with self.subTest(field=field), self.assertRaisesRegex(ValueError, 'historical pair'):
                    migration.rehearsal(request_value, old, new)
            native.atomic(path, contract.canonical(record))
            request_value['rehearsal']['sha256'] = native.digest_file(path)
            self.assertEqual(migration.rehearsal(request_value, old, new), record)


class MigrationHost(FakeHost):
    """A fake database command, explicitly not an engine migration test."""
    def check_pair(self, request_value, *_):
        self.event('check-pair')

    def migrate(self, request_value, _):
        label = 'migrate-copy' if 'restore-check' in request_value['databases']['cantos-server'] else 'migrate-live'
        # Model partial first-service progress before the failure is observed.
        path = Path(request_value['databases']['cantos-server'])/'server.db'
        path.write_bytes(b'modeled-v24-data')
        self.event(label)


class ColdPairAndRecovery(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        self.state = self.root/'state'
        self.state.mkdir(mode=0o700)
        self.units = self.root/'units'
        self.units.mkdir()
        replacement = patch.object(native, 'UNITS', self.units)
        replacement.start()
        self.addCleanup(replacement.stop)
        self.old, self.new = releases()
        self.request = adoption_request()
        config = self.root/'native.env'
        native.atomic(config, b'private synthetic configuration\n')
        self.request['configuration_files'] = {str(config): native.digest_file(config)}
        self.before = {}
        for service in native.SERVICES:
            root = self.root/service
            root.mkdir(mode=0o700)
            self.request['databases'][service] = str(root)
            filename = 'server.db' if service == 'cantos-server' else 'identity.db'
            for suffix in ('', '-wal', '.owner'):
                path = root/(filename+suffix)
                path.write_bytes((service+suffix).encode())
                path.chmod(0o600 if suffix == '.owner' else 0o644)
            self.before[service] = migration.data_inventory(root, service)
            unit = self.units/(service+'.container')
            native.atomic(unit, ('[Container]\nImage='+self.old['images'][service]['ref']+'\n').encode())
            self.request['units'][service] = native.digest_file(unit)

    def adopt(self, host=None):
        # Synthetic operator input exercises the real approval boundary; it never authorizes
        # a host or supplies real OCI qualification. FakeHost remains the explicit I/O double.
        raw = json.dumps(self.request, indent=2).encode()+b'\n'
        approved = {'schema_version': 1, 'operation': migration.OPERATION, 'request_sha256': native.digest_bytes(raw),
                    'inventory_sha256': self.request['inventory_sha256'], 'rehearsal_sha256': self.request['rehearsal']['sha256'],
                    'target_manifest_sha256': self.request['next']['manifest_digest'], 'maintenance': 'BOTH_NATIVE_WRITERS_FENCED',
                    'recovery': 'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START'}
        return migration.adopt(self.request, self.old, self.new, host or MigrationHost(),
                               request_bytes=raw, approval_bytes=json.dumps(approved, indent=2).encode()+b'\n', state=self.state)

    def attempt(self):
        return next(self.state.glob('attempt-*'))

    def recover(self, host=None):
        return migration.recover(self.request, self.old, self.new, self.attempt().name,
                                 host or MigrationHost(), state=self.state)

    def test_complete_cold_pair_restore_rehearsal_precedes_live_migrate_and_generation(self):
        host = MigrationHost()
        result = self.adopt(host)
        self.assertEqual(host.events, ['observe', 'pull-1', 'pull-2', 'stop', 'check-pair', 'check-pair',
                                       'migrate-copy', 'migrate-live', 'start-2'])
        self.assertEqual(result['operation'], 'ADOPTED')
        current = native.pinned(self.state/'current.json', None)
        self.assertEqual(current['generation'], 1)
        self.assertEqual(current['baseline'], self.old)
        self.assertEqual(current['adoption_approval_sha256'], native.digest_file(self.attempt()/'adoption-approval.json'))
        self.assertEqual(current['adoption_request_sha256'], native.digest_file(self.attempt()/'request.json'))
        self.assertNotIn('previous_manifest_sha256', current)  # No invented admitted old release.
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(self.attempt()/(service+'-database'), service), self.before[service])
        self.assertEqual((self.attempt()/'configuration'/'0').read_bytes(), b'private synthetic configuration\n')
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_every_interrupted_step_blocks_automatic_replay(self):
        # Each subtest owns a fresh fixture, so no previous failure supplies a false refusal.
        for stage in ['observe', 'pull-1', 'pull-2', 'stop', 'check-pair', 'migrate-copy', 'migrate-live', 'start-2']:
            with self.subTest(stage=stage):
                instance = ColdPairAndRecovery('test_complete_cold_pair_restore_rehearsal_precedes_live_migrate_and_generation')
                instance.setUp()
                try:
                    with self.assertRaisesRegex(ValueError, 'injected'):
                        instance.adopt(MigrationHost(fail=stage))
                    if stage != 'observe':
                        host = MigrationHost()
                        with self.assertRaisesRegex(ValueError, 'interrupted adoption'):
                            instance.adopt(host)
                        self.assertEqual(host.events, [])
                        self.assertFalse((instance.state/'current.json').exists())
                finally:
                    instance.doCleanups()

    def test_copy_rehearsal_failure_preserves_original_pair_and_allows_baseline_restart(self):
        with self.assertRaisesRegex(ValueError, 'injected'):
            self.adopt(MigrationHost(fail='migrate-copy'))
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), self.before[service])
        self.assertEqual(self.recover()['operation'], 'BASELINE_RESTORED_UNADOPTED')
        self.assertFalse((self.state/'current.json').exists())

    def test_partial_live_migration_restores_both_and_retains_failed_pair(self):
        with self.assertRaisesRegex(ValueError, 'injected'):
            self.adopt(MigrationHost(fail='migrate-live'))
        self.assertEqual(self.recover()['generation'], 'UNESTABLISHED')
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), self.before[service])
            self.assertTrue((self.root/(service+'.failed-'+self.attempt().name)).is_dir())
        with self.assertRaises(FileNotFoundError):
            self.recover()

    def test_crash_between_restore_renames_can_resume_without_overwriting_either_copy(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='migrate-live'))
        rename = Path.rename
        injected = False
        def fail_once(path, target):
            nonlocal injected
            if '.restore-attempt-' in path.name and not injected:
                injected = True
                raise OSError('injected restore interruption')
            return rename(path, target)
        with patch.object(Path, 'rename', fail_once), self.assertRaisesRegex(OSError, 'restore interruption'):
            self.recover()
        self.assertFalse(Path(self.request['databases']['cantos-server']).exists())
        self.recover()
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), self.before[service])

    def test_corrupt_one_backup_prevents_restoring_either_database(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='migrate-live'))
        before = {s: migration.data_inventory(Path(self.request['databases'][s]), s) for s in native.SERVICES}
        (self.attempt()/'cantos-identity-database'/'identity.db-wal').write_bytes(b'corrupt')
        with self.assertRaisesRegex(ValueError, 'backup corrupt'):
            self.recover()
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), before[service])

    def test_backup_manifest_is_sealed_in_the_durable_migration_intent(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='migrate-live'))
        manifest = self.attempt()/'backup.json'
        value = native.pinned(manifest, None)
        value['databases']['cantos-server']['server.db']['sha256'] = '0'*64
        native.atomic(manifest, contract.canonical(value))
        before = {s: migration.data_inventory(Path(self.request['databases'][s]), s) for s in native.SERVICES}
        with self.assertRaisesRegex(ValueError, 'manifest digest mismatch'):
            self.recover()
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), before[service])

    def test_start_failure_or_unknown_start_never_restores_security_state(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='start-2'))
        host = MigrationHost()
        with self.assertRaisesRegex(ValueError, 'forward recovery required'):
            self.recover(host)
        self.assertEqual(host.events, [])

    def test_configuration_drift_refuses_recovery(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='migrate-live'))
        config = Path(next(iter(self.request['configuration_files'])))
        config.write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'configuration drift'):
            self.recover()

    def test_foreign_unit_refuses_recovery_before_effects(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='migrate-live'))
        unit = self.units/'cantos-server.container'
        unit.write_bytes(unit.read_bytes()+b'Environment=FOREIGN=1\n')
        host = MigrationHost()
        with self.assertRaisesRegex(ValueError, 'foreign recovery unit'):
            self.recover(host)
        self.assertEqual(host.events, [])

    def test_another_generation_refuses_recovery_before_effects(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='migrate-live'))
        native.atomic(self.state/'current.json', contract.canonical({'generation': 9}))
        host = MigrationHost()
        with self.assertRaisesRegex(ValueError, 'forward recovery required'):
            self.recover(host)
        self.assertEqual(host.events, [])

    def test_recovery_rechecks_drift_after_pull_and_stop_before_restoring_either_database(self):
        with self.assertRaises(ValueError):
            self.adopt(MigrationHost(fail='migrate-live'))
        unit = self.units/'cantos-server.container'
        before = {s: migration.data_inventory(Path(self.request['databases'][s]), s) for s in native.SERVICES}
        host = MigrationHost(on_stop=lambda: unit.write_bytes(unit.read_bytes()+b'Environment=FOREIGN=1\n'))
        with self.assertRaisesRegex(ValueError, 'foreign recovery unit'):
            self.recover(host)
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), before[service])

    def test_partial_pair_backup_never_authorizes_migration(self):
        copy_database = migration.copy_database
        def fail_second(source, destination, service, expected=None):
            if destination.name == 'cantos-identity-database':
                raise OSError('injected disk exhaustion')
            return copy_database(source, destination, service, expected)
        host = MigrationHost()
        with patch.object(migration, 'copy_database', fail_second), self.assertRaisesRegex(OSError, 'disk exhaustion'):
            self.adopt(host)
        self.assertNotIn('migrate-live', host.events)
        self.assertFalse((self.attempt()/'backup.json').exists())
        self.assertEqual(native.pinned(self.state/'in-progress.json', None)['phase'], 'PREPARE')
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), self.before[service])

    def test_lost_receipt_retains_real_generation_and_requires_forward_recovery(self):
        atomic = native.atomic
        def fail_receipt(path, data, mode=0o600):
            if path.name == 'receipt.json':
                raise OSError('injected lost receipt')
            return atomic(path, data, mode)
        with patch.object(native, 'atomic', fail_receipt), self.assertRaisesRegex(OSError, 'lost receipt'):
            self.adopt()
        self.assertEqual(native.pinned(self.state/'current.json', None)['generation'], 1)
        self.assertTrue((self.state/'in-progress.json').exists())
        host = MigrationHost()
        with self.assertRaisesRegex(ValueError, 'forward recovery required'):
            self.recover(host)
        self.assertEqual(host.events, [])

    def test_configuration_drift_during_stop_refuses_before_live_migration(self):
        config = Path(next(iter(self.request['configuration_files'])))
        host = MigrationHost(on_stop=lambda: config.write_bytes(b'changed during stop'))
        with self.assertRaisesRegex(ValueError, 'configuration drift'):
            self.adopt(host)
        self.assertNotIn('migrate-live', host.events)
        for service in native.SERVICES:
            self.assertEqual(migration.data_inventory(Path(self.request['databases'][service]), service), self.before[service])

    def test_drift_between_host_observation_and_custody_refuses_before_pull_or_stop(self):
        unit = self.units/'cantos-server.container'
        class DriftHost(MigrationHost):
            def observe(self, *args):
                super().observe(*args)
                unit.write_bytes(unit.read_bytes()+b'Environment=FOREIGN=1\n')
        host = DriftHost()
        with self.assertRaisesRegex(ValueError, 'adoption unit drift'):
            self.adopt(host)
        self.assertEqual(host.events, ['observe'])
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_non_database_assets_symlinks_hardlinks_unsafe_modes_refuse_backup(self):
        source = Path(self.request['databases']['cantos-server'])
        foreign = source/'foreign'
        foreign.write_bytes(b'foreign')
        with self.assertRaisesRegex(ValueError, 'foreign, linked'):
            migration.data_inventory(source, 'cantos-server')
        foreign.unlink()
        for kind in ['symlink', 'hardlink', 'writable']:
            with self.subTest(kind=kind):
                path = source/'server.db-shm'
                if kind == 'symlink':
                    path.symlink_to(source/'server.db')
                elif kind == 'hardlink':
                    os.link(source/'server.db', path)
                else:
                    path.write_bytes(b'bad')
                    path.chmod(0o666)
                with self.assertRaisesRegex(ValueError, 'foreign, linked'):
                    migration.data_inventory(source, 'cantos-server')
                path.unlink()


if __name__ == '__main__':
    unittest.main()
