"""Decision boundary for forward-only recovery of the reviewed native adoption profile.

The host-local executor defaults to a plan and requires separate pinned operator approval for
execution. A plan is not admission, a generation, health evidence, or a completed recovery.
"""
import re
import time
from pathlib import Path

import release_artifact as contract
import release_native as native
import release_native_migration as migration
import release_oci as oci

def decode_unique(data):
    """Untrusted operator JSON cannot change meaning across parser boundaries."""
    import json
    def pairs(items):
        result = {}
        for key, value in items:
            contract.require(key not in result, 'duplicate operator JSON key')
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=pairs)


FORWARD = 'RECONCILE_NATIVE_START_INTENT'
MIGRATE = 'UPGRADE_NATIVE_V24_V27'
SCHEMA_27 = 'server-turso-epoch-1-v27'


def forward_unit(original, before, current, old_ref, target_ref):
    """Prove the previously authorized correction changed only one pre-start image pin."""
    contract.require(before == native.unit_image(original, old_ref, target_ref),
                     'saved pre-correction unit differs from adoption replacement')
    old = old_ref.encode('utf-8')
    target = target_ref.encode('utf-8')
    matches = [line for line in before.splitlines() if line.strip().startswith(b'ExecStartPre=') and old in line]
    contract.require(before.count(old) == 1 and len(matches) == 1 and matches[0].count(old) == 1,
                     'one exact pre-start image pin required')
    contract.require(current == before.replace(old, target), 'unit differs from approved pre-start correction')
    return native.digest_bytes(current)


def migration_unit(current, old_ref, target_ref):
    """Update the one container pin and one pre-start pin; preserve all other bytes."""
    old, target = old_ref.encode('utf-8'), target_ref.encode('utf-8')
    matches = [line for line in current.splitlines() if line.strip().startswith(b'ExecStartPre=') and old in line]
    contract.require(current.count(old) == 2 and len(matches) == 1 and matches[0].count(old) == 1,
                     'exact image and pre-start pins required')
    changed = native.unit_image(current, old_ref, target_ref)
    contract.require(changed.count(old) == 1, 'unexpected additional image reference')
    return changed.replace(old, target)


def forward_request(request, original_request, marker, old, running, now=None, *, recovery=False):
    """Validate immutable input bindings. Observations, backup and approval remain separate."""
    oci.fields(request, 'schema_version operation observed_at attempt marker_sha256 request_sha256 '
               'prior_intent_sha256 prior_receipt_sha256 prior_alignment_sha256 inventory_sha256 units executor_source executor_rehearsal',
               'exact forward request required')
    contract.require(type(request['schema_version']) is int and request['schema_version'] == 1 and
                     request['operation'] == FORWARD, 'forward request version or operation')
    now = time.time() if now is None else now
    contract.require(type(request['observed_at']) is int and (recovery or 0 <= now-request['observed_at'] <= 3600),
                     'fresh forward inventory required')
    contract.require(isinstance(request['attempt'], str) and
                     re.fullmatch(r'attempt-[a-f0-9]{32}', request['attempt']), 'exact adoption attempt required')
    for key in ('marker_sha256', 'request_sha256', 'prior_intent_sha256', 'prior_receipt_sha256',
                'prior_alignment_sha256', 'inventory_sha256'):
        contract.digest(request[key])
    contract.sha(request['executor_source'])
    oci.fields(request['executor_rehearsal'], 'path sha256', 'exact executor rehearsal pin required')
    contract.digest(request['executor_rehearsal']['sha256'])
    oci.fields(request['units'], ' '.join(oci.SERVICES), 'exact corrected unit pair required')
    for digest in request['units'].values():
        contract.digest(digest)
    oci.fields(marker, 'attempt operation phase request_sha256 backup_sha256', 'exact adoption intent required')
    contract.require(marker['attempt'] == request['attempt'] and marker['operation'] == migration.OPERATION and
                     marker['phase'] == 'START_INTENT' and marker['request_sha256'] == request['request_sha256'],
                     'original start intent mismatch')
    contract.digest(marker['backup_sha256'])
    migration.decide(original_request, old, running, now, recovery=True)
    return {'operation': FORWARD, 'status': 'PLAN_ONLY', 'admission': 'NOT_RUN',
            'activation': 'NOT_RUN', 'generation': 'NOT_COMMITTED',
            'rollback': 'FORBIDDEN_AFTER_START', 'database_restore': 'NOT_RUN'}


def prior_evidence(request, marker, intent, receipt, alignment, running):
    """Bind the retained prior recovery record; no claim can supply host observation."""
    contract.require(intent.get('operation') == 'AUTHORIZED_FORWARD_PRESTART_PIN_RECOVERY' and
                     intent.get('database_restore') == 'FORBIDDEN' and
                     intent.get('request_sha256') == marker['request_sha256'] and
                     intent.get('start_intent_sha256') == request['marker_sha256'], 'prior forward intent mismatch')
    contract.require(receipt.get('operation') == intent['operation'] and
                     receipt.get('attempt') == request['attempt'] and
                     receipt.get('status') == 'NATIVE_HEALTH_AND_INGRESS_RESTORED' and
                     receipt.get('phase') == 'START_INTENT_RETAINED' and
                     receipt.get('database_restore') == 'NOT_RUN' and
                     receipt.get('generation') == 'NOT_COMMITTED' and
                     receipt.get('journal_preserved') is True and
                     receipt.get('backup_sha256') == marker['backup_sha256'] and
                     receipt.get('source_sha') == running['source_sha'], 'prior forward receipt mismatch')
    contract.require(alignment.get('status') == 'APPLIED' and
                     alignment.get('only_change') == 'EXACT_PRESTART_IMAGE_DIGESTS' and
                     alignment.get('units') == request['units'], 'prior unit alignment mismatch')


def forward_approval(approval, request, request_sha256, manifest_sha256):
    oci.fields(approval, 'schema_version operation request_sha256 inventory_sha256 manifest_sha256 maintenance recovery',
               'exact forward approval required')
    contract.require(type(approval['schema_version']) is int and approval['schema_version'] == 1 and
                     approval['operation'] == FORWARD and approval['request_sha256'] == request_sha256 and
                     approval['inventory_sha256'] == request['inventory_sha256'] and
                     approval['manifest_sha256'] == manifest_sha256 and
                     approval['maintenance'] == 'BOTH_NATIVE_WRITERS_FENCED' and
                     approval['recovery'] == 'PRESERVE_POST_START_WRITES', 'forward operator approval mismatch')



RESUME_FORWARD='RESUME_NATIVE_FORWARD_RECONCILIATION'


def resume_approval(value, request_sha256, prior_attempt, marker_sha256, inventory_sha256, manifest_sha256, *, recovery=False):
    oci.fields(value, 'schema_version operation request_sha256 prior_attempt marker_sha256 inventory_sha256 '
               'manifest_sha256 maintenance recovery observed_at', 'exact forward resume approval required')
    contract.require(type(value['schema_version']) is int and value['schema_version']==1 and
                     value['operation']==RESUME_FORWARD and value['request_sha256']==request_sha256 and
                     value['prior_attempt']==prior_attempt and value['marker_sha256']==marker_sha256 and
                     value['inventory_sha256']==inventory_sha256 and value['manifest_sha256']==manifest_sha256 and
                     value['maintenance']=='BOTH_NATIVE_WRITERS_FENCED' and value['recovery']=='PRESERVE_POST_START_WRITES' and
                     type(value['observed_at']) is int and (recovery or 0 <= time.time()-value['observed_at'] <= 3600),
                     'forward resume operator approval mismatch')

def migration_transition(old, new):
    """Only this exact additive schema transition; preserve all other compatibility fields."""
    contract.require(old['platform'] == new['platform'], 'migration platform mismatch')
    before, after = old['compatibility'], new['compatibility']
    contract.require(before.get('server_schema') == migration.SCHEMA_24 and after.get('server_schema') == SCHEMA_27 and
                     before == {**after, 'server_schema': migration.SCHEMA_24},
                     'only reviewed v24 to v27 transition supported')


def migration_approval(approval, request_sha256, inventory_sha256, rehearsal_sha256, manifest_sha256):
    oci.fields(approval, 'schema_version operation request_sha256 inventory_sha256 rehearsal_sha256 '
               'target_manifest_sha256 maintenance recovery', 'exact migration approval required')
    contract.require(type(approval['schema_version']) is int and approval['schema_version'] == 1 and
                     approval['operation'] == MIGRATE and approval['request_sha256'] == request_sha256 and
                     approval['inventory_sha256'] == inventory_sha256 and
                     approval['rehearsal_sha256'] == rehearsal_sha256 and
                     approval['target_manifest_sha256'] == manifest_sha256 and
                     approval['maintenance'] == 'BOTH_NATIVE_WRITERS_FENCED' and
                     approval['recovery'] == 'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START',
                     'migration operator approval mismatch')



FORWARD_EXECUTOR_CHECKS = ['exact-images-process-and-private-bridge','systemd-quadlet-and-mounts',
    'foreign-owner-refused','current-cold-pair-and-restore-check','historical-intent-preserved',
    'failure-and-replay-preserve-accepted-writes']
MIGRATION_EXECUTOR_CHECKS = ['exact-images-process-and-private-bridge','systemd-quadlet-and-mounts',
    'foreign-owner-refused','v24-to-v27-current-cold-pair-and-restore-check',
    'prestart-rollback-crash-replay','poststart-forward-preserves-accepted-writes']


def executor_rehearsal(request, operation, previous, target, *, path=None):
    """A separate reviewed real-host transition record, never a synthetic model result."""
    pins=request['executor_rehearsal']
    oci.fields(pins, 'path sha256', 'exact executor rehearsal pin required')
    record=native.pinned(path or pins['path'],pins['sha256'])
    contract.require(operation in (FORWARD,MIGRATE), 'unknown executor rehearsal operation')
    checks=FORWARD_EXECUTOR_CHECKS if operation==FORWARD else MIGRATION_EXECUTOR_CHECKS
    contract.require(type(record.get('schema_version')) is int and record.get('schema_version')==1 and record.get('operation')==operation+'_EXECUTOR_REHEARSAL' and
                     record.get('status')=='PASS' and record.get('executor_source')==request['executor_source'] and
                     record.get('previous')==migration.new_identity(previous) and
                     record.get('target')==migration.new_identity(target) and record.get('checks')==checks and
                     record.get('evidence_level')=='integration-tested' and
                     record.get('execution_environment')=='linux-native-systemd-podman-exact-images' and
                     record.get('activation')=='NOT_RUN', 'exact executor and bridge rehearsal required')
    return record

def source_bound_target(original, running):
    """An admitted release must be the exact target named by the historical request."""
    target = original['next']
    contract.require(target['source_sha'] == running['source_sha'] and
                     target['configuration_digest'] == running['configuration_sha256'] and
                     target['platform'] == running['platform'], 'historical target release mismatch')


def verify_old_custody(state, request, old, running, *, marker_path=None, recovery=False, unit_override=None):
    """Read all retained state through private paths. No write, pull, stop or start."""
    contract.require(isinstance(request.get('attempt'), str) and re.fullmatch(r'attempt-[a-f0-9]{32}', request['attempt']),
                     'exact adoption attempt required')
    marker = native.pinned(marker_path or state/'in-progress.json', request['marker_sha256'])
    attempt = native.private_path(state/request['attempt'], directory=True)
    original = native.pinned(attempt/'request.json', request['request_sha256'])
    contract.require(original['baseline'] == old, 'historical baseline mismatch')
    source_bound_target(original, running)
    saved_approval = native.pinned(attempt/'adoption-approval.json', None)
    migration.approval(saved_approval, original, request['request_sha256'])
    forward_request(request, original, marker, old, running, recovery=recovery)
    former = native.private_path(attempt/'forward-recovery', directory=True)
    intent = native.pinned(former/'intent.json', request['prior_intent_sha256'])
    receipt = native.pinned(former/'receipt.json', request['prior_receipt_sha256'])
    alignment = native.pinned(former/'unit-alignment.json', request['prior_alignment_sha256'])
    prior_evidence(request, marker, intent, receipt, alignment, running)
    backup = native.pinned(attempt/'backup.json', marker['backup_sha256'])
    contract.require(backup.get('baseline') == old and backup.get('request_sha256') == marker['request_sha256'] and
                     backup.get('cut') == 'BOTH_OWNERS_STOPPED', 'historical backup provenance mismatch')
    oci.fields(backup['databases'], ' '.join(oci.SERVICES), 'complete historical backup pair required')
    for service in oci.SERVICES:
        contract.require(migration.data_inventory(attempt/(service+'-database'), service) == backup['databases'][service],
                         'historical backup integrity mismatch')
    metadata = native.pinned(attempt/'unit-metadata.json', None)
    oci.fields(metadata, ' '.join(oci.SERVICES), 'exact historical unit metadata required')
    corrected = {}
    for service in oci.SERVICES:
        old_data = native.private_path(attempt/(service+'.container')).read_bytes()
        before = native.private_path(former/(service+'.before')).read_bytes()
        current_path = native.private_path(native.UNITS/(service+'.container'))
        current = current_path.read_bytes() if unit_override is None else unit_override[service]
        digest = forward_unit(old_data, before, current, old['images'][service]['ref'], running['images'][service]['ref'])
        contract.require(digest == request['units'][service] and native.unit_metadata(current_path) == metadata[service],
                         'corrected unit identity or metadata mismatch')
        corrected[service] = current
    config_metadata = native.pinned(attempt/'configuration-metadata.json', None)
    contract.require(set(config_metadata) == set(original['configuration_files']), 'historical configuration inventory mismatch')
    config_copy = native.private_path(attempt/'configuration', directory=True)
    for index, (name, digest) in enumerate(sorted(original['configuration_files'].items())):
        record = config_metadata[name]
        oci.fields(record, 'uid gid mode sha256 copy', 'exact saved configuration metadata required')
        path = native.private_path(name)
        saved = native.private_path(config_copy/str(index))
        contract.require(record['copy'] == str(index) and record['sha256'] == digest and
                         native.digest_file(path) == digest and native.digest_file(saved) == digest and
                         native.unit_metadata(path) == {k:record[k] for k in ('uid','gid','mode')},
                         'historical configuration custody or metadata drift')
    return marker, original, backup, corrected, metadata


def _unit_and_config_fence(request, original, corrected, metadata):
    for service in oci.SERVICES:
        path = native.private_path(native.UNITS/(service+'.container'))
        contract.require(path.read_bytes() == corrected[service] and
                         native.digest_bytes(corrected[service]) == request['units'][service] and
                         native.unit_metadata(path) == metadata[service], 'corrected unit drift')
    for name, digest in original['configuration_files'].items():
        contract.require(native.digest_file(native.private_path(name)) == digest, 'runtime configuration drift')


def _current_layout(original, request):
    """Keep the existing mount, peer policy, health selector and config; only units differ."""
    return {**original, 'units': request['units']}


def _write_once(path, data):
    """A durable, non-overwriting private record in a newly owned attempt directory."""
    import os
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    native.sync_directory(path.parent)


class ForwardSystem(migration.AdoptionSystem):
    def recovery_host(self, request, old, new):
        import json
        super().recovery_host(request, old, new)
        # Reject another container holding a database bind even without a currently open FD.
        running=json.loads(self.command(['podman','ps','--format','json']))
        for row in running:
            names=row.get('Names',[])
            if isinstance(names,str): names=[names]
            if any(name in oci.SERVICES for name in names): continue
            identity=row.get('Id','')
            contract.require(re.fullmatch(r'[a-f0-9]{64}',identity), 'bounded container identity required')
            other=json.loads(self.command(['podman','container','inspect',identity]))[0]
            for mount in other['Mounts']:
                if mount.get('Type') == 'bind':
                    source=Path(mount['Source'])
                    contract.require(all(source != Path(db) and source not in Path(db).parents and Path(db) not in source.parents
                                         for db in request['databases'].values()), 'another container owns a database bind')

    def ensure_capacity(self, request):
        import os
        import shutil
        sizes=[p.stat().st_size for root in request['databases'].values() for p in Path(root).iterdir() if p.is_file()]
        capacity=os.statvfs(self.state)
        contract.require(shutil.disk_usage(self.state).free >= sum(sizes)*5+64*1024**2 and
                         capacity.f_favail >= len(sizes)*5+100, 'insufficient current backup capacity')

    def verify_current(self, request, running):
        for service in oci.SERVICES:
            self.database_command(running, service, Path(request['databases'][service]), check=True)


def forward_execute(request, request_bytes, approval_bytes, inventory_bytes, old, running, ports, *, state=native.STATE,
                    resume_attempt=None, resume_approval_bytes=None, resume_inventory_bytes=None):
    """One explicitly approved forward-finalization attempt; never restore or migrate data.

    A crash leaves the original marker intact until a separately bound receipt and generation
    both exist. Re-entry with a complete current/receipt pair may only verify running state and
    remove that exact old marker; a partial or foreign pair refuses.
    """
    import uuid
    request_digest = native.digest_bytes(request_bytes)
    contract.require(decode_unique(request_bytes) == request and
                     native.digest_bytes(inventory_bytes) == request['inventory_sha256'], 'request or inventory drift')
    approval = decode_unique(approval_bytes)
    with native.locked(state):
        marker, original, backup, corrected, metadata = verify_old_custody(state, request, old, running, recovery=resume_attempt is not None)
        manifest_digest = original['next']['manifest_digest']
        forward_approval(approval, request, request_digest, manifest_digest)
        executor_rehearsal(request, FORWARD, running, running)
        if resume_attempt is not None:
            contract.require(isinstance(resume_attempt,str) and re.fullmatch(r'forward-[a-f0-9]{32}',resume_attempt) and
                             resume_approval_bytes is not None and resume_inventory_bytes is not None,
                             'exact prior forward attempt and new approval required')
            prior=native.private_path(state/resume_attempt,directory=True)
            contract.require(native.digest_file(native.private_path(prior/'request.json'))==request_digest and
                             native.digest_file(native.private_path(prior/'approval.json'))==native.digest_bytes(approval_bytes) and
                             native.digest_file(native.private_path(prior/'inventory.json'))==request['inventory_sha256'] and
                             native.digest_file(native.private_path(prior/'original-intent.json'))==request['marker_sha256'],
                             'prior forward attempt custody mismatch')
            phase=native.pinned(prior/'phase-stop-intent.json',None)
            contract.require(phase.get('operation')==FORWARD and phase.get('phase')=='STOP_INTENT' and
                             phase.get('request_sha256')==request_digest and
                             phase.get('original_marker_sha256')==request['marker_sha256'], 'prior stop intent mismatch')
            resume_approval(decode_unique(resume_approval_bytes),request_digest,resume_attempt,
                            request['marker_sha256'],native.digest_bytes(resume_inventory_bytes),manifest_digest)
        layout = _current_layout(original, request)
        current_path = state/'current.json'
        if current_path.exists():
            current = native.pinned(current_path, None)
            details = current.get('forward_reconciliation')
            contract.require(isinstance(details, dict) and set(details) == {'attempt', 'receipt_sha256'} and
                             isinstance(details['attempt'], str) and
                             re.fullmatch(r'forward-[a-f0-9]{32}', details['attempt']), 'foreign current generation')
            attempt = native.private_path(state/details['attempt'], directory=True)
            receipt = native.pinned(attempt/'receipt.json', details['receipt_sha256'])
            contract.require(receipt.get('operation') == FORWARD and receipt.get('status') == 'OBSERVED_FORWARD_RECONCILED' and
                             receipt.get('request_sha256') == request_digest and
                             receipt.get('approval_sha256') == native.digest_bytes(approval_bytes) and
                             receipt.get('inventory_sha256') == request['inventory_sha256'] and
                             receipt.get('original_marker_sha256') == request['marker_sha256'] and
                             receipt.get('manifest_sha256') == manifest_digest and receipt.get('images') == running['images'] and
                             current.get('generation') == 1 and current.get('manifest_sha256') == manifest_digest and
                             current.get('images') == running['images'], 'incomplete or foreign forward generation')
            contract.require(native.digest_file(native.private_path(attempt/'request.json')) == request_digest and
                             native.digest_file(native.private_path(attempt/'approval.json')) == native.digest_bytes(approval_bytes) and
                             native.digest_file(native.private_path(attempt/'inventory.json')) == request['inventory_sha256'] and
                             native.digest_file(native.private_path(attempt/'executor-rehearsal.json')) == request['executor_rehearsal']['sha256'] and
                             native.digest_file(native.private_path(attempt/'original-intent.json')) == request['marker_sha256'],
                             'forward custody mismatch')
            cold = native.pinned(attempt/'backup.json', receipt['current_backup_sha256'])
            phase = native.pinned(attempt/'phase-start-intent.json', None)
            contract.require(cold.get('cut') == 'BOTH_OWNERS_STOPPED' and cold.get('restore') == 'NOT_RUN' and
                             cold.get('request_sha256') == request_digest and phase.get('operation') == FORWARD and
                             phase.get('phase') == 'START_INTENT' and phase.get('recovery') == 'FORWARD_ONLY' and
                             phase.get('backup_sha256') == receipt['current_backup_sha256'] and
                             current.get('adoption_request_sha256') == marker['request_sha256'] and
                             current.get('baseline') == old and current.get('adoption_approval_sha256') ==
                             native.digest_file(native.private_path(state/request['attempt']/'adoption-approval.json')) and
                             current.get('runtime_layout') ==
                             {'databases':original['databases'], 'socket_directory':original['socket_directory'],
                              'peer_policy':original['peer_policy']}, 'forward receipt or cold backup provenance mismatch')
            oci.fields(cold['databases'], ' '.join(oci.SERVICES), 'complete current backup pair required')
            for service in oci.SERVICES:
                contract.require(migration.data_inventory(attempt/(service+'-database'), service) == cold['databases'][service],
                                 'current cold backup integrity mismatch')
            ports.observe(layout, running)
            _unit_and_config_fence(request, original, corrected, metadata)
            contract.require(native.digest_file(state/'in-progress.json') == request['marker_sha256'], 'original marker changed')
            (state/'in-progress.json').unlink()
            native.sync_directory(state)
            return {'operation': FORWARD, 'status': 'RECONCILED_AFTER_INTERRUPTION', 'generation': 1,
                    'attempt': details['attempt'], 'live_business_acceptance': 'NOT_RUN'}

        if resume_attempt is None:
            ports.observe(layout, running)
        else:
            ports.recovery_host(layout, running, running)
            ports.ensure_capacity(layout)
        _unit_and_config_fence(request, original, corrected, metadata)
        attempt = state/('forward-'+uuid.uuid4().hex)
        attempt.mkdir(mode=0o700)
        native.sync_directory(state)
        _write_once(attempt/'request.json', request_bytes)
        _write_once(attempt/'approval.json', approval_bytes)
        _write_once(attempt/'inventory.json', inventory_bytes)
        _write_once(attempt/'executor-rehearsal.json', native.private_path(request['executor_rehearsal']['path']).read_bytes())
        native.pinned(attempt/'executor-rehearsal.json', request['executor_rehearsal']['sha256'])
        if resume_attempt is not None:
            _write_once(attempt/'resume-approval.json',resume_approval_bytes)
            _write_once(attempt/'resume-inventory.json',resume_inventory_bytes)
            _write_once(attempt/'prior-attempt.json',contract.canonical({'attempt':resume_attempt}))
        _write_once(attempt/'original-intent.json', native.private_path(state/'in-progress.json').read_bytes())
        _write_once(attempt/'phase-stop-intent.json', contract.canonical({'operation': FORWARD, 'phase': 'STOP_INTENT',
                     'request_sha256': request_digest, 'original_marker_sha256': request['marker_sha256']}))
        ports.stop(layout)
        # Both databases may contain writes accepted since the old START_INTENT. Never restore
        # the historical backup, and prove owners are stopped before copying current bytes.
        new_marker, new_original, new_backup, new_units, new_metadata = verify_old_custody(state, request, old, running, recovery=resume_attempt is not None)
        contract.require((new_marker, new_original, new_backup, new_units, new_metadata) ==
                         (marker, original, backup, corrected, metadata) and not current_path.exists(),
                         'historical state drift after stop')
        ports.recovery_host(layout, running, running)
        ports.ensure_capacity(layout)
        copies = {}
        for service in oci.SERVICES:
            copies[service] = migration.copy_database(Path(layout['databases'][service]), attempt/(service+'-database'), service)
        _write_once(attempt/'backup.json', contract.canonical({'databases':copies, 'cut':'BOTH_OWNERS_STOPPED',
                     'request_sha256':request_digest, 'restore':'NOT_RUN'}))
        for service in oci.SERVICES:
            migration.copy_database(attempt/(service+'-database'), attempt/(service+'-restore-check'), service, copies[service])
        restored = {**layout, 'databases': {service:str(attempt/(service+'-restore-check')) for service in oci.SERVICES}}
        ports.verify_current(restored, running)
        ports.verify_current(layout, running)
        _unit_and_config_fence(request, original, corrected, metadata)
        contract.require(native.digest_file(state/'in-progress.json') == request['marker_sha256'] and not current_path.exists(),
                         'original intent or generation drift before start')
        _write_once(attempt/'phase-start-intent.json', contract.canonical({'operation':FORWARD, 'phase':'START_INTENT',
                     'backup_sha256':native.digest_file(attempt/'backup.json'), 'recovery':'FORWARD_ONLY'}))
        ports.start(layout, running)
        ports.observe(layout, running)
        _unit_and_config_fence(request, original, corrected, metadata)
        contract.require(native.digest_file(state/'in-progress.json') == request['marker_sha256'] and not current_path.exists(),
                         'original intent or generation drift after start')
        receipt = {'operation':FORWARD, 'status':'OBSERVED_FORWARD_RECONCILED', 'request_sha256':request_digest,
                   'original_marker_sha256':request['marker_sha256'], 'historical_backup_sha256':marker['backup_sha256'],
                   'current_backup_sha256':native.digest_file(attempt/'backup.json'), 'manifest_sha256':manifest_digest,
                   'images':running['images'], 'approval_sha256':native.digest_bytes(approval_bytes),
                   'inventory_sha256':request['inventory_sha256'], 'live_business_acceptance':'NOT_RUN', 'database_restore':'NOT_RUN'}
        if resume_attempt is not None:
            receipt.update(prior_forward_attempt=resume_attempt, resume_approval_sha256=native.digest_bytes(resume_approval_bytes),
                           resume_inventory_sha256=native.digest_bytes(resume_inventory_bytes))
        _write_once(attempt/'receipt.json', contract.canonical(receipt))
        current = {'generation':1, 'manifest_sha256':manifest_digest, 'images':running['images'],
                   'adoption_request_sha256':marker['request_sha256'], 'baseline':old,
                   'adoption_approval_sha256':native.digest_file(state/request['attempt']/'adoption-approval.json'),
                   'runtime_layout':{'databases':original['databases'], 'socket_directory':original['socket_directory'],
                                     'peer_policy':original['peer_policy']},
                   'forward_reconciliation':{'attempt':attempt.name, 'receipt_sha256':native.digest_file(attempt/'receipt.json')}}
        native.atomic(current_path, contract.canonical(current))
        contract.require(native.digest_file(state/'in-progress.json') == request['marker_sha256'], 'original marker changed at commit')
        (state/'in-progress.json').unlink()
        native.sync_directory(state)
        return {'operation':FORWARD, 'status':'OBSERVED_FORWARD_RECONCILED', 'generation':1,
                'attempt':attempt.name, 'live_business_acceptance':'NOT_RUN'}


def main():
    """Host-local plan by default. Execution requires a source pin and separate approval."""
    import argparse
    import json
    import subprocess
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request', required=True, type=Path)
    parser.add_argument('--request-digest', required=True)
    parser.add_argument('--inventory', required=True, type=Path)
    parser.add_argument('--inventory-digest', required=True)
    parser.add_argument('--executor-source', required=True)
    parser.add_argument('--approval', type=Path)
    parser.add_argument('--approval-digest')
    parser.add_argument('--execute', action='store_true')
    parser.add_argument('--resume-attempt')
    parser.add_argument('--resume-approval', type=Path)
    parser.add_argument('--resume-approval-digest')
    parser.add_argument('--resume-inventory', type=Path)
    parser.add_argument('--resume-inventory-digest')
    args = parser.parse_args()
    try:
        contract.sha(args.executor_source)
        if args.resume_attempt is not None:
            contract.require(args.execute and all((args.resume_approval,args.resume_approval_digest,
                                                   args.resume_inventory,args.resume_inventory_digest)),
                             'resume requires explicit execution and separate pinned approval and inventory')
        else:
            contract.require(not any((args.resume_approval,args.resume_approval_digest,
                                      args.resume_inventory,args.resume_inventory_digest)), 'resume inputs require an attempt')
        contract.digest(args.inventory_digest)
        root = Path(__file__).resolve().parents[1]
        contract.require(oci.git(root, 'rev-parse', 'HEAD').decode().strip() == args.executor_source and
                         not oci.git(root, 'status', '--porcelain', '--untracked-files=normal').strip(),
                         'clean exact reviewed executor source required')
        request = native.pinned(args.request, args.request_digest)
        contract.require(request['executor_source']==args.executor_source, 'executor request source mismatch')
        contract.require(args.inventory_digest == request['inventory_sha256'], 'inventory digest mismatch')
        native.pinned(args.inventory, args.inventory_digest)
        contract.require(isinstance(request.get('attempt'), str) and re.fullmatch(r'attempt-[a-f0-9]{32}', request['attempt']),
                         'exact adoption attempt required')
        attempt = native.private_path(native.STATE/request['attempt'], directory=True)
        original = native.pinned(attempt/'request.json', request['request_sha256'])
        old = migration.baseline(root, original['baseline'])
        running = native.release(root, original['next'])
        migration.rehearsal(original, old, running)
        marker, _, _, _, _ = verify_old_custody(native.STATE, request, old, running, recovery=args.resume_attempt is not None)
        decision = forward_request(request, original, marker, old, running, recovery=args.resume_attempt is not None)
        executor_rehearsal(request, FORWARD, running, running)
        if args.execute:
            contract.require(args.approval is not None and args.approval_digest is not None,
                             'separately pinned forward approval required')
            approval = native.pinned(args.approval, args.approval_digest)
            forward_approval(approval, request, args.request_digest, original['next']['manifest_digest'])
            request_bytes, approval_bytes, inventory_bytes = (args.request.read_bytes(), args.approval.read_bytes(),
                                                              args.inventory.read_bytes())
            contract.require(native.digest_bytes(request_bytes) == args.request_digest and
                             native.digest_bytes(approval_bytes) == args.approval_digest and
                             native.digest_bytes(inventory_bytes) == args.inventory_digest, 'operator inputs changed')
            resume_approval_bytes=resume_inventory_bytes=None
            if args.resume_attempt is not None:
                native.pinned(args.resume_approval,args.resume_approval_digest)
                native.pinned(args.resume_inventory,args.resume_inventory_digest)
                resume_approval_bytes=args.resume_approval.read_bytes()
                resume_inventory_bytes=args.resume_inventory.read_bytes()
                contract.require(native.digest_bytes(resume_approval_bytes)==args.resume_approval_digest and
                                 native.digest_bytes(resume_inventory_bytes)==args.resume_inventory_digest,
                                 'resume inputs changed')
            decision = forward_execute(request, request_bytes, approval_bytes, inventory_bytes,
                                       old, running, ForwardSystem(), resume_attempt=args.resume_attempt,
                                       resume_approval_bytes=resume_approval_bytes, resume_inventory_bytes=resume_inventory_bytes)
        print(json.dumps(decision, sort_keys=True))
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError):
        parser.exit(1, 'forward reconciliation refused or interrupted; preserve both journals and inspect before retry\n')


if __name__ == '__main__':
    main()
