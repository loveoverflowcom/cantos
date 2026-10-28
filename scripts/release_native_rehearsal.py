"""Disposable Linux historical upgrade, separate from fresh/current-schema qualification.

Uses the actual old image pair to create/populate v23 + Identity v4, the actual new pair to
upgrade it, and the old pair again on a verified cold pair restore. Synthetic data only.
Never emits OCI admission, host generation, live readiness or an old-release qualification.
"""
import argparse
import json
from pathlib import Path
import platform
import re
import tempfile
import time

import release_artifact as contract
import release_native_migration as migration
import release_native_adapter_rehearsal as adapter_rehearsal
import release_oci as oci
import release_oci_qualify as qualify


def run(fixture, old, new, old_version='v23', new_version='v24'):
    f = fixture
    for release in (old, new):
        f.manifest = release
        for service in oci.SERVICES:
            auth = ['--authfile', str(f.authfile)] if f.authfile else []
            f.pod('pull', *auth, f.image(service), timeout=600)
            qualify.inspect_identity(service, release['images'][service], release,
                                     json.loads(f.pod('image', 'inspect', f.image(service)))[0])
    f.manifest = old
    f.migrate()
    f.start()
    f.call('signup')
    f.call('historical_state')
    f.stop()
    backups = {s: qualify.cold_copy(path, f.work/(old_version+'-backup')/s) for s, path in f.databases.items()}
    f.manifest = new
    before = qualify.directory_record(f.databases['cantos-server'])
    f.pod('run', '--rm', '--network=none', *f.options('cantos-server'), '--entrypoint',
          '/usr/local/bin/cantos-migrate', f.image('cantos-server'), '--check', expected=1)
    qualify.require(before == qualify.directory_record(f.databases['cantos-server']), 'PENDING_SCHEMA_CHECK_CHANGED_BYTES')
    f.databases = {s: f.work/(new_version+'-upgrade')/s for s in oci.SERVICES}
    for service in oci.SERVICES:
        qualify.cold_copy(f.work/(old_version+'-backup')/service, f.databases[service], backups[service])
    f.migrate()
    f.migrate()
    for service in oci.SERVICES:
        entry = '/usr/local/bin/cantos-migrate' if service == 'cantos-server' else '/usr/local/bin/cantos-identity'
        flag = '--check' if service == 'cantos-server' else 'check'
        f.pod('run', '--rm', '--network=none', *f.options(service), '--entrypoint', entry, f.image(service), flag)
    f.restart()
    f.call('historical_state')
    f.stop()
    f.adapter_record = adapter_rehearsal.run(f)
    f.manifest = old
    f.databases = {s: f.work/(old_version+'-restored')/s for s in oci.SERVICES}
    for service in oci.SERVICES:
        qualify.cold_copy(f.work/(old_version+'-backup')/service, f.databases[service], backups[service])
    # No migrate call here: the old binary must serve the unchanged cold baseline restore.
    f.restart()
    f.call('historical_state')
    f.stop()
    return backups


def rehearse(root, args):
    new, _ = qualify.validate_input(root, args)
    # This is a tracked public image-pin document, unlike private operator approvals. Root's
    # disposable runner may read the checkout owned by the Actions user; the digest and source
    # registry bind its content. No private-file ownership exception is made for live inputs.
    old = migration.baseline(root, contract.read_json(args.baseline, args.baseline_digest))
    migration.transition(old, new)
    contract.require(qualify.native_host() == new['platform'], 'NATIVE_PLATFORM_MISMATCH')
    args.output.mkdir(parents=True, mode=0o700, exist_ok=False)
    record = {'schema_version': 1, 'operation': 'HISTORICAL_V23_V24_REHEARSAL', 'status': 'NOT_RUN',
              'baseline': old, 'target': migration.new_identity(new), 'checks': [],
              'input_manifest_sha256': args.manifest_digest, 'baseline_sha256': args.baseline_digest,
              'fixture_configuration_sha256': contract.fingerprint(qualify.fixture_configuration(root)),
              'runner': {'system': platform.system(), 'machine': platform.machine()}, 'started_at': int(time.time()),
              'activation': 'NOT_RUN', 'admission': 'NOT_RUN', 'live_readiness': 'NOT_RUN'}
    contract.write_new(args.output/'attempt.json', record)
    fixture = None
    try:
        with tempfile.TemporaryDirectory(prefix='cantos-historical-rehearsal-') as temporary:
            fixture = qualify.Fixture(root, Path(temporary), old, args.authfile)
            try:
                backups = run(fixture, old, new)
            finally:
                fixture.close()
        record.update(status='PASS', evidence_level='integration-tested', execution_environment='linux-native-exact-images',
                      checks=migration.HISTORICAL_CHECKS, backup_files=backups, host_adapter=fixture.adapter_record)
    except Exception as error:
        # Keep static assertion codes, never subprocess stderr, configuration or data rows.
        safe = isinstance(error, ValueError) and re.fullmatch(r'[A-Za-z0-9_:/.-]{1,256}', str(error))
        code = error.code if isinstance(error, migration.adapter.Refusal) else None
        failure = code if code and re.fullmatch(r'[A-Z0-9_]{1,160}', code) else (str(error) if safe else type(error).__name__)
        record.update(status='FAIL', failure=failure)
        if hasattr(fixture, 'adapter_phase'):
            record.update(adapter_phase=fixture.adapter_phase, entrypoint_shapes=fixture.adapter_shapes
                          if hasattr(fixture, 'adapter_shapes') else {})
    contract.write_new(args.output/'historical-upgrade.json', record)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest', 'baseline', 'output'):
        parser.add_argument('--'+name, required=True, type=Path)
    for name in ('manifest-digest', 'source-sha', 'configuration-digest', 'platform', 'baseline-digest'):
        parser.add_argument('--'+name, required=True)
    parser.add_argument('--authfile', type=Path)
    args = parser.parse_args()
    try:
        record = rehearse(Path(__file__).resolve().parents[1], args)
    except (ValueError, OSError, KeyError, TypeError):
        parser.exit(1, 'historical rehearsal input refused; no host activation\n')
    print(json.dumps(record, sort_keys=True))
    return 0 if record['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
