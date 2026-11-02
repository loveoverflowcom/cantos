"""Disposable exact-image v27 -> v35 forward upgrade; never live activation/restore."""
import argparse
import json
from pathlib import Path
import platform
import re
import tempfile
import time

import release_artifact as contract
import release_native_adapter_rehearsal as bridge
import release_native_migration as migration
import release_oci as oci
import release_oci_qualify as qualify

OPERATION = 'HISTORICAL_V27_V35_FORWARD_REHEARSAL'
SCHEMA_27 = 'server-turso-epoch-1-v27'
SCHEMA_35 = 'server-turso-epoch-1-v35'
CHECKS = ['exact-v27-populated-http-auth-and-revoked-cookie',
          'v35-serve-before-migrate-refused-unchanged', 'complete-stopped-current-pair',
          'v27-to-v35-migrate', 'repeated-migrate', 'v35-integrity-and-foreign-keys',
          'preserved-http-auth-and-pre-cut-revoked-cookie',
          'post-upgrade-revocation-survives-restart', 'real-podman-process-and-bridge']


def transition(old, new):
    contract.require(old['platform'] == new['platform'], 'migration platform mismatch')
    before, after = old['compatibility'], new['compatibility']
    contract.require(before.get('server_schema') == SCHEMA_27 and
                     after.get('server_schema') == SCHEMA_35 and
                     before == {**after, 'server_schema': SCHEMA_27},
                     'only reviewed v27 to v35 transition supported')


def run(f, old, new):
    """Real files/images/HTTP; copied roots remain disposable and never become live data."""
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
    copies = {service: qualify.cold_copy(path, f.work/'v27-current-cut'/service)
              for service, path in f.databases.items()}
    f.manifest = new
    before = qualify.directory_record(f.databases['cantos-server'])
    f.pod('run', '--rm', '--network=none', *f.options('cantos-server'), '--entrypoint',
          '/usr/local/bin/cantos-migrate', f.image('cantos-server'), '--check', expected=1)
    qualify.require(before == qualify.directory_record(f.databases['cantos-server']),
                    'PENDING_SCHEMA_CHECK_CHANGED_BYTES')
    # The stopped source accepts no intervening writes. These fresh copies are for
    # forward qualification only; no historical snapshot is restored after a start.
    f.databases = {service: f.work/'v35-forward'/service for service in oci.SERVICES}
    for service in oci.SERVICES:
        qualify.cold_copy(f.work/'v27-current-cut'/service, f.databases[service], copies[service])
    f.migrate()
    f.migrate()
    for service in oci.SERVICES:
        entry = '/usr/local/bin/cantos-migrate' if service == 'cantos-server' else '/usr/local/bin/cantos-identity'
        flag = '--check' if service == 'cantos-server' else 'check'
        f.pod('run', '--rm', '--network=none', *f.options(service), '--entrypoint',
              entry, f.image(service), flag)
    f.restart()
    f.call('historical_state')
    f.call('revoke')
    f.stop()
    f.restart()
    f.call('revoked')
    f.stop()
    f.adapter_record = bridge.run(f)
    return copies


def rehearse(root, args):
    new, _ = qualify.validate_input(root, args)
    old = migration.baseline(root, contract.read_json(args.baseline, args.baseline_digest))
    transition(old, new)
    contract.require(qualify.native_host() == new['platform'], 'NATIVE_PLATFORM_MISMATCH')
    args.output.mkdir(parents=True, mode=0o700, exist_ok=False)
    record = {'schema_version': 1, 'operation': OPERATION, 'status': 'NOT_RUN',
              'baseline': old, 'target': migration.new_identity(new), 'checks': [],
              'input_manifest_sha256': args.manifest_digest, 'baseline_sha256': args.baseline_digest,
              'fixture_configuration_sha256': contract.fingerprint(qualify.fixture_configuration(root)),
              'runner': {'system': platform.system(), 'machine': platform.machine()},
              'started_at': int(time.time()), 'activation': 'NOT_RUN', 'admission': 'NOT_RUN',
              'live_readiness': 'NOT_RUN', 'historical_snapshot_restore': 'NOT_RUN',
              'recovery_acceptance': 'NO_GO_REMAINS_SEPARATE',
              'executor_transition': 'NOT_RUN'}
    contract.write_new(args.output/'attempt.json', record)
    fixture = None
    try:
        with tempfile.TemporaryDirectory(prefix='cantos-v27-v35-forward-') as tmp:
            fixture = qualify.Fixture(root, Path(tmp), old, args.authfile)
            try:
                copies = run(fixture, old, new)
            finally:
                fixture.close()
        record.update(status='PASS', evidence_level='integration-tested',
                      execution_environment='linux-native-exact-images', checks=CHECKS,
                      current_cut_files=copies, host_adapter=fixture.adapter_record)
    except Exception as error:
        code = str(error)
        safe = isinstance(error, ValueError) and re.fullmatch('[A-Z0-9_]{1,160}', code)
        record.update(status='FAIL', failure=code if safe else type(error).__name__)
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
        parser.exit(1, 'v27 to v35 forward rehearsal input refused; no host activation\n')
    print(json.dumps(record, sort_keys=True))
    return 0 if record['status'] == 'PASS' else 1


if __name__ == '__main__':
    raise SystemExit(main())
