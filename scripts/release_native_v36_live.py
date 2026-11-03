"""Initial v27/Identity4 to v36/Identity7 host upgrade, below independent admission.

The command is host-local and defaults to an offline plan. It never restores a database,
resets a floor, rotates an epoch on ordinary upgrade, changes ingress, or bootstraps a host.
Every potentially partial migration is forward-only. Operator approvals and an attributable
systemd/exact-image executor rehearsal are required separately from image qualification.
"""
import argparse
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import time
import uuid

import release_artifact as contract
import release_native as native
import release_native_adapter as adapter
import release_native_forward as forward
import release_native_migration as migration
import release_native_recovery as recovery
import release_native_v27_live as previous_live
import release_native_v36_policy as policy
import release_native_v36_rehearsal as historical
import release_oci as oci

CHECKS = ['exact-v27-v36-systemd-quadlet-process-and-private-bridge',
          'foreign-owner-and-drift-refused-before-stop', 'pre-migration-abort-keeps-current-unenrolled-pair',
          'current-cold-pair-and-copy-migration',
          'initial-owner-floors-and-blocked-exposure', 'forward-only-migration-crash-replay',
          'enrollment-crash-replay-without-floor-reset', 'start-crash-preserves-accepted-writes',
          'preserved-auth-and-revoked-session', 'ordinary-restart-keeps-valid-session']


def rehearsal(request, old, new):
    record = native.pinned(request['rehearsal']['path'], request['rehearsal']['sha256'])
    contract.require(record.get('operation') == historical.OPERATION and record.get('status') == 'PASS' and
        record.get('baseline') == migration.new_identity(old) and record.get('target') == migration.new_identity(new) and
        record.get('input_manifest_sha256') == request['next']['manifest_digest'] and
        record.get('checks') == historical.CHECKS and record.get('evidence_level') == 'integration-tested' and
        record.get('execution_environment') == 'linux-native-exact-images' and
        record.get('host_adapter') == {'status':'PASS', 'source_sha':new['source_sha'], 'images':new['images'],
                                    'checks':adapter.CHECKS, 'activation':'NOT_RUN'},
        'exact v27 to v36 historical rehearsal required')
    pins = request['executor_rehearsal']
    record = native.pinned(pins['path'], pins['sha256'])
    contract.require(record.get('schema_version') == 1 and type(record['schema_version']) is int and
        record.get('operation') == policy.OPERATION+'_EXECUTOR_REHEARSAL' and record.get('status') == 'PASS' and
        record.get('executor_source') == request['executor_source'] and
        record.get('target_manifest_sha256') == request['next']['manifest_digest'] and
        record.get('previous') == migration.new_identity(old) and record.get('target') == migration.new_identity(new) and
        record.get('checks') == CHECKS and record.get('evidence_level') == 'integration-tested' and
        record.get('execution_environment') == 'linux-native-systemd-podman-exact-images' and
        record.get('activation') == 'NOT_RUN', 'exact v36 host executor rehearsal required')


def approval(value, request, digest):
    oci.fields(value, 'schema_version operation request_sha256 inventory_sha256 target_manifest_sha256 '
               'rehearsal_sha256 executor_rehearsal_sha256 security_plan_sha256 maintenance recovery',
               'exact v36 operator approval required')
    expected = {'schema_version':1, 'operation':policy.OPERATION, 'request_sha256':digest,
        'inventory_sha256':request['inventory_sha256'], 'target_manifest_sha256':request['next']['manifest_digest'],
        'rehearsal_sha256':request['rehearsal']['sha256'],
        'executor_rehearsal_sha256':request['executor_rehearsal']['sha256'],
        'security_plan_sha256':request['security_plan_sha256'], 'maintenance':'BOTH_NATIVE_WRITERS_FENCED',
        'recovery':'FORWARD_ONLY_FROM_MIGRATE_INTENT'}
    contract.require(type(value['schema_version']) is int and value == expected,
                     'approval must bind exact target, security access, evidence and forward-only boundary')


def phase(state, attempt, request_digest, name):
    contract.require(name in policy.PHASES, 'unknown v36 phase')
    value = {'operation':policy.OPERATION, 'attempt':attempt.name,
             'request_sha256':request_digest, 'phase':name}
    native.atomic(state/'in-progress.json', contract.canonical(value))
    retain(attempt/('phase-'+name.lower()+'.json'), contract.canonical(value))
    return value


def retain(path, data):
    """A retry may acknowledge identical immutable bytes, never replace prior evidence."""
    if path.exists():
        contract.require(native.private_path(path).read_bytes() == data, 'retained v36 evidence drift')
    else:
        forward._write_once(path, data)


def pinned_units(request, saved, metadata, operation, old, new, *, allow_target=False):
    for service in oci.SERVICES:
        path = native.private_path(native.UNITS/(service+'.container'))
        data = saved[service]
        contract.require(native.digest_bytes(data) == request['units'][service] and
                         native.unit_metadata(path) == metadata[service], 'original unit custody drift')
        allowed = [data]
        if allow_target:
            allowed.append(policy.target_unit(data, service, old['images'][service]['ref'],
                                              new['images'][service]['ref'], operation))
        contract.require(path.read_bytes() in allowed, 'v36 unit drift')
    for name, digest in request['configuration_files'].items():
        contract.require(native.digest_file(native.private_path(name)) == digest, 'v36 runtime config drift')


def verify_backup(attempt, request_digest):
    backup = native.pinned(attempt/'backup.json', None)
    contract.require(backup.get('cut') == 'BOTH_OWNERS_STOPPED' and backup.get('request_sha256') == request_digest and
                     backup.get('restore') == 'FORBIDDEN', 'v36 cold cut provenance mismatch')
    oci.fields(backup['databases'], ' '.join(oci.SERVICES), 'complete v36 cold cut required')
    for service in oci.SERVICES:
        contract.require(migration.data_inventory(attempt/(service+'-database'), service) == backup['databases'][service],
                         'v36 immutable cold cut drift')
    return backup


class InitialSystem(previous_live.LiveSystem):
    """The known rootful adapter, with only the approved owner-floor/exposure access delta."""
    def provision(self, operation):
        for item in policy.security_layout().values():
            path = Path(item['path'])
            if not path.exists():
                path.mkdir(mode=item['mode'])
                os.chown(path, item['uid'], item['gid'])
                path.chmod(item['mode'])
                native.sync_directory(path.parent)
            meta = path.lstat()
            contract.require(path == path.resolve() and stat.S_ISDIR(meta.st_mode) and
                (meta.st_uid,meta.st_gid,stat.S_IMODE(meta.st_mode)) == (item['uid'],item['gid'],item['mode']),
                'partial security provisioning drift')
        barrier = Path(policy.SECURITY_ROOT)/'exposure/barrier'
        if barrier.exists():
            self.security(operation)
        recovery.barrier(barrier, operation)

    def security(self, operation):
        for item in policy.security_layout().values():
            path = Path(item['path'])
            meta = path.lstat()
            contract.require(path == path.resolve() and stat.S_ISDIR(meta.st_mode) and
                             (meta.st_uid, meta.st_gid, stat.S_IMODE(meta.st_mode)) ==
                             (item['uid'], item['gid'], item['mode']), 'security access plan drift')
        barrier = Path(policy.SECURITY_ROOT)/'exposure/barrier'
        meta = barrier.lstat()
        contract.require(stat.S_ISREG(meta.st_mode) and meta.st_uid == 0 and meta.st_gid == 0 and
                         stat.S_IMODE(meta.st_mode) == 0o644 and meta.st_nlink == 1 and
                         barrier.read_bytes() in [self.barrier_bytes(operation, ready) for ready in (False, True)],
                         'foreign security exposure barrier')

    @staticmethod
    def barrier_bytes(operation, ready):
        return ('CANTOS_RECOVERY_BARRIER_V1\n'+('ready' if ready else 'blocked')+' '+operation+'\n').encode()

    def exposure(self, operation, ready=False):
        self.security(operation)
        recovery.barrier(Path(policy.SECURITY_ROOT)/'exposure/barrier', operation, ready=ready)

    def observe(self, request, old):
        super().observe(request, old)
        for service in oci.SERVICES:
            env = self.inspect(service)['Config']['Env']
            contract.require(not any(item.split('=',1)[0] in
                ('CANTOS_SERVER_RECOVERY_FENCE','CANTOS_IDENTITY_RECOVERY_FENCE',
                 'CANTOS_RECOVERY_BARRIER','CANTOS_RECOVERY_OPERATION') for item in env),
                'initial transition refuses already configured recovery authority')

    def observe_target(self, request, new):
        migration.AdoptionSystem.observe(self, request, new)

    def check_container(self, request, service, container, images):
        # Validate the security binds first, then reuse the original closed database/socket
        # and process validator on a view containing exactly the original mounts.
        extra = [m for m in container['Mounts'] if m.get('Source','').startswith(policy.SECURITY_ROOT+'/')]
        if extra:
            expected = {(policy.security_layout()[service]['path'], policy.FLOOR_DESTINATION, True),
                        (policy.SECURITY_ROOT+'/exposure', policy.BARRIER_DESTINATION, False)}
            contract.require(len(extra) == 2 and
                {(m['Source'],m['Destination'],m['RW']) for m in extra} == expected and
                all(m['Type'] == 'bind' for m in extra), 'wrong owner floor or exposure mount')
            self.security(request['security_operation'])
            env = container['Config']['Env']
            for key, expected_value in policy.settings(service, request['security_operation']).items():
                contract.require([x.split('=',1)[1] for x in env if x.startswith(key+'=')] == [expected_value],
                                 'effective recovery environment drift')
        else:
            contract.require('security_operation' not in request, 'missing target security mounts')
        clean = {**container, 'Mounts':[m for m in container['Mounts'] if m not in extra]}
        super().check_container(request, service, clean, images)

    def database_command(self, release, service, directory, *, check=False, enrolled=False):
        if not enrolled:
            return super().database_command(release, service, directory, check=check)
        self.owner_command(release, service, directory, 'observe')
        contract.require(check, 'post-enrollment schema changes require a separate reviewed executor')

    def owner_command(self, release, service, directory, action):
        contract.require(action in ('enroll','observe','reconcile'), 'initial-upgrade owner action refused')
        server = service == 'cantos-server'
        key = 'CANTOS_SERVER' if server else 'CANTOS_IDENTITY'
        entry = '/usr/local/bin/cantos-migrate' if server else '/usr/local/bin/cantos-identity'
        flag = '--recovery-'+action if server else 'recovery-'+action
        raw = self.command(['podman','run','--rm','--pull=never','--network=none','--read-only','--cap-drop=ALL',
            '--security-opt=no-new-privileges', '--mount',f'type=bind,src={directory},dst=/database',
            '--mount',f"type=bind,src={policy.security_layout()[service]['path']},dst=/floor",
            '--env',key+'_DATABASE=/database/'+('server.db' if server else 'identity.db'),
            '--env',key+'_RECOVERY_FENCE=/floor/current','--entrypoint',entry,release['images'][service]['ref'],flag])
        return recovery.proof(forward.decode_unique(raw), service)

    def enroll_or_reconcile(self, release, service, directory):
        floor = Path(policy.security_layout()[service]['path'])/'current'
        # A present Pending floor is reconciled only by the owner against its committed
        # database marker. A missing floor around an enrolled database makes enroll refuse.
        action = 'reconcile' if floor.exists() else 'enroll'
        return self.owner_command(release, service, directory, action)


def execute(request, request_bytes, approval_bytes, inventory_bytes, old, new, ports, *, state=native.STATE):
    """One approved initial transition. Interrupted attempts refuse; no automatic replay."""
    digest = native.digest_bytes(request_bytes)
    contract.require(forward.decode_unique(request_bytes) == request and
                     native.digest_bytes(inventory_bytes) == request['inventory_sha256'], 'v36 input custody drift')
    approval(forward.decode_unique(approval_bytes), request, digest)
    with native.locked(state):
        contract.require(not (state/'in-progress.json').exists(), 'interrupted v36 operation requires forward reconciliation')
        current_bytes = native.private_path(state/'current.json').read_bytes()
        contract.require(native.digest_bytes(current_bytes) == request['current_sha256'], 'current generation drift')
        current = forward.decode_unique(current_bytes)
        policy.decide(request, current, old, new, int(time.time()))
        rehearsal(request, old, new)
        contract.require(not Path(policy.SECURITY_ROOT).exists() and not Path(policy.SECURITY_ROOT).is_symlink(),
                         'initial upgrade refuses an existing security authority')
        ports.observe(request, old)
        saved = {s:native.private_path(native.UNITS/(s+'.container')).read_bytes() for s in oci.SERVICES}
        metadata = {s:native.unit_metadata(native.UNITS/(s+'.container')) for s in oci.SERVICES}
        operation = uuid.uuid4().hex
        targets = {s:policy.target_unit(saved[s], s, old['images'][s]['ref'], new['images'][s]['ref'], operation)
                   for s in oci.SERVICES}
        pinned_units(request, saved, metadata, operation, old, new)
        # Pull/read back while the old pair remains healthy. A failure here creates no host
        # operation marker, stops no owner, and provisions no security directory.
        ports.pull(new)
        ports.observe(request, old)
        pinned_units(request, saved, metadata, operation, old, new)
        attempt = state/('v36-'+operation)
        attempt.mkdir(mode=0o700)
        native.sync_directory(state)
        for name, data in [('request.json',request_bytes),('approval.json',approval_bytes),
                           ('inventory.json',inventory_bytes),('previous.json',current_bytes),
                           ('unit-metadata.json',contract.canonical(metadata))]:
            forward._write_once(attempt/name, data)
        for service in oci.SERVICES:
            forward._write_once(attempt/(service+'.container'), saved[service])
            forward._write_once(attempt/(service+'.target.container'), targets[service])
        phase(state, attempt, digest, 'PREPARE')
        phase(state, attempt, digest, 'STOP_INTENT')
        ports.stop(request)
        ports.require_stopped(request)
        ports.recovery_host(request, old, new)
        ports.ensure_capacity(request)
        pinned_units(request, saved, metadata, operation, old, new)
        contract.require(native.digest_file(state/'current.json') == request['current_sha256'], 'generation changed after stop')
        copies = {s:migration.copy_database(Path(request['databases'][s]), attempt/(s+'-database'), s) for s in oci.SERVICES}
        forward._write_once(attempt/'backup.json', contract.canonical({'cut':'BOTH_OWNERS_STOPPED',
            'request_sha256':digest,'restore':'FORBIDDEN','databases':copies}))
        phase(state, attempt, digest, 'COLD_CUT')
        # Rehearse against complete current copies, preserving the immutable original cut.
        phase(state, attempt, digest, 'COPY_REHEARSAL')
        for service in oci.SERVICES:
            trial = attempt/(service+'-trial')
            migration.copy_database(attempt/(service+'-database'), trial, service, copies[service])
            for _ in range(2):
                ports.database_command(new, service, trial)
            ports.database_command(new, service, trial, check=True)
        verify_backup(attempt, digest)
        pinned_units(request, saved, metadata, operation, old, new)
        ports.require_stopped(request)
        # Once this intent is durable, any partial prefix or unknown command result requires
        # compatible forward recovery. No code in this owner restores old database bytes.
        phase(state, attempt, digest, 'MIGRATE_INTENT')
        for service in oci.SERVICES:
            path = Path(request['databases'][service])
            ports.database_command(new, service, path)
            ports.database_command(new, service, path, check=True)
        phase(state, attempt, digest, 'ENROLL_INTENT')
        ports.provision(operation)
        proofs = {}
        for service in oci.SERVICES:
            proofs[service] = ports.owner_command(new, service, Path(request['databases'][service]), 'enroll')
            forward._write_once(attempt/(service+'-enrollment.json'), contract.canonical(proofs[service]))
        for service in oci.SERVICES:
            recovery.same(ports.owner_command(new, service, Path(request['databases'][service]), 'observe'), proofs[service])
        verify_backup(attempt, digest)
        ports.require_stopped(request)
        pinned_units(request, saved, metadata, operation, old, new)
        phase(state, attempt, digest, 'UNIT_INTENT')
        for service in oci.SERVICES:
            native.replace_unit(native.UNITS/(service+'.container'), targets[service], metadata[service])
        pinned_units(request, saved, metadata, operation, old, new, allow_target=True)
        phase(state, attempt, digest, 'START_INTENT')
        # All owner checks completed while both services were stopped. Opening the barrier
        # binds the same new operation in both units; a failed start closes it again.
        ports.exposure(operation, ready=True)
        layout = {**request, 'security_operation':operation,
                  'units':{s:native.digest_bytes(targets[s]) for s in oci.SERVICES}}
        try:
            ports.start(layout, new)
            ports.observe_target(layout, new)
            pinned_units(request, saved, metadata, operation, old, new, allow_target=True)
        except BaseException:
            ports.exposure(operation, ready=False)
            raise
        phase(state, attempt, digest, 'OBSERVED')
        receipt = {'schema_version':1, 'operation':policy.OPERATION, 'status':'OBSERVED_V36_UPGRADE',
            'request_sha256':digest,'approval_sha256':native.digest_bytes(approval_bytes),
            'inventory_sha256':request['inventory_sha256'],'previous_sha256':request['current_sha256'],
            'manifest_sha256':request['next']['manifest_digest'],'images':new['images'],
            'backup_sha256':native.digest_file(attempt/'backup.json'),'security_operation':operation,
            'security_plan_sha256':request['security_plan_sha256'],'database_restore':'NOT_RUN',
            'live_business_acceptance':'NOT_RUN'}
        forward._write_once(attempt/'receipt.json', contract.canonical(receipt))
        next_current = {'generation':request['generation']+1,'manifest_sha256':request['next']['manifest_digest'],
            'images':new['images'],'previous_manifest_sha256':current['manifest_sha256'],
            'runtime_layout':{k:request[k] for k in ('databases','socket_directory','peer_policy')},
            'security':{'operation':operation,'plan_sha256':request['security_plan_sha256']},
            'v36_upgrade':{'attempt':attempt.name,'receipt_sha256':native.digest_file(attempt/'receipt.json')}}
        contract.require(native.digest_file(state/'current.json') == request['current_sha256'], 'generation changed before commit')
        native.atomic(state/'current.json', contract.canonical(next_current))
        phase(state, attempt, digest, 'COMMITTED')
        (state/'in-progress.json').unlink()
        native.sync_directory(state)
        return {'operation':policy.OPERATION,'status':'OBSERVED_V36_UPGRADE',
                'attempt':attempt.name,'generation':next_current['generation'],'live_business_acceptance':'NOT_RUN'}


def resume_forward(request, request_bytes, resume_bytes, inventory_bytes, old, new, ports, *, state=native.STATE):
    """Explicit fresh approval, same source/target and attempt; no old-byte restoration.

    Before migration intent this path deliberately refuses. After intent it preserves every
    current file and floor, repeats only owner-idempotent operations, then observes the pair.
    Enrollment uncertainty with no committed next database marker stays closed.
    """
    digest = native.digest_bytes(request_bytes)
    contract.require(forward.decode_unique(request_bytes) == request, 'v36 resume request drift')
    value = forward.decode_unique(resume_bytes)
    oci.fields(value, 'schema_version operation request_sha256 attempt marker_sha256 inventory_sha256 '
               'observed_at target_manifest_sha256 security_plan_sha256 recovery', 'exact v36 forward approval required')
    contract.require(type(value['schema_version']) is int and value['schema_version'] == 1 and
        value['operation'] == policy.OPERATION+'_FORWARD_RESUME' and value['request_sha256'] == digest and
        re.fullmatch(r'v36-[a-f0-9]{32}',value['attempt']) and
        type(value['observed_at']) is int and 0 <= int(time.time())-value['observed_at'] <= 3600 and
        value['inventory_sha256'] == native.digest_bytes(inventory_bytes) and
        value['target_manifest_sha256'] == request['next']['manifest_digest'] and
        value['security_plan_sha256'] == request['security_plan_sha256'] and
        value['recovery'] == 'PRESERVE_CURRENT_PAIR_AND_EXTERNAL_FLOORS', 'fresh v36 forward approval mismatch')
    contract.digest(value['marker_sha256'])
    with native.locked(state):
        marker = native.pinned(state/'in-progress.json',value['marker_sha256'])
        oci.fields(marker,'operation attempt request_sha256 phase','exact v36 intent required')
        contract.require(marker['operation'] == policy.OPERATION and marker['attempt'] == value['attempt'] and
                         marker['request_sha256'] == digest and marker['phase'] in policy.PHASES and
                         not policy.may_resume_old(marker['phase']), 'v36 forward recovery requires migration intent')
        attempt = native.private_path(state/value['attempt'],directory=True)
        operation = attempt.name.removeprefix('v36-')
        contract.require(native.private_path(attempt/'request.json').read_bytes() == request_bytes and
            native.digest_file(attempt/'inventory.json') == request['inventory_sha256'] and
            native.digest_file(attempt/'previous.json') == request['current_sha256'], 'v36 retained input custody drift')
        approval(native.pinned(attempt/'approval.json',None),request,digest)
        previous = native.pinned(attempt/'previous.json',request['current_sha256'])
        # Freshness comes from the separate bound recovery approval, never by rewriting the
        # original request or its observation time. All original policy checks still apply.
        policy.decide(request,previous,old,new,int(time.time()),recovery=True)
        rehearsal(request,old,new)
        saved = {s:native.private_path(attempt/(s+'.container')).read_bytes() for s in oci.SERVICES}
        metadata = native.pinned(attempt/'unit-metadata.json',None)
        targets = {s:policy.target_unit(saved[s],s,old['images'][s]['ref'],new['images'][s]['ref'],operation)
                   for s in oci.SERVICES}
        for service in oci.SERVICES:
            contract.require(native.private_path(attempt/(service+'.target.container')).read_bytes() == targets[service],
                             'v36 retained target unit drift')
        stage = policy.PHASES.index(marker['phase'])
        pinned_units(request,saved,metadata,operation,old,new,allow_target=True)
        verify_backup(attempt,digest)
        layout = {**request,'security_operation':operation,
                  'units':{s:native.digest_bytes(targets[s]) for s in oci.SERVICES}}
        current = native.pinned(state/'current.json',None)
        committed = current.get('v36_upgrade')
        if committed is not None:
            receipt_digest = native.digest_file(attempt/'receipt.json')
            contract.require(stage >= policy.PHASES.index('OBSERVED') and
                committed == {'attempt':attempt.name,'receipt_sha256':receipt_digest} and
                current.get('generation') == request['generation']+1 and current.get('images') == new['images'] and
                current.get('manifest_sha256') == request['next']['manifest_digest'] and
                current.get('previous_manifest_sha256') == request['current_manifest_sha256'] and
                current.get('security') == {'operation':operation,'plan_sha256':request['security_plan_sha256']} and
                current.get('runtime_layout') == {k:request[k] for k in ('databases','socket_directory','peer_policy')},
                'foreign or incomplete v36 generation')
            expected = native.pinned(attempt/'receipt.json',receipt_digest)
            contract.require(expected.get('request_sha256') == digest and expected.get('images') == new['images'] and
                             expected.get('manifest_sha256') == request['next']['manifest_digest'] and
                             expected.get('database_restore') == 'NOT_RUN', 'v36 receipt mismatch')
            ports.observe_target(layout,new)
            (state/'in-progress.json').unlink()
            native.sync_directory(state)
            return {'status':'RECONCILED_COMMITTED_V36','attempt':attempt.name,'generation':current['generation']}
        contract.require(native.digest_file(state/'current.json') == request['current_sha256'], 'v36 previous generation drift')
        if stage >= policy.PHASES.index('START_INTENT'):
            for service in oci.SERVICES:
                contract.require(native.private_path(native.UNITS/(service+'.container')).read_bytes() == targets[service],
                                 'poststart unit drift requires separate reconciliation')
        ports.pull(new)
        recovery_attempt = attempt/('forward-'+uuid.uuid4().hex)
        recovery_attempt.mkdir(mode=0o700)
        for name,data in [('approval.json',resume_bytes),('inventory.json',inventory_bytes),
                          ('original-marker.json',contract.canonical(marker))]:
            forward._write_once(recovery_attempt/name,data)
        # Running owners are allowed only after START_INTENT and must be the exact new
        # fenced pair. They may contain accepted writes; no old copy is ever installed.
        if stage >= policy.PHASES.index('START_INTENT'):
            ports.recovery_host(layout,new,new)
            ports.exposure(operation,ready=False)
            ports.stop(layout)
        ports.require_stopped(request)
        pinned_units(request,saved,metadata,operation,old,new,allow_target=True)
        if stage == policy.PHASES.index('MIGRATE_INTENT'):
            contract.require(not Path(policy.SECURITY_ROOT).exists(), 'unexpected authority before enrollment intent')
            for service in oci.SERVICES:
                path = Path(request['databases'][service])
                ports.database_command(new,service,path)
                ports.database_command(new,service,path,check=True)
            phase(state,attempt,digest,'ENROLL_INTENT')
            stage = policy.PHASES.index('ENROLL_INTENT')
        if stage == policy.PHASES.index('ENROLL_INTENT'):
            ports.provision(operation)
        else:
            ports.security(operation)
            ports.exposure(operation,ready=False)
        for service in oci.SERVICES:
            observed = ports.enroll_or_reconcile(new,service,Path(request['databases'][service]))
            path = attempt/(service+'-enrollment.json')
            if not path.exists():
                contract.require(stage == policy.PHASES.index('ENROLL_INTENT'), 'missing original enrollment evidence')
                forward._write_once(path,contract.canonical(observed))
            elif stage <= policy.PHASES.index('UNIT_INTENT'):
                recovery.same(observed,native.pinned(path,None))
            # After a start intent, revisions may have advanced. The real owner validates
            # the CURRENT external floor; comparing it with a cold counter would lose writes.
            recovery.same(ports.owner_command(new,service,Path(request['databases'][service]),'observe'),observed)
        if stage <= policy.PHASES.index('UNIT_INTENT'):
            phase(state,attempt,digest,'UNIT_INTENT')
            for service in oci.SERVICES:
                native.replace_unit(native.UNITS/(service+'.container'),targets[service],metadata[service])
        for service in oci.SERVICES:
            contract.require(native.private_path(native.UNITS/(service+'.container')).read_bytes() == targets[service],
                             'exact new unit pair required before forward start')
        ports.require_stopped(request)
        # No rewind of the durable phase. The new attempt records its own start intent.
        retain(recovery_attempt/'start-intent.json',contract.canonical({'operation':operation,'recovery':'FORWARD_ONLY'}))
        if stage < policy.PHASES.index('START_INTENT'):
            phase(state,attempt,digest,'START_INTENT')
        try:
            ports.exposure(operation,ready=True)
            ports.start(layout,new)
            ports.observe_target(layout,new)
        except BaseException:
            ports.exposure(operation,ready=False)
            raise
        receipt = {'schema_version':1,'operation':policy.OPERATION,'status':'OBSERVED_V36_UPGRADE',
            'request_sha256':digest,'approval_sha256':native.digest_file(attempt/'approval.json'),
            'inventory_sha256':request['inventory_sha256'],'previous_sha256':request['current_sha256'],
            'manifest_sha256':request['next']['manifest_digest'],'images':new['images'],
            'backup_sha256':native.digest_file(attempt/'backup.json'),'security_operation':operation,
            'security_plan_sha256':request['security_plan_sha256'],'database_restore':'NOT_RUN',
            'live_business_acceptance':'NOT_RUN'}
        retain(attempt/'receipt.json',contract.canonical(receipt))
        next_current = {'generation':request['generation']+1,'manifest_sha256':request['next']['manifest_digest'],
            'images':new['images'],'previous_manifest_sha256':request['current_manifest_sha256'],
            'runtime_layout':{k:request[k] for k in ('databases','socket_directory','peer_policy')},
            'security':{'operation':operation,'plan_sha256':request['security_plan_sha256']},
            'v36_upgrade':{'attempt':attempt.name,'receipt_sha256':native.digest_file(attempt/'receipt.json')}}
        contract.require(native.digest_file(state/'current.json') == request['current_sha256'], 'v36 generation drift at forward commit')
        native.atomic(state/'current.json',contract.canonical(next_current))
        forward._write_once(recovery_attempt/'receipt.json',contract.canonical({'status':'OBSERVED_V36_FORWARD_RECONCILED',
            'request_sha256':digest,'prior_marker_sha256':value['marker_sha256'],'database_restore':'NOT_RUN',
            'new_current_sha256':native.digest_file(state/'current.json')}))
        (state/'in-progress.json').unlink()
        native.sync_directory(state)
        return {'status':'OBSERVED_V36_FORWARD_RECONCILED','attempt':attempt.name,'generation':next_current['generation']}


def abort_before_migrate(request, request_bytes, approval_bytes, inventory_bytes, old, new, ports, *, state=native.STATE):
    """Explicitly resume only the unchanged old owner pair before any migration intent.

    No cold copy is installed. Even a partial migration or enrollment makes the old owner
    check refuse. Original units/config/current state must still match their retained bytes.
    """
    digest=native.digest_bytes(request_bytes)
    contract.require(forward.decode_unique(request_bytes)==request,'v36 abort request drift')
    value=forward.decode_unique(approval_bytes)
    oci.fields(value,'schema_version operation request_sha256 attempt marker_sha256 inventory_sha256 '
               'observed_at target_manifest_sha256 security_plan_sha256 recovery','exact v36 abort approval required')
    contract.require(type(value['schema_version']) is int and value['schema_version']==1 and
        value['operation']==policy.OPERATION+'_ABORT_BEFORE_MIGRATE' and value['request_sha256']==digest and
        isinstance(value['attempt'],str) and re.fullmatch(r'v36-[a-f0-9]{32}',value['attempt']) and
        type(value['observed_at']) is int and 0<=int(time.time())-value['observed_at']<=3600 and
        value['inventory_sha256']==native.digest_bytes(inventory_bytes) and
        value['target_manifest_sha256']==request['next']['manifest_digest'] and
        value['security_plan_sha256']==request['security_plan_sha256'] and
        value['recovery']=='UNCHANGED_UNENROLLED_PAIR_ONLY','fresh v36 abort approval mismatch')
    contract.digest(value['marker_sha256'])
    with native.locked(state):
        marker=native.pinned(state/'in-progress.json',value['marker_sha256'])
        oci.fields(marker,'operation attempt request_sha256 phase','exact v36 abort intent required')
        contract.require(marker['operation']==policy.OPERATION and marker['attempt']==value['attempt'] and
            marker['request_sha256']==digest and policy.may_resume_old(marker['phase']),
            'old image resume forbidden from migration intent onward')
        attempt=native.private_path(state/value['attempt'],directory=True)
        operation=attempt.name.removeprefix('v36-')
        contract.require(native.private_path(attempt/'request.json').read_bytes()==request_bytes and
            native.digest_file(attempt/'inventory.json')==request['inventory_sha256'] and
            native.digest_file(attempt/'previous.json')==request['current_sha256'] and
            native.digest_file(state/'current.json')==request['current_sha256'] and
            not Path(policy.SECURITY_ROOT).exists() and not Path(policy.SECURITY_ROOT).is_symlink(),
            'abort requires unchanged generation and no new authority')
        approval(native.pinned(attempt/'approval.json',None),request,digest)
        policy.decide(request,native.pinned(attempt/'previous.json',None),old,new,int(time.time()),recovery=True)
        saved={s:native.private_path(attempt/(s+'.container')).read_bytes() for s in oci.SERVICES}
        metadata=native.pinned(attempt/'unit-metadata.json',None)
        pinned_units(request,saved,metadata,operation,old,new)
        ports.recovery_host(request,old,old)
        ports.pull(old)
        recovery_attempt=attempt/('abort-'+uuid.uuid4().hex);recovery_attempt.mkdir(mode=0o700)
        for name,data in [('approval.json',approval_bytes),('inventory.json',inventory_bytes),
                          ('original-marker.json',contract.canonical(marker))]:
            forward._write_once(recovery_attempt/name,data)
        ports.stop(request)
        ports.require_stopped(request)
        for service in oci.SERVICES:
            ports.database_command(old,service,Path(request['databases'][service]),check=True)
        pinned_units(request,saved,metadata,operation,old,new)
        forward._write_once(recovery_attempt/'start-intent.json',contract.canonical({'images':old['images'],
            'database_restore':'FORBIDDEN','request_sha256':digest}))
        ports.start(request,old)
        ports.observe(request,old)
        pinned_units(request,saved,metadata,operation,old,new)
        contract.require(native.digest_file(state/'in-progress.json')==value['marker_sha256'] and
            native.digest_file(state/'current.json')==request['current_sha256'],'abort generation or intent drift')
        forward._write_once(recovery_attempt/'receipt.json',contract.canonical({'status':'OBSERVED_UNCHANGED_V27_ABORT',
            'request_sha256':digest,'marker_sha256':value['marker_sha256'],'database_restore':'NOT_RUN',
            'generation':request['generation']}))
        (state/'in-progress.json').unlink();native.sync_directory(state)
        return {'status':'OBSERVED_UNCHANGED_V27_ABORT','attempt':attempt.name,'generation':request['generation']}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('request','inventory'):
        parser.add_argument('--'+name, required=True, type=Path)
        parser.add_argument('--'+name+'-digest', required=True)
    parser.add_argument('--executor-source', required=True)
    parser.add_argument('--approval', type=Path)
    parser.add_argument('--approval-digest')
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--resume-forward', action='store_true')
    parser.add_argument('--abort-before-migrate', action='store_true')
    for name in ('resume-approval','resume-inventory'):
        parser.add_argument('--'+name,type=Path)
        parser.add_argument('--'+name+'-digest')
    args = parser.parse_args()
    try:
        root = Path(__file__).resolve().parents[1]
        contract.sha(args.executor_source)
        contract.require(oci.git(root,'rev-parse','HEAD').decode().strip() == args.executor_source and
                         not oci.git(root,'status','--porcelain','--untracked-files=normal').strip(),
                         'clean exact reviewed executor source required')
        request = native.pinned(args.request, args.request_digest)
        contract.require(request['executor_source'] == args.executor_source and
                         request['inventory_sha256'] == args.inventory_digest, 'executor or inventory identity mismatch')
        inventory = native.private_path(args.inventory).read_bytes()
        contract.require(native.digest_bytes(inventory) == args.inventory_digest, 'inventory bytes drift')
        old = migration.baseline(root, contract.read_json(root/'infra/oci/historical-v27.json',None))
        new, _ = native.release(root, request['next'])
        resume_inputs=(args.resume_approval,args.resume_approval_digest,args.resume_inventory,args.resume_inventory_digest)
        contract.require(not (args.resume_forward and args.abort_before_migrate),'one explicit recovery action required')
        recovering=args.resume_forward or args.abort_before_migrate
        if recovering:
            contract.require(args.execute and all(resume_inputs), 'explicit execution and fresh recovery inputs required')
            resume = native.private_path(args.resume_approval).read_bytes()
            fresh = native.private_path(args.resume_inventory).read_bytes()
            contract.require(native.digest_bytes(resume) == args.resume_approval_digest and
                             native.digest_bytes(fresh) == args.resume_inventory_digest, 'fresh recovery input digest mismatch')
            action=resume_forward if args.resume_forward else abort_before_migrate
            result = action(request,native.private_path(args.request).read_bytes(),resume,fresh,old,new,InitialSystem())
        else:
            contract.require(not any(resume_inputs), 'recovery inputs require explicit forward resume')
            current = native.pinned(native.STATE/'current.json', request['current_sha256'])
            result = policy.decide(request,current,old,new,int(time.time()))
            rehearsal(request,old,new)
        if args.execute and not recovering:
            contract.require(args.approval is not None and args.approval_digest is not None,
                             'separate exact operator approval required')
            raw = native.private_path(args.approval).read_bytes()
            contract.require(native.digest_bytes(raw) == args.approval_digest, 'approval bytes drift')
            result = execute(request,native.private_path(args.request).read_bytes(),raw,inventory,old,new,InitialSystem())
        print(json.dumps(result,sort_keys=True))
    except (ValueError,OSError,KeyError,TypeError,subprocess.SubprocessError) as error:
        reason = str(error) if isinstance(error,ValueError) and re.fullmatch(r'[A-Za-z0-9 _:/.-]{1,160}',str(error)) else type(error).__name__
        parser.exit(1,'v36 host transition refused: '+reason+'; retained intent requires reconciliation\n')


if __name__ == '__main__':
    main()
