"""Disposable Linux systemd/Quadlet fixture; not a transition-executor producer.

Run only on a fresh GitHub-hosted runner after the independent prerequisite probe. It creates
synthetic state under the two known fixture roots, two reserved units and a uniquely named
internal Podman network. Never connect this fixture to a real host or real data. The runner is
discarded after the job; this code deliberately performs no deletion of pre-existing objects.
Successful observation remains below the executor-rehearsal gate.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import uuid

import release_native as native
import release_native_executor_fixture as fixture
import release_native_executor_probe as probe
import release_native_forward as forward
import release_native_migration as migration
import release_artifact as contract
import release_oci as oci
import release_oci_build as build
import release_oci_qualify as qualify

PHASE = 'PREFLIGHT'


def setup(root, args, *, historical=False):
    global PHASE
    probe.preflight()
    manifest, _ = qualify.validate_input(root, args)
    qualify.require(qualify.native_host() == manifest['platform'], 'native fixture platform mismatch')
    if historical:
        previous = migration.baseline(root, contract.read_json(root/'infra/oci/historical-v23.json', None))
        running = migration.baseline(root, contract.read_json(root/'infra/oci/historical-v24-8862.json', None))
        migration.transition(previous, running)
        forward.migration_transition(running, manifest)
    else:
        previous = running = manifest
    PHASE = 'CREATE_ROOTS'
    # Every root is absent when checked by preflight; O_EXCL/mkdir refuse races. The runner
    # owns this global Podman store and will be destroyed after the job, not cleaned remotely.
    state = native.STATE
    base = Path(migration.ROOT)
    state.mkdir(mode=0o700)
    fixture_marker = os.urandom(32)
    forward._write_once(state/'disposable-fixture-marker', fixture_marker)
    base.mkdir(mode=0o700)
    (base/'config').mkdir(mode=0o700)
    socket = base/'run'
    socket.mkdir(mode=0o700)
    os.chown(socket, 10002, 10001)
    socket.chmod(0o2750)
    for service, uid in (('cantos-server', 10001), ('cantos-identity', 10002)):
        path = Path(migration.DATABASES[service])
        path.mkdir(mode=0o700)
        os.chown(path, uid, uid)
    native.UNITS.mkdir(mode=0o755, parents=True, exist_ok=True)
    qualify.require(native.UNITS == native.UNITS.resolve() and native.UNITS.is_dir(), 'canonical unit directory required')
    name = 'cantos-rehearsal-'+uuid.uuid4().hex
    system = forward.ForwardSystem()
    # User-provided registry credentials are never copied into units or output.
    PHASE = 'PULL_EXACT_IMAGES'
    pulled = set()
    for release in (previous, running, manifest):
        for service in oci.SERVICES:
            reference = release['images'][service]['ref']
            if reference in pulled:
                continue
            system.command(['podman', 'pull', '--authfile', str(args.authfile), reference])
            inspected = json.loads(system.command(['podman', 'image', 'inspect', reference]))
            qualify.require(len(inspected) == 1 and reference in inspected[0].get('RepoDigests', []), 'image digest mismatch')
            digest = build.check_local_image(service, inspected[0], release['source_sha'],
                                             release['configuration_sha256'], release['platform'])
            qualify.require(digest == release['images'][service]['config_digest'], 'image configuration mismatch')
            pulled.add(reference)
    PHASE = 'CREATE_PRIVATE_NETWORK'
    system.command(['podman', 'network', 'create', '--internal', '--subnet', '10.231.42.0/24', name])
    network = json.loads(system.command(['podman', 'network', 'inspect', name]))
    qualify.require(len(network) == 1, 'unique network required')
    configs, units = {}, {}
    PHASE = 'CREATE_UNITS_AND_MIGRATE'
    for service in oci.SERVICES:
        rendered = fixture.render(service, running['images'][service]['ref'], name)
        env = Path(rendered['environment_path'])
        forward._write_once(env, rendered['environment'])
        configs[str(env)] = native.digest_file(env)
        unit = native.UNITS/(service+'.container')
        forward._write_once(unit, rendered['unit'])
        units[service] = native.digest_file(unit)
        system.database_command(previous, service, Path(migration.DATABASES[service]))
    v23_roots = {}
    if historical:
        baseline_dir = state/'v23-fixture'
        baseline_dir.mkdir(mode=0o700)
        for service in oci.SERVICES:
            v23_roots[service] = baseline_dir/service
            migration.copy_database(Path(migration.DATABASES[service]), v23_roots[service], service)
    for service in oci.SERVICES:
        if historical:
            system.database_command(running, service, Path(migration.DATABASES[service]))
        system.database_command(running, service, Path(migration.DATABASES[service]), check=True)
    request = {'host_machine_sha256': hashlib.sha256(Path('/etc/machine-id').read_bytes()).hexdigest(),
               'databases': migration.DATABASES, 'socket_directory': migration.SOCKET,
               'configuration_files': configs, 'peer_policy': 'direct_peer', 'units': units,
               'health_url': {'kind': 'podman-bridge', 'network': name, 'network_id': network[0]['id'],
                              'subnet': '10.231.42.0/24', 'port': 8080, 'path': '/api/v1/theatre/works'},
               'mounts': {s: [{'source': migration.DATABASES[s], 'destination': '/database', 'read_only': False},
                              {'source': migration.SOCKET, 'destination': '/identity', 'read_only': s == 'cantos-server'}]
                          for s in oci.SERVICES}}
    return system, request, previous, running, manifest, v23_roots, fixture_marker


def run(root, args):
    global PHASE
    system, request, _, manifest, _, _, _ = setup(root, args)
    PHASE = 'SYSTEMD_START'
    system.start(request, manifest)
    PHASE = 'OBSERVE_PAIR'
    system.observe(request, manifest)
    PHASE = 'SYSTEMD_STOP'
    system.stop(request)
    PHASE = 'STOPPED_OWNER_AND_DATABASE_CHECK'
    system.recovery_host(request, manifest, manifest)
    for service in oci.SERVICES:
        system.database_command(manifest, service, Path(migration.DATABASES[service]), check=True)
    return {'status': 'PARTIAL_SYSTEMD_FIXTURE_PASS', 'source_sha': manifest['source_sha'],
            'images': manifest['images'], 'checks': ['real-systemd-start-and-stop', 'real-quadlet-units',
            'real-podman-pair-and-bridge', 'actual-current-schema-check'],
            'executor_transition': 'NOT_RUN', 'crash_replay': 'NOT_RUN', 'activation': 'NOT_RUN'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest', 'authfile'):
        parser.add_argument('--'+name, type=Path, required=True)
    for name in ('manifest-digest', 'source-sha', 'configuration-digest', 'platform'):
        parser.add_argument('--'+name, required=True)
    args = parser.parse_args()
    try:
        result = run(Path(__file__).resolve().parents[1], args)
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        reason = str(error) if isinstance(error, ValueError) and re.fullmatch(r'[A-Za-z0-9 _:/.-]{1,160}', str(error)) else type(error).__name__
        parser.exit(1, 'disposable systemd fixture failed in '+PHASE+': '+reason+'; executor transition remains NOT_RUN\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
