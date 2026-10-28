"""Synthetic historical custody for a disposable executor rehearsal only.

Inputs must be real v23/v24 fixture databases and observed fixture units. The private provisional
executor record breaks the test bootstrap cycle only; it is not exported, reviewed evidence,
admission, a host generation or permission to operate on a real host.
"""
import copy
import os
from pathlib import Path
import time
import uuid

import release_artifact as contract
import release_native as native
import release_native_forward as forward
import release_native_migration as migration
import release_oci as oci


def write(path, value):
    forward._write_once(path, contract.canonical(value))


def create(layout, old, running, v23_roots, executor_source, manifest_digest, fixture_marker):
    migration.transition(old, running)
    contract.sha(executor_source)
    contract.digest(manifest_digest)
    state = native.private_path(native.STATE, directory=True)
    marker_path = native.private_path(state/'disposable-fixture-marker')
    contract.require(os.geteuid() == 0 and os.environ.get('GITHUB_ACTIONS') == 'true' and
                     os.environ.get('RUNNER_ENVIRONMENT') == 'github-hosted' and
                     isinstance(fixture_marker, bytes) and len(fixture_marker) == 32 and
                     marker_path.read_bytes() == fixture_marker,
                     'owned disposable fixture marker required')
    contract.require(not (state/'in-progress.json').exists() and not (state/'current.json').exists(),
                     'fresh fixture state required')
    attempt = state/('attempt-'+uuid.uuid4().hex)
    attempt.mkdir(mode=0o700)
    (attempt/'configuration').mkdir(mode=0o700)
    former = attempt/'forward-recovery'
    former.mkdir(mode=0o700)
    unit_metadata, saved_units, corrected = {}, {}, {}
    for service in oci.SERVICES:
        current_path = native.private_path(native.UNITS/(service+'.container'))
        current = current_path.read_bytes()
        previous_ref = old['images'][service]['ref']
        current_ref = running['images'][service]['ref']
        original = forward.migration_unit(current, current_ref, previous_ref)
        before = native.unit_image(original, previous_ref, current_ref)
        corrected[service] = forward.forward_unit(original, before, current, previous_ref, current_ref)
        saved_units[service] = native.digest_bytes(original)
        unit_metadata[service] = native.unit_metadata(current_path)
        forward._write_once(attempt/(service+'.container'), original)
        forward._write_once(former/(service+'.before'), before)
    write(attempt/'unit-metadata.json', unit_metadata)
    config_metadata = {}
    for index, (name, digest) in enumerate(sorted(layout['configuration_files'].items())):
        path = native.private_path(name)
        contract.require(native.digest_file(path) == digest, 'fixture configuration drift')
        forward._write_once(attempt/'configuration'/str(index), path.read_bytes())
        config_metadata[name] = {**native.unit_metadata(path), 'sha256': digest, 'copy': str(index)}
    write(attempt/'configuration-metadata.json', config_metadata)
    original = {**copy.deepcopy(layout), 'schema_version': 1, 'operation': migration.OPERATION,
                'observed_at': int(time.time()), 'inventory_sha256': 'a'*64, 'baseline': old,
                'next': {'manifest_digest': manifest_digest, 'source_sha': running['source_sha'],
                         'configuration_digest': running['configuration_sha256'], 'platform': running['platform']},
                'units': saved_units, 'rehearsal': {'path': '/fixture/test-only', 'sha256': 'b'*64}}
    migration.decide(original, old, running, time.time())
    write(attempt/'request.json', original)
    request_digest = native.digest_file(attempt/'request.json')
    write(attempt/'adoption-approval.json', {'schema_version': 1, 'operation': migration.OPERATION,
          'request_sha256': request_digest, 'inventory_sha256': original['inventory_sha256'],
          'rehearsal_sha256': original['rehearsal']['sha256'], 'target_manifest_sha256': manifest_digest,
          'maintenance': 'BOTH_NATIVE_WRITERS_FENCED', 'recovery': 'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START'})
    backups = {service: migration.copy_database(Path(v23_roots[service]), attempt/(service+'-database'), service)
               for service in oci.SERVICES}
    write(attempt/'backup.json', {'baseline': old, 'request_sha256': request_digest,
                                  'databases': backups, 'cut': 'BOTH_OWNERS_STOPPED'})
    marker = {'attempt': attempt.name, 'operation': migration.OPERATION, 'phase': 'START_INTENT',
              'request_sha256': request_digest, 'backup_sha256': native.digest_file(attempt/'backup.json')}
    write(state/'in-progress.json', marker)
    marker_digest = native.digest_file(state/'in-progress.json')
    write(former/'intent.json', {'operation': 'AUTHORIZED_FORWARD_PRESTART_PIN_RECOVERY',
          'database_restore': 'FORBIDDEN', 'request_sha256': request_digest, 'start_intent_sha256': marker_digest})
    write(former/'receipt.json', {'operation': 'AUTHORIZED_FORWARD_PRESTART_PIN_RECOVERY', 'attempt': attempt.name,
          'status': 'NATIVE_HEALTH_AND_INGRESS_RESTORED', 'phase': 'START_INTENT_RETAINED',
          'database_restore': 'NOT_RUN', 'generation': 'NOT_COMMITTED', 'journal_preserved': True,
          'backup_sha256': marker['backup_sha256'], 'source_sha': running['source_sha']})
    write(former/'unit-alignment.json', {'status': 'APPLIED', 'only_change': 'EXACT_PRESTART_IMAGE_DIGESTS',
                                          'units': corrected})
    record_path = attempt/'PROVISIONAL_TEST_INPUT.json'
    write(record_path, {'schema_version': 1, 'operation': forward.FORWARD+'_EXECUTOR_REHEARSAL', 'status': 'PASS',
          'executor_source': executor_source, 'previous': migration.new_identity(running),
          'target': migration.new_identity(running), 'checks': forward.FORWARD_EXECUTOR_CHECKS,
          'evidence_level': 'integration-tested', 'execution_environment': 'linux-native-systemd-podman-exact-images',
          'activation': 'NOT_RUN'})
    inventory = contract.canonical({'fixture': 'synthetic-disposable-runner'})
    request = {'schema_version': 1, 'operation': forward.FORWARD, 'observed_at': int(time.time()),
               'attempt': attempt.name, 'marker_sha256': marker_digest, 'request_sha256': request_digest,
               'prior_intent_sha256': native.digest_file(former/'intent.json'),
               'prior_receipt_sha256': native.digest_file(former/'receipt.json'),
               'prior_alignment_sha256': native.digest_file(former/'unit-alignment.json'),
               'inventory_sha256': native.digest_bytes(inventory), 'units': corrected, 'executor_source': executor_source,
               'executor_rehearsal': {'path': str(record_path), 'sha256': native.digest_file(record_path)}}
    raw = contract.canonical(request)
    approval = contract.canonical({'schema_version': 1, 'operation': forward.FORWARD,
                'request_sha256': native.digest_bytes(raw), 'inventory_sha256': request['inventory_sha256'],
                'manifest_sha256': manifest_digest, 'maintenance': 'BOTH_NATIVE_WRITERS_FENCED',
                'recovery': 'PRESERVE_POST_START_WRITES'})
    return request, raw, approval, inventory
