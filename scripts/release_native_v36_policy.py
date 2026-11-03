"""Pure policy for the reviewed initial v27/unfenced to v36/fenced host transition.

This module has no host adapter, command entrypoint, admission or activation side effect.
Its input checks and byte-preserving unit transform are prerequisites for a separately
qualified executor. Existing same-schema/v27 executors retain their original bounds.
"""
from pathlib import PurePosixPath
import re

import release_artifact as contract
import release_native as native
import release_native_adapter as adapter
import release_native_forward as forward
import release_native_migration as migration
import release_native_recovery as recovery
import release_native_v36_rehearsal as rehearsal
import release_oci as oci

OPERATION = 'UPGRADE_NATIVE_V27_V36'
SECURITY_ROOT = '/var/lib/cantos-security'
FLOOR_DESTINATION = '/var/lib/cantos/security-floor'
BARRIER_DESTINATION = '/var/lib/cantos/recovery-control'
PHASES = ('PREPARE', 'STOP_INTENT', 'COLD_CUT', 'COPY_REHEARSAL', 'MIGRATE_INTENT',
          'ENROLL_INTENT', 'UNIT_INTENT', 'START_INTENT', 'OBSERVED', 'COMMITTED')


def security_layout():
    """The exact owner-approved access delta; callers cannot substitute another path/UID."""
    return {
        'root': {'path': SECURITY_ROOT, 'uid': 0, 'gid': 0, 'mode': 0o755},
        'cantos-server': {'path': SECURITY_ROOT+'/server', 'uid': 10001, 'gid': 10001,
                          'mode': 0o700, 'file_mode': 0o600, 'destination': FLOOR_DESTINATION},
        'cantos-identity': {'path': SECURITY_ROOT+'/identity', 'uid': 10002, 'gid': 10002,
                            'mode': 0o700, 'file_mode': 0o600, 'destination': FLOOR_DESTINATION},
        'exposure': {'path': SECURITY_ROOT+'/exposure', 'uid': 0, 'gid': 0, 'mode': 0o755,
                     'file_mode': 0o644, 'destination': BARRIER_DESTINATION, 'read_only': True},
        'operations': {'path': SECURITY_ROOT+'/operations', 'uid': 0, 'gid': 0,
                       'mode': 0o700, 'service_mount': False},
    }


def settings(service, operation):
    contract.require(service in oci.SERVICES, 'exact native owner required')
    recovery.operation(operation)
    prefix = 'CANTOS_SERVER' if service == 'cantos-server' else 'CANTOS_IDENTITY'
    return {prefix+'_RECOVERY_FENCE': FLOOR_DESTINATION+'/current',
            'CANTOS_RECOVERY_BARRIER': BARRIER_DESTINATION+'/barrier',
            'CANTOS_RECOVERY_OPERATION': operation}


def target_unit(data, service, previous, target, operation):
    """Pin the image, remove implicit startup migration and add approved security binds.

    The caller must separately pin/inspect the complete effective environment before any
    write. Config file contents, peer policy, credentials, roster and network are untouched.
    Existing security directives indicate an already-enrolled/foreign profile and refuse.
    """
    desired = settings(service, operation)
    oci.image_ref(previous, service)
    oci.image_ref(target, service)
    for key in (*desired, 'CANTOS_SERVER_RECOVERY_FENCE', 'CANTOS_IDENTITY_RECOVERY_FENCE'):
        contract.require(key.encode() not in data, 'existing recovery configuration refused')
    contract.require(SECURITY_ROOT.encode() not in data and
                     FLOOR_DESTINATION.encode() not in data and BARRIER_DESTINATION.encode() not in data,
                     'existing security mount refused')
    prestarts = [line for line in data.splitlines() if line.startswith(b'ExecStartPre=')]
    uid = 10001 if service == 'cantos-server' else 10002
    prefix = f'ExecStartPre=/usr/bin/podman run --rm --network none --user {uid}:{uid} '
    migration_command = (prefix+'-e CANTOS_SERVER_DATABASE=/var/lib/cantos/db/server.db '
        '-v /root/cantos-native/server-db:/var/lib/cantos/db:Z '
        '--entrypoint /usr/local/bin/cantos-migrate '+previous if service == 'cantos-server' else
        prefix+'--env-file /root/cantos-native/config/identity.env '
        '-v /root/cantos-native/identity-db:/var/lib/cantos/db:Z '+previous+' migrate')
    image_check = 'ExecStartPre=/usr/bin/podman image exists '
    contract.require(len(prestarts) == 1 and prestarts[0] in
                     ((image_check+previous).encode(), migration_command.encode()),
                     'unreviewed pre-start command refused')
    changed = forward.migration_unit(data, previous, target)
    changed = changed.replace(prestarts[0].replace(previous.encode(), target.encode()),
                              (image_check+target).encode(), 1)
    sections = [line for line in changed.splitlines(keepends=True) if line.rstrip(b'\r\n') == b'[Container]']
    contract.require(len(sections) == 1, 'one canonical Container section required')
    section = sections[0]
    contract.require(section.endswith(b'\n'), 'terminated Container section required')
    newline = b'\r\n' if section.endswith(b'\r\n') else b'\n'
    layout = security_layout()
    lines = [f'Environment={key}={value}' for key, value in desired.items()]
    lines += ['Volume='+layout[service]['path']+':'+FLOOR_DESTINATION,
              'Volume='+layout['exposure']['path']+':'+BARRIER_DESTINATION+':ro']
    addition = newline.join(line.encode() for line in lines)+newline
    return changed.replace(section, section+addition, 1)


def current_layout(request):
    """Only the existing named data/socket/config/media profile, with no security mounts yet."""
    contract.require(request['databases'] == migration.DATABASES and
                     request['socket_directory'] == migration.SOCKET and
                     request['peer_policy'] in ('direct_peer', 'trusted_proxy'),
                     'exact current native layout required')
    configs = request['configuration_files']
    contract.require(type(configs) is dict and 1 <= len(configs) <= 32, 'complete current config pins required')
    for name, digest in configs.items():
        contract.require(isinstance(name, str) and name.startswith(migration.ROOT+'/config/') and
                         re.fullmatch('/[A-Za-z0-9_./-]+', name) and '..' not in PurePosixPath(name).parts and
                         str(PurePosixPath(name)) == name, 'canonical current configuration path required')
        contract.digest(digest)
    oci.fields(request['units'], ' '.join(oci.SERVICES), 'exact current unit pair required')
    oci.fields(request['mounts'], ' '.join(oci.SERVICES), 'exact current mount pair required')
    for service in oci.SERVICES:
        contract.digest(request['units'][service])
        native.validate_mount_plan(request, service, media_root=migration.MEDIA+'/', exact_media_root=migration.MEDIA)
    adapter.selector(request['health_url'])


def decide(request, current, baseline, target, now, *, recovery=False):
    """Describe an exact plan; a valid dictionary never claims trusted execution or admission."""
    oci.fields(request, 'schema_version operation observed_at host_machine_sha256 inventory_sha256 '
               'current_sha256 generation current_manifest_sha256 next executor_source security_plan_sha256 '
               'databases socket_directory peer_policy configuration_files units mounts health_url '
               'rehearsal executor_rehearsal', 'exact v27 to v36 request required')
    contract.require(type(request['schema_version']) is int and request['schema_version'] == 1 and
                     request['operation'] == OPERATION and type(request['observed_at']) is int and
                     0 <= now-request['observed_at'] and
                     (recovery or now-request['observed_at'] <= 3600), 'fresh exact v36 transition required')
    for key in ('host_machine_sha256', 'inventory_sha256', 'current_sha256',
                'current_manifest_sha256', 'security_plan_sha256'):
        contract.digest(request[key])
    contract.sha(request['executor_source'])
    contract.require(request['security_plan_sha256'] == contract.fingerprint(security_layout()),
                     'approved security access plan mismatch')
    contract.require(type(request['generation']) is int and request['generation'] >= 2 and
                     current.get('generation') == request['generation'] and
                     current.get('manifest_sha256') == request['current_manifest_sha256'] and
                     current.get('images') == baseline['images'] and
                     current.get('runtime_layout') == {key:request[key] for key in
                                                      ('databases', 'socket_directory', 'peer_policy')},
                     'current native generation mismatch')
    contract.require(request['next']['manifest_digest'] != request['current_manifest_sha256'], 'target must change')
    rehearsal.transition(baseline, target)
    forward.source_bound_target({'next':request['next']}, target)
    current_layout(request)
    for key in ('rehearsal', 'executor_rehearsal'):
        oci.fields(request[key], 'path sha256', 'exact separately reviewed evidence pin required')
        contract.digest(request[key]['sha256'])
    return {'operation': OPERATION, 'status': 'PLAN_ONLY', 'activation': 'NOT_RUN',
            'admission': 'NOT_RUN', 'host_observation': 'NOT_RUN', 'executor_transition': 'NOT_RUN',
            'recovery': 'FORWARD_ONLY_FROM_MIGRATE_INTENT', 'security_provisioning': 'NOT_RUN'}


def may_resume_old(phase):
    contract.require(phase in PHASES, 'unknown migration phase')
    # This is permission to consider a verified unchanged old pair, never an automatic
    # restart. Unknown migration acknowledgement is already forward-only.
    return PHASES.index(phase) < PHASES.index('MIGRATE_INTENT')
