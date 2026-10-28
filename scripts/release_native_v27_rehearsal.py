"""Disposable exact-image v24 -> v27 and cold pair restore; never host activation."""
import argparse
import json
from pathlib import Path
import platform
import re
import tempfile
import time

import release_artifact as contract
import release_native_migration as migration
import release_native_rehearsal as historical
import release_oci_qualify as qualify

SCHEMA_24 = 'server-turso-epoch-1-v24'
SCHEMA_27 = 'server-turso-epoch-1-v27'
CHECKS = ['v24-populated-http-auth-and-revoked-cookie', 'v27-serve-before-migrate-refused',
          'complete-cold-pair-backup', 'v24-to-v27-migrate', 'repeated-migrate',
          'v27-engine-integrity-and-foreign-keys', 'v27-preserved-http-auth-and-revoked-cookie',
          'restore-both-v24-roots', 'v24-restored-http-auth-and-revoked-cookie']

def transition(old, new):
    contract.require(old['platform'] == new['platform'], 'migration platform mismatch')
    before, after = old['compatibility'], new['compatibility']
    contract.require(before.get('server_schema') == SCHEMA_24 and after.get('server_schema') == SCHEMA_27 and
                     before == {**after, 'server_schema':SCHEMA_24}, 'only reviewed v24 to v27 transition supported')

def rehearse(root, args):
    new, _ = qualify.validate_input(root, args)
    old = migration.baseline(root, contract.read_json(args.baseline, args.baseline_digest))
    transition(old, new)
    contract.require(qualify.native_host() == new['platform'], 'NATIVE_PLATFORM_MISMATCH')
    args.output.mkdir(parents=True, mode=0o700, exist_ok=False)
    record = {'schema_version':1, 'operation':'HISTORICAL_V24_V27_REHEARSAL', 'status':'NOT_RUN',
              'baseline':old, 'target':migration.new_identity(new), 'checks':[],
              'input_manifest_sha256':args.manifest_digest, 'baseline_sha256':args.baseline_digest,
              'fixture_configuration_sha256':contract.fingerprint(qualify.fixture_configuration(root)),
              'runner':{'system':platform.system(), 'machine':platform.machine()}, 'started_at':int(time.time()),
              'activation':'NOT_RUN', 'admission':'NOT_RUN', 'live_readiness':'NOT_RUN'}
    contract.write_new(args.output/'attempt.json', record)
    fixture = None
    try:
        with tempfile.TemporaryDirectory(prefix='cantos-v24-v27-rehearsal-') as temporary:
            fixture = qualify.Fixture(root, Path(temporary), old, args.authfile)
            try:
                backups = historical.run(fixture, old, new, old_version='v24', new_version='v27')
            finally:
                fixture.close()
        record.update(status='PASS', evidence_level='integration-tested', execution_environment='linux-native-exact-images',
                      checks=CHECKS, backup_files=backups, host_adapter=fixture.adapter_record,
                      forward_bridge=getattr(fixture, 'forward_bridge_record', {'status':'NOT_RUN'}))
    except Exception as error:
        safe = isinstance(error, ValueError) and re.fullmatch(r'[A-Za-z0-9_:/.-]{1,256}', str(error))
        code = error.code if isinstance(error, migration.adapter.Refusal) else None
        failure = code if code and re.fullmatch(r'[A-Z0-9_]{1,160}', code) else (str(error) if safe else type(error).__name__)
        record.update(status='FAIL', failure=failure)
        if hasattr(fixture, 'adapter_phase'):
            record.update(adapter_phase=fixture.adapter_phase, entrypoint_shapes=getattr(fixture, 'adapter_shapes', {}))
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
        parser.exit(1, 'v24 to v27 rehearsal input refused; no host activation\n')
    print(json.dumps(record, sort_keys=True))
    return 0 if record['status'] == 'PASS' else 1

if __name__ == '__main__':
    raise SystemExit(main())
