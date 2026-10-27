"""Explicit v23 -> v24 adoption of the reviewed /root/cantos-native layout.

Separate from native-upgrade: an observed baseline is never a fabricated admitted release.
An operator approval binds the request, historical rehearsal and maintenance fence. Default
is offline planning. No SSH, credentials, ingress edits, bootstrap, reset or automatic replay.
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import time
import uuid

import release_artifact as contract
import release_native as native
import release_native_adapter as adapter
import release_oci as oci
import release_oci_build as build

ROOT = '/root/cantos-native'
DATABASES = {'cantos-server': ROOT+'/server-db', 'cantos-identity': ROOT+'/identity-db'}
SOCKET = ROOT+'/run'
MEDIA = ROOT+'/storage'
SCHEMA_23 = 'server-turso-epoch-1-v23'
SCHEMA_24 = 'server-turso-epoch-1-v24'
OPERATION = 'ADOPT_NATIVE_V23_V24'


def transition(old, new):
    """Only the reviewed additive Server suffix; every other compatibility key is equal."""
    contract.require(old['platform'] == new['platform'], 'migration platform mismatch')
    before, after = old['compatibility'], new['compatibility']
    contract.require(before.get('server_schema') == SCHEMA_23 and after.get('server_schema') == SCHEMA_24 and
                     before == {**after, 'server_schema': SCHEMA_23}, 'only reviewed v23 to v24 transition supported')


def baseline(root, value):
    oci.fields(value, 'source_sha configuration_sha256 platform compatibility images', 'exact observed baseline required')
    contract.sha(value['source_sha'])
    contract.digest(value['configuration_sha256'])
    contract.require(value['platform'] in oci.PLATFORMS and
                     value['compatibility'] == build.compatibility(root, value['source_sha']),
                     'baseline source registry mismatch')
    oci.fields(value['images'], ' '.join(oci.SERVICES), 'exact baseline pair required')
    for service, image in value['images'].items():
        oci.fields(image, 'ref config_digest', 'exact baseline image pins required')
        oci.image_ref(image['ref'], service)
        oci.oci_digest(image['config_digest'])
    return value


def decide(request, old, new, now, *, recovery=False):
    oci.fields(request, 'schema_version operation host_machine_sha256 observed_at inventory_sha256 baseline next '
               'databases socket_directory units configuration_files mounts health_url peer_policy rehearsal',
               'exact adoption request fields required')
    contract.require(type(request['schema_version']) is int and request['schema_version'] == 1 and
                     request['operation'] == OPERATION, 'adoption request version or operation')
    contract.require(type(request['observed_at']) is int and
                     (recovery or 0 <= now-request['observed_at'] <= 3600), 'fresh adoption inventory required')
    for name in ('host_machine_sha256', 'inventory_sha256'):
        contract.digest(request[name])
    transition(old, new)
    contract.require(request['databases'] == DATABASES and request['socket_directory'] == SOCKET,
                     'unknown existing-host layout; explicit review required')
    contract.require(request['peer_policy'] in ('direct_peer', 'trusted_proxy'), 'explicit preserved peer policy required')
    oci.fields(request['units'], ' '.join(oci.SERVICES), 'exact unit pair required')
    for digest in request['units'].values():
        contract.digest(digest)
    configs = request['configuration_files']
    contract.require(isinstance(configs, dict) and 1 <= len(configs) <= 32, 'bounded complete configuration inventory required')
    for name, digest in configs.items():
        contract.require(isinstance(name, str) and re.fullmatch(re.escape(ROOT)+r'/(?:config/[A-Za-z0-9_.-]+|[A-Za-z0-9_.-]+\.env)', name),
                         'unknown adoption configuration path')
        contract.digest(digest)
    oci.fields(request['mounts'], ' '.join(oci.SERVICES), 'exact adoption mounts required')
    for service in oci.SERVICES:
        native.validate_mount_plan(request, service, media_root=MEDIA+'/', exact_media_root=MEDIA)
        for mount in request['mounts'][service]:
            contract.require(mount['source'] in (DATABASES[service], SOCKET, *configs) or
                             (service == 'cantos-server' and mount['source'] == MEDIA), 'unknown adoption bind root')
    adapter.selector(request['health_url'])
    return {'operation': OPERATION, 'activation': 'NOT_RUN', 'baseline_trust': 'OBSERVED_REQUIRES_OPERATOR_REVIEW',
            'data_paths': request['databases'], 'peer_policy': request['peer_policy'],
            'recovery': 'COLD_PAIR_RESTORE_BEFORE_START_ONLY; AFTER_START_FORWARD_RECOVERY_REQUIRED'}


def rehearsal(request, old, new):
    pins = request['rehearsal']
    oci.fields(pins, 'path sha256', 'pinned historical rehearsal required')
    record = native.pinned(pins['path'], pins['sha256'])
    contract.require(record.get('operation') == 'HISTORICAL_V23_V24_REHEARSAL' and record.get('status') == 'PASS' and
                     record.get('baseline') == old and record.get('target') == new_identity(new) and
                     record.get('evidence_level') == 'integration-tested' and
                     record.get('execution_environment') == 'linux-native-exact-images' and
                     record.get('checks') == HISTORICAL_CHECKS, 'exact historical pair upgrade/restore evidence required')
    contract.require(record.get('host_adapter') == {'status': 'PASS', 'source_sha': new['source_sha'],
                     'images': new['images'], 'checks': adapter.CHECKS, 'activation': 'NOT_RUN'},
                     'exact real Podman bridge adapter evidence required')
    return record


def new_identity(value):
    return {key: value[key] for key in ('source_sha', 'configuration_sha256', 'platform', 'compatibility', 'images')}


HISTORICAL_CHECKS = ['v23-populated-http-auth-and-revoked-cookie', 'v24-serve-before-migrate-refused', 'complete-cold-pair-backup',
                     'v23-to-v24-migrate', 'repeated-migrate', 'v24-engine-integrity-and-foreign-keys',
                     'v24-preserved-http-auth-and-revoked-cookie', 'restore-both-v23-roots', 'v23-restored-http-auth-and-revoked-cookie']


def approval(value, request, request_digest):
    oci.fields(value, 'schema_version operation request_sha256 inventory_sha256 rehearsal_sha256 '
               'target_manifest_sha256 maintenance recovery', 'exact operator adoption approval required')
    contract.require(type(value['schema_version']) is int and value['schema_version'] == 1 and
                     value['operation'] == OPERATION and value['request_sha256'] == request_digest and
                     value['inventory_sha256'] == request['inventory_sha256'] and
                     value['rehearsal_sha256'] == request['rehearsal']['sha256'] and
                     value['target_manifest_sha256'] == request['next']['manifest_digest'] and
                     value['maintenance'] == 'BOTH_NATIVE_WRITERS_FENCED' and
                     value['recovery'] == 'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START',
                     'operator approval must bind request, inventory, rehearsal, target and write-loss boundary')


def data_inventory(root, service):
    """0644 engine files are safe only inside this owner's 0700 directory; no links/mounts."""
    meta = root.lstat()
    contract.require(root == root.resolve() and stat.S_ISDIR(meta.st_mode) and stat.S_IMODE(meta.st_mode) == 0o700,
                     'private canonical database root required')
    filename = 'server.db' if service == 'cantos-server' else 'identity.db'
    allowed = {filename, filename+'-wal', filename+'-shm', filename+'.owner'}
    entries = list(root.iterdir())
    contract.require(filename in {p.name for p in entries} and len(entries) <= len(allowed), 'native database file set required')
    result = {'.': {'uid': meta.st_uid, 'gid': meta.st_gid, 'mode': 0o700, 'sha256': None}}
    for path in sorted(entries):
        info = path.lstat()
        contract.require(path.name in allowed and stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and
                         info.st_dev == meta.st_dev and (info.st_uid, info.st_gid) == (meta.st_uid, meta.st_gid) and
                         not info.st_mode & 0o133 and (not path.name.endswith('.owner') or stat.S_IMODE(info.st_mode) == 0o600),
                         'foreign, linked or unsafe native sidecar refused')
        result[path.name] = {'uid': info.st_uid, 'gid': info.st_gid, 'mode': stat.S_IMODE(info.st_mode),
                             'sha256': native.digest_file(path)}
    return result


def copy_database(source, destination, service, expected=None):
    before = data_inventory(source, service)
    contract.require(expected is None or before == expected, 'cold pair backup corrupt')
    shutil.copytree(source, destination)
    for name, entry in before.items():
        path = destination/name
        os.chown(path, entry['uid'], entry['gid'])
        os.chmod(path, entry['mode'])
        if entry['sha256'] is not None:
            with path.open('rb') as stream:
                os.fsync(stream.fileno())
    contract.require(before == data_inventory(source, service) == data_inventory(destination, service), 'cold pair copy drift')
    native.sync_directory(destination)
    native.sync_directory(destination.parent)
    return before


class AdoptionSystem(native.System):
    configuration_root = ROOT+'/'

    def observe(self, request, old):
        super().observe(request, old)
        # Refuse a wrong/unreachable bridge before PREPARE, pulls, stops or migration.
        self.health(request, old)
        sizes = [p.stat().st_size for name in request['databases'].values() for p in Path(name).iterdir() if p.is_file()]
        capacity = os.statvfs(self.state)
        contract.require(shutil.disk_usage(self.state).free >= sum(sizes)*5+64*1024**2 and
                         capacity.f_favail >= len(sizes)*5+100, 'insufficient migration/backup/restore capacity')

    def check_container(self, request, service, container, images):
        native.check_container(request, service, adapter.command_view(service, container), images, peer_policy=request['peer_policy'],
                               media_root=MEDIA+'/', exact_media_root=MEDIA)
        adapter.check_process(service, container, lambda: self.inspect(service))

    def health(self, request, release):
        value = request['health_url']
        if adapter.selector(value) == 'loopback':
            return super().health(request, release)
        def network():
            values = json.loads(self.command(['podman', 'network', 'inspect', value['network']]))
            contract.require(isinstance(values, list) and len(values) == 1, 'exact bridge network required')
            return values[0]
        adapter.bridge_health(value, lambda: self.inspect('cantos-server'), network,
                              lambda container: self.check_container(request, 'cantos-server', container,
                                                                     [release['images']['cantos-server']]),
                              self.read_health)

    def recovery_host(self, request, old, new):
        self.host_identity(request, old)
        for service in oci.SERVICES:
            self.unit_policy(request, service)
            running = json.loads(self.command(['podman', 'ps', '--format', 'json', '--filter', 'name=^'+service+'$']))
            if running:
                self.check_container(request, service, self.inspect(service), [old['images'][service], new['images'][service]])
            # A journaled crash between the two restore renames can leave the canonical root
            # absent. Only the exact request-bound sibling names are reconciled by recover().
            path = Path(request['databases'][service])
            if path.exists():
                info = path.lstat()
                uid = 10001 if service == 'cantos-server' else 10002
                contract.require(path == path.resolve() and stat.S_ISDIR(info.st_mode) and info.st_uid == uid and
                                 info.st_gid == uid and stat.S_IMODE(info.st_mode) == 0o700, 'recovery database ownership drift')

    def database_command(self, release, service, directory, *, check=False):
        filename = 'server.db' if service == 'cantos-server' else 'identity.db'
        key = 'CANTOS_SERVER_DATABASE' if service == 'cantos-server' else 'CANTOS_IDENTITY_DATABASE'
        entry = '/usr/local/bin/cantos-migrate' if service == 'cantos-server' else '/usr/local/bin/cantos-identity'
        arguments = (['--check'] if check else []) if service == 'cantos-server' else (['check'] if check else ['migrate'])
        self.command(['podman', 'run', '--rm', '--pull=never', '--network=none', '--read-only', '--cap-drop=ALL',
                      '--security-opt=no-new-privileges', '--mount', f'type=bind,src={directory},dst=/database',
                      '--env', key+'=/database/'+filename, '--entrypoint', entry, release['images'][service]['ref'], *arguments])

    def check_pair(self, request, old, new):
        # Old Server verifies the exact v23 history; unchanged Identity v4 uses the new check CLI.
        self.database_command(old, 'cantos-server', Path(request['databases']['cantos-server']), check=True)
        self.database_command(new, 'cantos-identity', Path(request['databases']['cantos-identity']), check=True)

    def migrate(self, request, new):
        for service in oci.SERVICES:
            self.database_command(new, service, Path(request['databases'][service]))
            self.database_command(new, service, Path(request['databases'][service]), check=True)


def unchanged(request, saved_units, metadata):
    for service in oci.SERVICES:
        path = native.private_path(native.UNITS/(service+'.container'))
        contract.require(native.digest_bytes(saved_units[service]) == request['units'][service] and
                         path.read_bytes() == saved_units[service] and native.unit_metadata(path) == metadata[service], 'adoption unit drift')
    for name, digest in request['configuration_files'].items():
        contract.require(native.digest_file(native.private_path(name)) == digest, 'adoption configuration drift')


def adopt(request, old, new, ports, *, request_bytes, approval_bytes, state=native.STATE):
    # State directory creation is a separate explicit operator step, never an invented current.json.
    contract.require(json.loads(request_bytes) == request, 'request changed before custody')
    approval(json.loads(approval_bytes), request, native.digest_bytes(request_bytes))
    with native.locked(state):
        contract.require(not (state/'current.json').exists() and not (state/'in-progress.json').exists(),
                         'existing generation or interrupted adoption; refuse replay')
        ports.observe(request, old)
        attempt = state/('attempt-'+uuid.uuid4().hex)
        attempt.mkdir(mode=0o700)
        units = {s: (native.UNITS/(s+'.container')).read_bytes() for s in oci.SERVICES}
        metadata = {s: native.unit_metadata(native.UNITS/(s+'.container')) for s in oci.SERVICES}
        unchanged(request, units, metadata)
        native.atomic(attempt/'request.json', request_bytes)
        native.atomic(attempt/'adoption-approval.json', approval_bytes)
        native.atomic(attempt/'unit-metadata.json', contract.canonical(metadata))
        for service, data in units.items():
            native.atomic(attempt/(service+'.container'), data)
        # Retain private config bytes with their bound hashes, without printing payloads.
        (attempt/'configuration').mkdir(mode=0o700)
        config_metadata = {}
        for index, (name, digest) in enumerate(sorted(request['configuration_files'].items())):
            data = native.private_path(name).read_bytes()
            contract.require(native.digest_bytes(data) == digest, 'configuration changed before custody')
            native.atomic(attempt/'configuration'/str(index), data)
            config_metadata[name] = {**native.unit_metadata(Path(name)), 'sha256': digest, 'copy': str(index)}
        native.atomic(attempt/'configuration-metadata.json', contract.canonical(config_metadata))
        marker = {'attempt': attempt.name, 'operation': OPERATION, 'phase': 'PREPARE',
                  'request_sha256': native.digest_bytes(request_bytes)}
        native.atomic(state/'in-progress.json', contract.canonical(marker))
        ports.pull(old)
        ports.pull(new)
        ports.stop(request)
        # First prove actual schema state; labels alone are never a DB observation.
        ports.check_pair(request, old, new)
        backups = {s: copy_database(Path(request['databases'][s]), attempt/(s+'-database'), s) for s in oci.SERVICES}
        native.atomic(attempt/'backup.json', contract.canonical({'request_sha256': marker['request_sha256'],
                      'baseline': old, 'databases': backups, 'cut': 'BOTH_OWNERS_STOPPED'}))
        unchanged(request, units, metadata)
        # Verify restore into fresh roots before the first live schema write. Originals remain.
        for service in oci.SERVICES:
            copy_database(attempt/(service+'-database'), attempt/(service+'-restore-check'), service, backups[service])
        restored = {**request, 'databases': {s: str(attempt/(s+'-restore-check')) for s in oci.SERVICES}}
        ports.check_pair(restored, old, new)
        ports.migrate(restored, new)
        marker['backup_sha256'] = native.digest_file(attempt/'backup.json')
        marker['phase'] = 'MIGRATE_INTENT'
        native.atomic(state/'in-progress.json', contract.canonical(marker))
        ports.migrate(request, new)
        unchanged(request, units, metadata)
        for service in oci.SERVICES:
            native.replace_unit(native.UNITS/(service+'.container'),
                                native.unit_image(units[service], old['images'][service]['ref'], new['images'][service]['ref']), metadata[service])
        # Once start is attempted, accepted writes are possible even if health fails or the
        # acknowledgement is lost. Recovery must not restore older authentication/session data.
        marker['phase'] = 'START_INTENT'
        native.atomic(state/'in-progress.json', contract.canonical(marker))
        ports.start(request, new)
        current = {'generation': 1, 'manifest_sha256': request['next']['manifest_digest'], 'images': new['images'],
                   'adoption_request_sha256': marker['request_sha256'], 'baseline': old,
                   'adoption_approval_sha256': native.digest_file(attempt/'adoption-approval.json'),
                   'runtime_layout': {'databases': request['databases'], 'socket_directory': request['socket_directory'],
                                      'peer_policy': request['peer_policy']}}
        native.atomic(state/'current.json', contract.canonical(current))
        native.atomic(attempt/'receipt.json', contract.canonical({'operation': 'ADOPTED', **current,
                      'backup_sha256': native.digest_file(attempt/'backup.json'), 'live_business_acceptance': 'NOT_RUN'}))
        (state/'in-progress.json').unlink()
        native.sync_directory(state)
        return {'operation': 'ADOPTED', 'generation': 1, 'attempt': attempt.name, 'live_business_acceptance': 'NOT_RUN'}


def recovery_fence(request, old, new, units, metadata):
    for service in oci.SERVICES:
        original = units[service]
        path = native.private_path(native.UNITS/(service+'.container'))
        contract.require(native.digest_bytes(original) == request['units'][service] and path.read_bytes() in
                         (original, native.unit_image(original, old['images'][service]['ref'], new['images'][service]['ref'])) and
                         native.unit_metadata(path) == metadata[service], 'foreign recovery unit')
    for name, digest in request['configuration_files'].items():
        contract.require(native.digest_file(native.private_path(name)) == digest, 'recovery configuration drift')


def recover(request, old, new, attempt_name, ports, *, state=native.STATE):
    contract.require(re.fullmatch(r'attempt-[a-f0-9]{32}', attempt_name), 'invalid adoption attempt')
    with native.locked(state):
        marker = native.pinned(state/'in-progress.json', None)
        contract.require(marker.get('attempt') == attempt_name and marker.get('operation') == OPERATION and
                         marker.get('phase') in ('PREPARE', 'MIGRATE_INTENT') and not (state/'current.json').exists(),
                         'after start or foreign generation: forward recovery required; no automatic data loss')
        attempt = native.private_path(state/attempt_name, directory=True)
        contract.require(native.pinned(attempt/'request.json', None) == request and not (attempt/'recovered.json').exists(),
                         'recovery request mismatch or already recovered')
        units = {s: native.private_path(attempt/(s+'.container')).read_bytes() for s in oci.SERVICES}
        metadata = native.pinned(attempt/'unit-metadata.json', None)
        recovery_fence(request, old, new, units, metadata)
        ports.recovery_host(request, old, new)
        ports.pull(old)
        ports.pull(new)
        ports.stop(request)
        contract.require(native.pinned(state/'in-progress.json', None) == marker and not (state/'current.json').exists(),
                         'recovery generation or intent drift')
        recovery_fence(request, old, new, units, metadata)
        ports.recovery_host(request, old, new)
        if marker['phase'] == 'MIGRATE_INTENT':
            record = native.pinned(attempt/'backup.json', marker['backup_sha256'])
            contract.require(record['baseline'] == old and record['request_sha256'] == marker['request_sha256'], 'backup provenance mismatch')
            # Validate the entire pair before the first rename. Never delete a failed generation.
            for service in oci.SERVICES:
                contract.require(data_inventory(attempt/(service+'-database'), service) == record['databases'][service], 'cold pair backup corrupt')
            for service in oci.SERVICES:
                destination = Path(request['databases'][service])
                staged = destination.with_name(destination.name+'.restore-'+attempt_name)
                retained = destination.with_name(destination.name+'.failed-'+attempt_name)
                # Named progress permits explicit reconciliation of a crash between pair renames.
                if not retained.exists():
                    if staged.exists():
                        contract.require(data_inventory(staged, service) == record['databases'][service], 'partial restore staging; manual reconciliation required')
                    else:
                        copy_database(attempt/(service+'-database'), staged, service, record['databases'][service])
                    destination.rename(retained)
                    native.sync_directory(destination.parent)
                if not destination.exists():
                    staged.rename(destination)
                    native.sync_directory(destination.parent)
                contract.require(data_inventory(destination, service) == record['databases'][service], 'recovery data drift')
        ports.check_pair(request, old, new)
        for service in oci.SERVICES:
            native.replace_unit(native.UNITS/(service+'.container'), units[service], metadata[service])
        # Recovery start is also an uncertain write boundary. A crash here must never replay
        # the old snapshot after either owner may have accepted a new revocation or session.
        marker['phase'] = 'RECOVERY_START_INTENT'
        native.atomic(state/'in-progress.json', contract.canonical(marker))
        ports.start(request, old)
        native.atomic(attempt/'recovered.json', contract.canonical({'operation': 'BASELINE_RESTORED_UNADOPTED',
                      'baseline': old, 'generation': 'UNESTABLISHED'}))
        (state/'in-progress.json').unlink()
        native.sync_directory(state)
        return {'operation': 'BASELINE_RESTORED_UNADOPTED', 'generation': 'UNESTABLISHED'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request', required=True, type=Path)
    parser.add_argument('--request-digest', required=True)
    parser.add_argument('--approval', type=Path)
    parser.add_argument('--approval-digest')
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--recover-attempt')
    args = parser.parse_args()
    try:
        request = native.pinned(args.request, args.request_digest)
        root = Path(__file__).resolve().parents[1]
        old = baseline(root, request['baseline'])
        new = native.release(root, request['next'])
        decision = decide(request, old, new, time.time(), recovery=bool(args.recover_attempt))
        rehearsal(request, old, new)
        if args.execute:
            contract.require(oci.git(root, 'rev-parse', 'HEAD').decode().strip() == new['source_sha'] and
                             not oci.git(root, 'status', '--porcelain', '--untracked-files=normal').strip(),
                             'execute requires the clean exact reviewed target source')
            contract.require(args.approval is not None and args.approval_digest is not None, 'pinned operator approval required')
            approved = native.pinned(args.approval, args.approval_digest)
            approval(approved, request, args.request_digest)
            request_bytes = args.request.read_bytes()
            approval_bytes = args.approval.read_bytes()
            contract.require(native.digest_bytes(request_bytes) == args.request_digest and
                             native.digest_bytes(approval_bytes) == args.approval_digest, 'operator inputs changed before custody')
            decision = (recover(request, old, new, args.recover_attempt, AdoptionSystem()) if args.recover_attempt else
                        adopt(request, old, new, AdoptionSystem(), request_bytes=request_bytes, approval_bytes=approval_bytes))
        print(json.dumps(decision, sort_keys=True))
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError):
        parser.exit(1, 'native adoption refused or interrupted; reconcile private marker; never retry blindly\n')


if __name__ == '__main__':
    main()
