"""Disposable real-systemd executor transition rehearsal, never host activation.

Runs only inside a fresh GitHub-hosted Linux VM, guarded by the fixture's strict preflight. It
uses source-pinned executor functions and unmodified real host adapters against synthetic data.
The test-only provisional input is private to the disposable VM and never exported. This is
not a substitute for operator admission, a production write fence or live acceptance.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time
import uuid

import release_artifact as contract
import release_native as native
import release_native_executor_history_fixture as history
import release_native_executor_journey as journey
import release_native_executor_systemd_fixture as fixture
import release_native_executor_trace as trace
import release_native_forward as forward
import release_native_migration as migration
import release_native_v27_live as live
import release_oci as oci

PHASE = 'PREFLIGHT'


def target(source, anchor, wanted):
    path = Path(source).resolve()
    lines = path.read_text().splitlines()
    positions = [i for i, line in enumerate(lines) if line == anchor]
    contract.require(len(positions) == 1, 'unique crash anchor required')
    index = positions[0]+1
    while index < len(lines) and not lines[index].strip():
        index += 1
    contract.require(index < len(lines) and lines[index] == wanted, 'exact crash boundary required')
    return path, hashlib.sha256(path.read_bytes()).hexdigest(), index+1, wanted


def interrupt(source, function, anchor, wanted, invoke, on_stopped=None):
    path, digest, line, expected = target(source, anchor, wanted)
    return trace.interrupt_at_line(path, digest, function, line, expected, invoke,
                                   timeout=120, on_stopped=on_stopped)


def provisional(path, operation, previous, target_release, source_sha):
    # This record exists only inside the disposable fixture to exercise the closed input gate.
    # It is never copied to the artifact or treated as independently reviewed evidence.
    checks = forward.FORWARD_EXECUTOR_CHECKS if operation == forward.FORWARD else forward.MIGRATION_EXECUTOR_CHECKS
    forward._write_once(path, contract.canonical({'schema_version': 1, 'operation': operation+'_EXECUTOR_REHEARSAL',
        'status': 'PASS', 'executor_source': source_sha, 'previous': migration.new_identity(previous),
        'target': migration.new_identity(target_release), 'checks': checks, 'evidence_level': 'integration-tested',
        'execution_environment': 'linux-native-systemd-podman-exact-images', 'activation': 'NOT_RUN'}))


def record(operation, previous, target_release, source_sha, manifest_digest, provenance):
    checks = forward.FORWARD_EXECUTOR_CHECKS if operation == forward.FORWARD else forward.MIGRATION_EXECUTOR_CHECKS
    result = {'schema_version': 1, 'operation': operation+'_EXECUTOR_REHEARSAL', 'status': 'PASS',
            'executor_source': source_sha, 'previous': migration.new_identity(previous),
            'target': migration.new_identity(target_release), 'checks': checks,
            'evidence_level': 'integration-tested', 'execution_environment': 'linux-native-systemd-podman-exact-images',
            'activation': 'NOT_RUN', 'live_business_acceptance': 'NOT_RUN',
            'fixture_scope': 'synthetic-runner-real-systemd-podman-executor-functions',
            'synthetic_receipt_sha256': provenance}
    if manifest_digest is not None:
        result['target_manifest_sha256'] = manifest_digest
    return result


def foreign_bind_control(system, layout, running):
    name = 'cantos-rehearsal-foreign-'+uuid.uuid4().hex
    reference = running['images']['cantos-server']['ref']
    system.command(['podman', 'run', '-d', '--name', name, '--network=none', '--pull=never', '--read-only',
                    '--cap-drop=ALL', '--security-opt=no-new-privileges', '--user', '10001:10001',
                    '--mount', 'type=bind,src='+layout['databases']['cantos-server']+',dst=/foreign,ro=true',
                    '--entrypoint', '/bin/sleep', reference, '120'])
    try:
        try:
            system.recovery_host(layout, running, running)
        except ValueError as error:
            contract.require(str(error) == 'another container owns a database bind', 'foreign bind refusal mismatch')
        else:
            raise ValueError('foreign database bind accepted')
    finally:
        system.command(['podman', 'rm', '-f', name])


def migration_request(state, prior_request, old, target_release, args):
    current = native.pinned(state/'current.json', None)
    historical = native.private_path(args.historical)
    provisional_path = state/'PROVISIONAL_MIGRATION_TEST_INPUT.json'
    provisional(provisional_path, forward.MIGRATE, old, target_release, args.source_sha)
    inventory = contract.canonical({'fixture': 'synthetic-v27-migration'})
    request = {'schema_version': 1, 'operation': forward.MIGRATE, 'observed_at': int(time.time()),
               'inventory_sha256': native.digest_bytes(inventory), 'current_sha256': native.digest_file(state/'current.json'),
               'generation': current['generation'], 'current_manifest_sha256': current['manifest_sha256'],
               'forward_request_sha256': native.digest_file(state/current['forward_reconciliation']['attempt']/'request.json'),
               'units': prior_request['units'],
               'next': {'manifest_digest': args.manifest_digest, 'source_sha': target_release['source_sha'],
                        'configuration_digest': target_release['configuration_sha256'], 'platform': target_release['platform']},
               'rehearsal': {'path': str(historical), 'sha256': native.digest_file(historical)},
               'executor_source': args.source_sha,
               'executor_rehearsal': {'path': str(provisional_path), 'sha256': native.digest_file(provisional_path)}}
    raw = contract.canonical(request)
    approval = contract.canonical({'schema_version': 1, 'operation': forward.MIGRATE,
        'request_sha256': native.digest_bytes(raw), 'inventory_sha256': request['inventory_sha256'],
        'rehearsal_sha256': request['rehearsal']['sha256'], 'target_manifest_sha256': args.manifest_digest,
        'maintenance': 'BOTH_NATIVE_WRITERS_FENCED', 'recovery': 'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START'})
    return request, raw, approval, inventory


def recovery_approval(request, raw, operation, inventory):
    marker_path = native.STATE/'in-progress.json'
    marker = native.pinned(marker_path, None)
    common = {'schema_version': 1, 'operation': operation, 'request_sha256': native.digest_bytes(raw),
              'attempt': marker['attempt'], 'marker_sha256': native.digest_file(marker_path),
              'inventory_sha256': native.digest_bytes(inventory), 'previous_sha256': request['current_sha256'],
              'maintenance': 'BOTH_NATIVE_WRITERS_FENCED', 'observed_at': int(time.time())}
    if operation == live.ROLLBACK:
        return contract.canonical({**common, 'backup_sha256': marker['backup_sha256'],
                                   'recovery': 'RESTORE_ONLY_BEFORE_ANY_START'})
    return contract.canonical({**common, 'target_manifest_sha256': request['next']['manifest_digest'],
                               'recovery': 'PRESERVE_POST_START_WRITES'})


def run(root, args):
    global PHASE
    system, layout, old23, running24, target27, v23_roots, fixture_marker = fixture.setup(root, args, historical=True)
    system = live.LiveSystem()
    PHASE = 'START_OLD_PAIR'
    system.start(layout, running24)
    system.observe(layout, running24)
    foreign_bind_control(system, layout, running24)
    # The prior incident is synthetic; its v23 backups were made from the actual old images.
    request, raw, approval, inventory = history.create(layout, old23, running24, v23_roots,
                                                        args.source_sha, 'c'*64, fixture_marker)
    marker_bytes = (native.STATE/'in-progress.json').read_bytes()
    provenance = {'historical_start_intent': hashlib.sha256(marker_bytes).hexdigest()}
    PHASE = 'ACCEPT_SYNTHETIC_WRITE'
    actor = journey.signup(system, layout)
    PHASE = 'FORWARD_CRASH_REPLAY'
    interrupt(Path(forward.__file__), 'forward_execute',
              "        contract.require(native.digest_file(state/'in-progress.json') == request['marker_sha256'], 'original marker changed at commit')",
              "        (state/'in-progress.json').unlink()",
              lambda: forward.forward_execute(request, raw, approval, inventory, old23, running24, system))
    contract.require((native.STATE/'in-progress.json').read_bytes() == marker_bytes, 'historical marker not retained')
    result = forward.forward_execute(request, raw, approval, inventory, old23, running24, system)
    contract.require(result['status'] == 'RECONCILED_AFTER_INTERRUPTION' and
                     (native.STATE/result['attempt']/'original-intent.json').read_bytes() == marker_bytes,
                     'forward replay did not preserve marker')
    provenance['forward_receipt'] = native.digest_file(native.STATE/result['attempt']/'receipt.json')
    contract.require(journey.login(system, layout) == actor, 'accepted v24 user did not survive forward recovery')
    PHASE = 'MIGRATION_PRESTART_CRASH'
    mrequest, mraw, mapproval, minventory = migration_request(native.STATE, request, running24, target27, args)
    live.rehearsal(mrequest, running24, target27)
    interrupt(Path(live.__file__), 'execute', '        ports.migrate(layout,target27)',
              '        forward._unit_and_config_fence(prior,original,corrected,metadata)',
              lambda: live.execute(mrequest, mraw, mapproval, minventory, old23, running24, target27, system))
    marker = native.pinned(native.STATE/'in-progress.json', None)
    contract.require(marker['phase'] == 'MIGRATE_INTENT', 'prestart crash boundary mismatch')
    provenance['prestart_cold_backup'] = marker['backup_sha256']
    recovery_inventory = contract.canonical({'fixture': 'prestart-restore'})
    recovery = recovery_approval(mrequest, mraw, live.ROLLBACK, recovery_inventory)
    PHASE = 'ROLLBACK_RENAME_CRASH_REPLAY'
    interrupt(Path(live.__file__), 'rollback',
              '                if not destination.exists():',
              '                    staged[service].rename(destination);native.sync_directory(destination.parent)',
              lambda: live.rollback(mrequest, mraw, recovery, recovery_inventory, old23, running24, target27, system))
    result = live.rollback(mrequest, mraw, recovery, recovery_inventory, old23, running24, target27, system)
    contract.require(result['status'] == 'OBSERVED_V24_RESTORED' and journey.login(system, layout) == actor,
                     'prestart restore did not preserve synthetic user')
    provenance['prestart_rollback_receipt'] = native.digest_file(native.STATE/result['attempt']/'rollback-receipt.json')
    PHASE = 'MIGRATION_POSTSTART_CRASH'
    # A successful login while the child is stopped proves a write was accepted after start.
    accepted = {}
    def write_after_start():
        accepted.update(journey.login_session(system, layout))
    interrupt(Path(live.__file__), 'execute', '        ports.observe(new_layout,target27)',
              "        forward._unit_and_config_fence({'units':new_layout['units']},original,new_units,metadata)",
              lambda: live.execute(mrequest, mraw, mapproval, minventory, old23, running24, target27, system),
              on_stopped=write_after_start)
    contract.require(accepted.get('actor_id') == actor, 'poststart accepted actor mismatch')
    provenance['poststart_intent'] = native.digest_file(native.STATE/'in-progress.json')
    try:
        live.rollback(mrequest, mraw, recovery, recovery_inventory, old23, running24, target27, system)
    except ValueError:
        pass
    else:
        raise ValueError('poststart rollback unexpectedly accepted')
    post_inventory = contract.canonical({'fixture': 'poststart-forward'})
    post_approval = recovery_approval(mrequest, mraw, live.POSTSTART, post_inventory)
    PHASE = 'POSTSTART_FORWARD_RECONCILIATION'
    result = live.poststart(mrequest, mraw, post_approval, post_inventory, old23, running24, target27, system)
    contract.require(result['status'] == 'OBSERVED_FORWARD_RECONCILED', 'poststart reconciliation failed')
    journey.verify_session(system, layout, accepted)
    contract.require(journey.login(system, layout) == actor, 'accepted actor did not survive v27 recovery')
    provenance['poststart_forward_receipt'] = native.digest_file(native.STATE/result['attempt']/'receipt.json')
    sources = {Path(module.__file__).name: native.digest_file(Path(module.__file__))
               for module in (forward, live, fixture, history, journey, trace)}
    # Only after all controls pass may these actual, source/image-bound records leave the VM.
    PHASE = 'WRITE_RESULTS'
    contract.require(args.output.is_absolute() and not args.output.exists() and not args.output.is_symlink() and
                     args.output.parent == args.output.parent.resolve(), 'fresh canonical output required')
    args.output.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='cantos-executor-result-') as temporary:
        stage = Path(temporary)/'result'
        stage.mkdir(mode=0o700)
        first = record(forward.FORWARD, running24, running24, args.source_sha, None, provenance)
        second = record(forward.MIGRATE, running24, target27, args.source_sha, args.manifest_digest, provenance)
        first['source_file_sha256'] = sources
        second['source_file_sha256'] = sources
        contract.write_new(stage/'forward-executor-rehearsal.json', first)
        contract.write_new(stage/'migration-executor-rehearsal.json', second)
        contract.require(stage.stat().st_dev == args.output.parent.stat().st_dev, 'same-filesystem result staging required')
        os.rename(stage, args.output)
        native.sync_directory(args.output.parent)
    return {'status': 'PASS', 'source_sha': args.source_sha, 'activation': 'NOT_RUN',
            'live_business_acceptance': 'NOT_RUN', 'scope': 'synthetic-disposable-real-systemd-executor'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest', 'authfile', 'historical', 'output'):
        parser.add_argument('--'+name, type=Path, required=True)
    for name in ('manifest-digest', 'source-sha', 'configuration-digest', 'platform'):
        parser.add_argument('--'+name, required=True)
    args = parser.parse_args()
    try:
        result = run(Path(__file__).resolve().parents[1], args)
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError, TimeoutError, RuntimeError) as error:
        reason = str(error) if isinstance(error, ValueError) and re.fullmatch(r'[A-Za-z0-9 _:/.-]{1,160}', str(error)) else type(error).__name__
        parser.exit(1, 'disposable executor rehearsal failed in '+PHASE+': '+reason+'; no transition evidence emitted\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
