"""Candidate v24 -> v27 native-host lane; explicit approval, never implicit deployment.

The exact old and new images, rehearsal, host observation, backup and maintenance approval are
separate boundaries. Any interruption retains the operation marker and requires reconciliation;
no backup is automatically restored after a start intent.
"""
import re
import time
from pathlib import Path

import release_artifact as contract
import release_native as native
import release_native_adapter as adapter
import release_native_forward as forward
import release_native_migration as migration
import release_oci as oci

CHECKS = ['v24-populated-http-auth-and-revoked-cookie', 'v27-serve-before-migrate-refused',
          'complete-cold-pair-backup', 'v24-to-v27-migrate', 'repeated-migrate',
          'v27-engine-integrity-and-foreign-keys', 'v27-preserved-http-auth-and-revoked-cookie',
          'restore-both-v24-roots', 'v24-restored-http-auth-and-revoked-cookie']


def decide(request, current, old, new, now=None):
    oci.fields(request, 'schema_version operation observed_at inventory_sha256 current_sha256 '
               'generation current_manifest_sha256 forward_request_sha256 units next rehearsal executor_source executor_rehearsal',
               'exact v24 to v27 request required')
    now = time.time() if now is None else now
    contract.require(type(request['schema_version']) is int and request['schema_version'] == 1 and
                     request['operation'] == forward.MIGRATE and type(request['observed_at']) is int and
                     0 <= now-request['observed_at'] <= 3600, 'fresh v24 to v27 request required')
    for name in ('inventory_sha256','current_sha256','current_manifest_sha256','forward_request_sha256'):
        contract.digest(request[name])
    contract.require(type(request['generation']) is int and request['generation'] == 1 and
                     current.get('generation') == request['generation'] and
                     current.get('manifest_sha256') == request['current_manifest_sha256'] and
                     current.get('images') == old['images'], 'current generation or image pair mismatch')
    contract.sha(request['executor_source'])
    oci.fields(request['executor_rehearsal'], 'path sha256', 'exact executor rehearsal pin required')
    contract.digest(request['executor_rehearsal']['sha256'])
    oci.fields(request['units'], ' '.join(oci.SERVICES), 'exact current unit pair required')
    for digest in request['units'].values(): contract.digest(digest)
    oci.fields(request['rehearsal'], 'path sha256', 'exact rehearsal pin required')
    contract.digest(request['rehearsal']['sha256'])
    forward.migration_transition(old,new)
    forward.source_bound_target({'next':request['next']},new)
    contract.require(request['next']['manifest_digest'] != request['current_manifest_sha256'], 'target must change')
    return {'operation':forward.MIGRATE,'status':'PLAN_ONLY','activation':'NOT_RUN',
            'admission':'NOT_RUN','migration':'NOT_RUN','recovery':'BEFORE_START_ONLY'}


def rehearsal(request, old, new):
    """Require actual observed v24 pair, not another image sharing its schema label."""
    pins=request['rehearsal'];record=native.pinned(pins['path'],pins['sha256'])
    contract.require(record.get('operation') == 'HISTORICAL_V24_V27_REHEARSAL' and
                     record.get('status') == 'PASS' and record.get('baseline') == migration.new_identity(old) and
                     record.get('target') == migration.new_identity(new) and
                     record.get('input_manifest_sha256') == request['next']['manifest_digest'] and
                     record.get('evidence_level') == 'integration-tested' and
                     record.get('execution_environment') == 'linux-native-exact-images' and
                     record.get('checks') == CHECKS, 'exact live-baseline v24 to v27 rehearsal required')
    contract.require(record.get('host_adapter') == {'status':'PASS','source_sha':new['source_sha'],
                     'images':new['images'],'checks':adapter.CHECKS,'activation':'NOT_RUN'},
                     'exact real bridge adapter evidence required')
    return record


def current_custody(state, request, old23, current24, *, allow_marker=False, unit_override=None, current_path=None):
    """Revalidate forward-finalized state and retained historical evidence under host lock."""
    if not allow_marker:
        contract.require(not (state/'in-progress.json').exists(), 'interrupted operation requires reconciliation')
    current=native.pinned(current_path or state/'current.json',request['current_sha256'])
    info=current.get('forward_reconciliation')
    contract.require(isinstance(info,dict) and set(info)=={'attempt','receipt_sha256'} and
                     isinstance(info['attempt'],str) and re.fullmatch(r'forward-[a-f0-9]{32}',info['attempt']),
                     'forward generation evidence required')
    directory=native.private_path(state/info['attempt'],directory=True)
    receipt=native.pinned(directory/'receipt.json',info['receipt_sha256'])
    f_request=native.pinned(directory/'request.json',request['forward_request_sha256'])
    contract.require(receipt.get('operation')==forward.FORWARD and receipt.get('status')=='OBSERVED_FORWARD_RECONCILED' and
                     receipt.get('request_sha256')==request['forward_request_sha256'] and
                     receipt.get('original_marker_sha256')==f_request['marker_sha256'] and
                     receipt.get('manifest_sha256')==current['manifest_sha256'] and receipt.get('images')==current24['images'] and
                     receipt.get('database_restore')=='NOT_RUN' and current.get('adoption_request_sha256')==f_request['request_sha256'],
                     'forward receipt mismatch')
    contract.require(native.digest_file(native.private_path(directory/'inventory.json')) == f_request['inventory_sha256'],
                     'forward inventory custody mismatch')
    marker, original, _, corrected, metadata = forward.verify_old_custody(
        state,f_request,old23,current24,marker_path=directory/'original-intent.json',recovery=True,unit_override=unit_override)
    contract.digest(receipt['approval_sha256'])
    approval = native.pinned(directory/'approval.json',receipt['approval_sha256'])
    forward.executor_rehearsal(f_request,forward.FORWARD,current24,current24,path=directory/'executor-rehearsal.json')
    contract.require(receipt.get('inventory_sha256')==f_request['inventory_sha256'], 'forward receipt inventory mismatch')
    forward.forward_approval(approval,f_request,request['forward_request_sha256'],original['next']['manifest_digest'])
    if 'prior_forward_attempt' in receipt:
        resume=native.pinned(directory/'resume-approval.json',receipt['resume_approval_sha256'])
        contract.require(native.digest_file(native.private_path(directory/'resume-inventory.json'))==receipt['resume_inventory_sha256'],
                         'forward resume inventory mismatch')
        prior_attempt=receipt['prior_forward_attempt']
        contract.require(isinstance(prior_attempt,str) and re.fullmatch(r'forward-[a-f0-9]{32}',prior_attempt),
                         'invalid previous forward attempt')
        previous_dir=native.private_path(state/prior_attempt,directory=True)
        prior_phase=native.pinned(previous_dir/'phase-stop-intent.json',None)
        contract.require(native.digest_file(native.private_path(previous_dir/'request.json'))==request['forward_request_sha256'] and
                         native.digest_file(native.private_path(previous_dir/'original-intent.json'))==f_request['marker_sha256'] and
                         prior_phase.get('operation')==forward.FORWARD and prior_phase.get('phase')=='STOP_INTENT' and
                         prior_phase.get('request_sha256')==request['forward_request_sha256'] and
                         prior_phase.get('original_marker_sha256')==f_request['marker_sha256'],
                         'previous forward attempt custody mismatch')
        forward.resume_approval(resume,request['forward_request_sha256'],prior_attempt,
                                f_request['marker_sha256'],receipt['resume_inventory_sha256'],original['next']['manifest_digest'],recovery=True)
    contract.require(request['units']==f_request['units'] and current.get('baseline')==old23 and
                     current.get('adoption_approval_sha256')==native.digest_file(native.private_path(
                         state/f_request['attempt']/'adoption-approval.json')) and
                     current.get('runtime_layout')=={'databases':original['databases'],
                     'socket_directory':original['socket_directory'],'peer_policy':original['peer_policy']},
                     'forward runtime or current unit mismatch')
    cold=native.pinned(directory/'backup.json',receipt['current_backup_sha256'])
    phase=native.pinned(directory/'phase-start-intent.json',None)
    contract.require(cold.get('cut')=='BOTH_OWNERS_STOPPED' and cold.get('restore')=='NOT_RUN' and
                     cold.get('request_sha256')==request['forward_request_sha256'] and
                     phase.get('operation')==forward.FORWARD and phase.get('phase')=='START_INTENT' and
                     phase.get('backup_sha256')==receipt['current_backup_sha256'], 'forward backup provenance mismatch')
    oci.fields(cold['databases'], ' '.join(oci.SERVICES), 'complete forward backup pair required')
    for service in oci.SERVICES:
        contract.require(migration.data_inventory(directory/(service+'-database'),service)==cold['databases'][service],
                         'forward backup corrupt')
    return current,original,f_request,corrected,metadata


class LiveSystem(forward.ForwardSystem):
    """The existing known-layout adapter, including its private bridge checks."""
    def assert_closed(self, paths):
        native.no_open_database_files(paths, owner_uids=(10001,10002))

    def ensure_restore_capacity(self, request, attempt):
        import os
        groups={}
        for service in oci.SERVICES:
            parent=Path(request['databases'][service]).parent
            contract.require(parent == parent.resolve() and parent.is_dir(), 'canonical restore parent required')
            device=parent.stat().st_dev
            files=[p for p in (attempt/(service+'-database')).iterdir() if p.is_file()]
            item=groups.setdefault(device, {'path':parent,'bytes':0,'files':0})
            item['bytes']+=sum(p.stat().st_size for p in files)
            item['files']+=len(files)+1
        for item in groups.values():
            capacity=os.statvfs(item['path'])
            contract.require(capacity.f_bavail*capacity.f_frsize >= item['bytes']+64*1024**2 and
                             capacity.f_favail >= item['files']+100, 'insufficient restore capacity')

    def require_stopped(self, request):
        import json
        for service in oci.SERVICES:
            contract.require(self.command(['systemctl','show','--property=ActiveState','--value',service+'.service']).strip() == b'inactive',
                             'post-migration owner unexpectedly active; restore refused')
            contract.require(json.loads(self.command(['podman','ps','--format','json','--filter','name=^'+service+'$'])) == [],
                             'post-migration container unexpectedly active; restore refused')
        self.assert_closed(request['databases'].values())


def execute(request, request_bytes, approval_bytes, inventory_bytes, old23, current24, target27, ports, *, state=native.STATE):
    """Migrate once under a write fence. Never automatically restore after START_INTENT."""
    import uuid
    request_digest=native.digest_bytes(request_bytes)
    contract.require(forward.decode_unique(request_bytes)==request and
                     native.digest_bytes(inventory_bytes)==request['inventory_sha256'], 'request or inventory drift')
    approval=forward.decode_unique(approval_bytes)
    with native.locked(state):
        current, original, prior, corrected, metadata=current_custody(state,request,old23,current24)
        decide(request,current,current24,target27)
        rehearsal(request,current24,target27)
        forward.executor_rehearsal(request,forward.MIGRATE,current24,target27)
        forward.migration_approval(approval,request_digest,request['inventory_sha256'],
                                   request['rehearsal']['sha256'],request['next']['manifest_digest'])
        layout=forward._current_layout(original,prior)
        ports.observe(layout,current24)
        forward._unit_and_config_fence(prior,original,corrected,metadata)
        attempt=state/('v27-'+uuid.uuid4().hex);attempt.mkdir(mode=0o700);native.sync_directory(state)
        forward._write_once(attempt/'request.json',request_bytes)
        forward._write_once(attempt/'approval.json',approval_bytes)
        forward._write_once(attempt/'inventory.json',inventory_bytes)
        forward._write_once(attempt/'executor-rehearsal.json',native.private_path(request['executor_rehearsal']['path']).read_bytes())
        native.pinned(attempt/'executor-rehearsal.json',request['executor_rehearsal']['sha256'])
        forward._write_once(attempt/'previous.json',native.private_path(state/'current.json').read_bytes())
        forward._write_once(attempt/'unit-metadata.json',contract.canonical(metadata))
        for service in oci.SERVICES: forward._write_once(attempt/(service+'.container'),corrected[service])
        marker={'operation':forward.MIGRATE,'attempt':attempt.name,'phase':'PREPARE',
                'request_sha256':request_digest,'previous_sha256':request['current_sha256'],
                'target_manifest_sha256':request['next']['manifest_digest']}
        contract.require(not (state/'in-progress.json').exists() and
                         native.digest_file(state/'current.json')==request['current_sha256'], 'generation changed before intent')
        native.atomic(state/'in-progress.json',contract.canonical(marker))
        ports.pull(target27)
        ports.stop(layout)
        contract.require(native.pinned(state/'in-progress.json',None)==marker,'migration intent changed')
        rechecked=current_custody(state,request,old23,current24,allow_marker=True)
        contract.require(rechecked==(current,original,prior,corrected,metadata),'current custody changed after stop')
        ports.recovery_host(layout,current24,target27)
        copies={}
        for service in oci.SERVICES:
            copies[service]=migration.copy_database(Path(layout['databases'][service]),attempt/(service+'-database'),service)
        forward._write_once(attempt/'backup.json',contract.canonical({'databases':copies,'cut':'BOTH_OWNERS_STOPPED',
                            'request_sha256':request_digest,'previous_sha256':request['current_sha256']}))
        for service in oci.SERVICES:
            migration.copy_database(attempt/(service+'-database'),attempt/(service+'-restore-check'),service,copies[service])
        restored={**layout,'databases':{service:str(attempt/(service+'-restore-check')) for service in oci.SERVICES}}
        ports.verify_current(restored,current24)
        ports.migrate(restored,target27)
        ports.verify_current(layout,current24)
        forward._unit_and_config_fence(prior,original,corrected,metadata)
        contract.require(native.pinned(state/'in-progress.json',None)==marker and
                         native.digest_file(state/'current.json')==request['current_sha256'], 'pre-migration state drift')
        marker['phase']='MIGRATE_INTENT';marker['backup_sha256']=native.digest_file(attempt/'backup.json')
        native.atomic(state/'in-progress.json',contract.canonical(marker))
        ports.migrate(layout,target27)
        forward._unit_and_config_fence(prior,original,corrected,metadata)
        contract.require(native.pinned(state/'in-progress.json',None)==marker and
                         native.digest_file(state/'current.json')==request['current_sha256'], 'post-migration state drift')
        new_units={service:forward.migration_unit(corrected[service],current24['images'][service]['ref'],
                                                  target27['images'][service]['ref']) for service in oci.SERVICES}
        for service in oci.SERVICES:
            native.replace_unit(native.UNITS/(service+'.container'),new_units[service],metadata[service])
        new_layout={**layout,'units':{service:native.digest_bytes(new_units[service]) for service in oci.SERVICES}}
        marker['phase']='START_INTENT'
        native.atomic(state/'in-progress.json',contract.canonical(marker))
        ports.start(new_layout,target27)
        ports.observe(new_layout,target27)
        forward._unit_and_config_fence({'units':new_layout['units']},original,new_units,metadata)
        contract.require(native.pinned(state/'in-progress.json',None)==marker and
                         native.digest_file(state/'current.json')==request['current_sha256'], 'post-start generation drift')
        receipt={'operation':forward.MIGRATE,'status':'OBSERVED_MIGRATED','request_sha256':request_digest,
                 'previous_sha256':request['current_sha256'],'target_manifest_sha256':request['next']['manifest_digest'],
                 'backup_sha256':marker['backup_sha256'],'images':target27['images'],
                 'approval_sha256':native.digest_bytes(approval_bytes),'inventory_sha256':request['inventory_sha256'],
                 'rehearsal_sha256':request['rehearsal']['sha256'],'live_business_acceptance':'NOT_RUN','rollback':'FORWARD_ONLY_AFTER_START'}
        forward._write_once(attempt/'receipt.json',contract.canonical(receipt))
        updated={'generation':current['generation']+1,'manifest_sha256':request['next']['manifest_digest'],
                 'images':target27['images'],'previous_manifest_sha256':current['manifest_sha256'],
                 'runtime_layout':current['runtime_layout'],'migration':{'attempt':attempt.name,
                 'receipt_sha256':native.digest_file(attempt/'receipt.json')}}
        native.atomic(state/'current.json',contract.canonical(updated))
        contract.require(native.pinned(state/'in-progress.json',None)==marker,'intent changed at commit')
        (state/'in-progress.json').unlink();native.sync_directory(state)
        return {'operation':forward.MIGRATE,'status':'OBSERVED_MIGRATED','generation':updated['generation'],
                'attempt':attempt.name,'live_business_acceptance':'NOT_RUN'}


ROLLBACK='RESTORE_NATIVE_V24_BEFORE_START'


def rollback_approval(value, request_digest, attempt, marker_digest, inventory_digest, previous_digest, backup_digest):
    oci.fields(value, 'schema_version operation request_sha256 attempt marker_sha256 inventory_sha256 '
               'previous_sha256 backup_sha256 maintenance recovery observed_at', 'exact pre-start recovery approval required')
    contract.require(type(value['schema_version']) is int and value['schema_version']==1 and
                     value['operation']==ROLLBACK and value['request_sha256']==request_digest and
                     value['attempt']==attempt and value['marker_sha256']==marker_digest and
                     value['inventory_sha256']==inventory_digest and value['previous_sha256']==previous_digest and
                     value['backup_sha256']==backup_digest and value['maintenance']=='BOTH_NATIVE_WRITERS_FENCED' and
                     value['recovery']=='RESTORE_ONLY_BEFORE_ANY_START' and type(value['observed_at']) is int and
                     0 <= time.time()-value['observed_at'] <= 3600, 'pre-start recovery approval mismatch')


def rollback(request, request_bytes, recovery_bytes, inventory_bytes, old23, current24, target27, ports, *, state=native.STATE):
    """Explicit pre-start recovery; once either new or recovered owner may start, refuse restore."""
    req_digest=native.digest_bytes(request_bytes)
    contract.require(forward.decode_unique(request_bytes)==request, 'request drift')
    recovery=forward.decode_unique(recovery_bytes)
    with native.locked(state):
        marker=native.pinned(state/'in-progress.json',None)
        contract.require(marker.get('operation')==forward.MIGRATE and marker.get('phase') in ('PREPARE','MIGRATE_INTENT') and
                         marker.get('request_sha256')==req_digest and marker.get('previous_sha256')==request['current_sha256'] and
                         marker.get('target_manifest_sha256')==request['next']['manifest_digest'] and
                         isinstance(marker.get('attempt'),str) and re.fullmatch(r'v27-[a-f0-9]{32}',marker['attempt']),
                         'after-start or foreign migration cannot restore old databases')
        attempt=native.private_path(state/marker['attempt'],directory=True)
        contract.require(native.digest_file(native.private_path(attempt/'request.json'))==req_digest and
                         native.digest_file(native.private_path(attempt/'previous.json'))==request['current_sha256'],
                         'migration custody mismatch')
        initial_approval=native.pinned(attempt/'approval.json',None)
        forward.migration_approval(initial_approval,req_digest,request['inventory_sha256'],
                                   request['rehearsal']['sha256'],request['next']['manifest_digest'])
        contract.require(native.digest_file(native.private_path(attempt/'inventory.json'))==request['inventory_sha256'],
                         'migration inventory custody mismatch')
        marker_digest=native.digest_file(state/'in-progress.json')
        backup_digest=marker.get('backup_sha256','NONE')
        inventory_digest=native.digest_bytes(inventory_bytes)
        rollback_approval(recovery,req_digest,attempt.name,marker_digest,inventory_digest,request['current_sha256'],backup_digest)
        if marker['phase']=='MIGRATE_INTENT': contract.digest(backup_digest)
        else: contract.require(backup_digest=='NONE','unexpected PREPARE backup seal')
        saved={service:native.private_path(attempt/(service+'.container')).read_bytes() for service in oci.SERVICES}
        current,original,prior,corrected,metadata=current_custody(state,request,old23,current24,allow_marker=True,unit_override=saved)
        decide(request,current,current24,target27,now=request['observed_at'])
        rehearsal(request,current24,target27)
        forward.executor_rehearsal(request,forward.MIGRATE,current24,target27,path=attempt/'executor-rehearsal.json')
        recorded=native.pinned(attempt/'unit-metadata.json',None)
        contract.require(recorded==metadata and corrected==saved, 'saved unit metadata mismatch')
        layout=forward._current_layout(original,prior)
        for service in oci.SERVICES:
            path=native.private_path(native.UNITS/(service+'.container'))
            allowed=(saved[service],forward.migration_unit(saved[service],current24['images'][service]['ref'],target27['images'][service]['ref']))
            contract.require(path.read_bytes() in allowed and native.unit_metadata(path)==metadata[service], 'foreign recovery unit')
        for name,digest in original['configuration_files'].items():
            contract.require(native.digest_file(native.private_path(name))==digest,'recovery configuration drift')
        # Persist the separately approved recovery and inventory before the first host effect.
        for name, data in [('rollback-approval.json',recovery_bytes),('rollback-inventory.json',inventory_bytes)]:
            path=attempt/name
            if path.exists():
                contract.require(native.private_path(path).read_bytes()==data, 'recovery approval custody drift')
            else:
                forward._write_once(path,data)
        ports.recovery_host(layout,current24,target27)
        if marker['phase']=='MIGRATE_INTENT':
            # If anyone restarted either owner after migration began, accepted writes are possible.
            # Reject before stopping or restoring; a separate forward reconciliation is required.
            ports.require_stopped(layout)
        else:
            ports.stop(layout)
        contract.require(native.digest_file(state/'in-progress.json')==marker_digest and
                         native.digest_file(state/'current.json')==request['current_sha256'], 'recovery state drift')
        ports.recovery_host(layout,current24,target27)
        rechecked=current_custody(state,request,old23,current24,allow_marker=True,unit_override=saved)
        contract.require(rechecked==(current,original,prior,corrected,metadata),'recovery custody drift after stop')
        for service in oci.SERVICES:
            path=native.private_path(native.UNITS/(service+'.container'))
            allowed=(saved[service],forward.migration_unit(saved[service],current24['images'][service]['ref'],target27['images'][service]['ref']))
            contract.require(path.read_bytes() in allowed and native.unit_metadata(path)==metadata[service], 'recovery unit drift after stop')
        if marker['phase']=='MIGRATE_INTENT':
            backup=native.pinned(attempt/'backup.json',backup_digest)
            contract.require(backup.get('cut')=='BOTH_OWNERS_STOPPED' and backup.get('request_sha256')==req_digest and
                             backup.get('previous_sha256')==request['current_sha256'], 'recovery backup provenance mismatch')
            oci.fields(backup['databases'],' '.join(oci.SERVICES),'complete cold pair required')
            staged={};retained={}
            for service in oci.SERVICES:
                contract.require(migration.data_inventory(attempt/(service+'-database'),service)==backup['databases'][service],
                                 'pre-start backup integrity mismatch')
                destination=Path(layout['databases'][service])
                staged[service]=destination.with_name(destination.name+'.restore-'+attempt.name)
                retained[service]=destination.with_name(destination.name+'.failed-'+attempt.name)
            ports.assert_closed([*layout['databases'].values(),*staged.values(),*retained.values()])
            ports.ensure_restore_capacity(layout,attempt)
            for service in oci.SERVICES:
                ports.require_stopped(layout)
                destination=Path(layout['databases'][service])
                failed_record=attempt/('failed-'+service+'-inventory.json')
                if not retained[service].exists():
                    # Seal the failed bytes before the first rename; no retry may adopt an
                    # unrelated retained directory or erase evidence from a prior attempt.
                    failed=migration.data_inventory(destination,service)
                    if failed_record.exists():
                        contract.require(native.pinned(failed_record,None)==failed,'failed generation drift')
                    else:
                        forward._write_once(failed_record,contract.canonical(failed))
                    if staged[service].exists():
                        contract.require(migration.data_inventory(staged[service],service)==backup['databases'][service],
                                         'partial recovery staging refused')
                    else:
                        migration.copy_database(attempt/(service+'-database'),staged[service],service,backup['databases'][service])
                    contract.require(migration.data_inventory(destination,service)==failed,'failed generation changed before rename')
                    destination.rename(retained[service]);native.sync_directory(destination.parent)
                else:
                    failed=native.pinned(failed_record,None)
                    contract.require(migration.data_inventory(retained[service],service)==failed,'retained failed generation drift')
                if not destination.exists():
                    staged[service].rename(destination);native.sync_directory(destination.parent)
                contract.require(migration.data_inventory(destination,service)==backup['databases'][service],
                                 'restored database differs from cold backup')
            ports.require_stopped(layout)
        ports.verify_current(layout,current24)
        for service in oci.SERVICES:
            native.replace_unit(native.UNITS/(service+'.container'),saved[service],metadata[service])
        forward._unit_and_config_fence(prior,original,corrected,metadata)
        contract.require(native.digest_file(state/'in-progress.json')==marker_digest and
                         native.digest_file(state/'current.json')==request['current_sha256'], 'recovery fence drift before restart')
        marker['phase']='ROLLBACK_START_INTENT'
        native.atomic(state/'in-progress.json',contract.canonical(marker))
        ports.start(layout,current24)
        ports.observe(layout,current24)
        contract.require(native.pinned(state/'in-progress.json',None)==marker and
                         native.digest_file(state/'current.json')==request['current_sha256'], 'recovery state drift after restart')
        forward._write_once(attempt/'rollback-receipt.json',contract.canonical({'operation':ROLLBACK,
                            'status':'OBSERVED_V24_RESTORED','previous_sha256':request['current_sha256'],
                            'backup_sha256':backup_digest,'approval_sha256':native.digest_bytes(recovery_bytes),
                            'inventory_sha256':inventory_digest,'live_business_acceptance':'NOT_RUN',
                            'forward_only_after_restart':'YES'}))
        (state/'in-progress.json').unlink();native.sync_directory(state)
        return {'operation':ROLLBACK,'status':'OBSERVED_V24_RESTORED','generation':current['generation'],
                'attempt':attempt.name,'live_business_acceptance':'NOT_RUN'}


def main():
    """Plan only by default. Host mutation additionally requires a pinned operator approval."""
    import argparse
    import json
    import subprocess
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request',required=True,type=Path)
    parser.add_argument('--request-digest',required=True)
    parser.add_argument('--inventory',required=True,type=Path)
    parser.add_argument('--inventory-digest',required=True)
    parser.add_argument('--executor-source',required=True)
    parser.add_argument('--approval',type=Path)
    parser.add_argument('--approval-digest')
    parser.add_argument('--execute',action='store_true')
    recovery=parser.add_mutually_exclusive_group()
    recovery.add_argument('--recover-before-start',action='store_true')
    recovery.add_argument('--reconcile-after-start',action='store_true')
    args=parser.parse_args()
    try:
        contract.sha(args.executor_source);contract.digest(args.inventory_digest)
        contract.require(not (args.recover_before_start or args.reconcile_after_start) or args.execute,
                         'recovery requires explicit execution')
        root=Path(__file__).resolve().parents[1]
        contract.require(oci.git(root,'rev-parse','HEAD').decode().strip()==args.executor_source and
                         not oci.git(root,'status','--porcelain','--untracked-files=normal').strip(),
                         'clean exact reviewed executor source required')
        request=native.pinned(args.request,args.request_digest)
        contract.require(request['executor_source']==args.executor_source, 'executor request source mismatch')
        native.pinned(args.inventory,args.inventory_digest)
        if args.reconcile_after_start:
            marker=native.pinned(native.STATE/'in-progress.json',None)
            contract.require(isinstance(marker.get('attempt'),str) and re.fullmatch(r'v27-[a-f0-9]{32}',marker['attempt']),
                             'exact interrupted migration attempt required')
            current=native.pinned(native.STATE/marker['attempt']/'previous.json',request['current_sha256'])
        else:
            current=native.pinned(native.STATE/'current.json',request['current_sha256'])
        info=current['forward_reconciliation']
        contract.require(isinstance(info,dict) and isinstance(info.get('attempt'),str) and
                         re.fullmatch(r'forward-[a-f0-9]{32}',info['attempt']),'exact forward generation required')
        prior=native.pinned(native.STATE/info['attempt']/'request.json',request['forward_request_sha256'])
        contract.require(isinstance(prior.get('attempt'),str) and re.fullmatch(r'attempt-[a-f0-9]{32}',prior['attempt']),
                         'exact historical attempt required')
        original=native.pinned(native.STATE/prior['attempt']/'request.json',prior['request_sha256'])
        old23=migration.baseline(root,original['baseline'])
        current24=native.release(root,original['next'])
        target27=native.release(root,request['next'])
        migration.rehearsal(original,old23,current24)
        rehearsal(request,current24,target27)
        if not (args.recover_before_start or args.reconcile_after_start):
            forward.executor_rehearsal(request,forward.MIGRATE,current24,target27)
        if not (args.recover_before_start or args.reconcile_after_start):
            contract.require(args.inventory_digest==request['inventory_sha256'],'inventory digest mismatch')
            current_custody(native.STATE,request,old23,current24)
            decision=decide(request,current,current24,target27)
        else:
            decision={'operation':POSTSTART if args.reconcile_after_start else ROLLBACK,
                      'status':'PLAN_ONLY','activation':'NOT_RUN','restore':'NOT_RUN'}
        if args.execute:
            contract.require(args.approval is not None and args.approval_digest is not None,
                             'separately pinned operator approval required')
            native.pinned(args.approval,args.approval_digest)
            raw,approval,inventory=args.request.read_bytes(),args.approval.read_bytes(),args.inventory.read_bytes()
            contract.require(native.digest_bytes(raw)==args.request_digest and
                             native.digest_bytes(approval)==args.approval_digest and
                             native.digest_bytes(inventory)==args.inventory_digest,'operator inputs changed')
            if args.recover_before_start:
                decision=rollback(request,raw,approval,inventory,old23,current24,target27,LiveSystem())
            elif args.reconcile_after_start:
                decision=poststart(request,raw,approval,inventory,old23,current24,target27,LiveSystem())
            else:
                decision=execute(request,raw,approval,inventory,old23,current24,target27,LiveSystem())
        print(json.dumps(decision,sort_keys=True))
    except (ValueError,OSError,KeyError,TypeError,subprocess.SubprocessError):
        parser.exit(1,'v27 migration or pre-start recovery refused; preserve journal and inspect before retry\n')


POSTSTART='RECONCILE_NATIVE_V27_START'


def poststart_approval(value, request_digest, attempt, marker_digest, inventory_digest, previous_digest, manifest_digest):
    oci.fields(value, 'schema_version operation request_sha256 attempt marker_sha256 inventory_sha256 '
               'previous_sha256 target_manifest_sha256 maintenance recovery observed_at',
               'exact post-start approval required')
    contract.require(type(value['schema_version']) is int and value['schema_version']==1 and
                     value['operation']==POSTSTART and value['request_sha256']==request_digest and
                     value['attempt']==attempt and value['marker_sha256']==marker_digest and
                     value['inventory_sha256']==inventory_digest and value['previous_sha256']==previous_digest and
                     value['target_manifest_sha256']==manifest_digest and
                     value['maintenance']=='BOTH_NATIVE_WRITERS_FENCED' and value['recovery']=='PRESERVE_POST_START_WRITES' and
                     type(value['observed_at']) is int and 0 <= time.time()-value['observed_at'] <= 3600,
                     'post-start operator approval mismatch')


def poststart(request, request_bytes, approval_bytes, inventory_bytes, old23, current24, target27, ports, *, state=native.STATE):
    """Reconcile an exact started target by capturing fresh data; old backups are never restored."""
    import uuid
    req_digest=native.digest_bytes(request_bytes)
    contract.require(forward.decode_unique(request_bytes)==request, 'request drift')
    approval=forward.decode_unique(approval_bytes)
    with native.locked(state):
        marker=native.pinned(state/'in-progress.json',None)
        contract.require(marker.get('operation')==forward.MIGRATE and marker.get('phase')=='START_INTENT' and
                         marker.get('request_sha256')==req_digest and marker.get('previous_sha256')==request['current_sha256'] and
                         marker.get('target_manifest_sha256')==request['next']['manifest_digest'] and
                         isinstance(marker.get('attempt'),str) and re.fullmatch(r'v27-[a-f0-9]{32}',marker['attempt']),
                         'exact migration start intent required')
        contract.digest(marker['backup_sha256'])
        attempt=native.private_path(state/marker['attempt'],directory=True)
        contract.require(native.digest_file(native.private_path(attempt/'request.json'))==req_digest and
                         native.digest_file(native.private_path(attempt/'previous.json'))==request['current_sha256'] and
                         native.digest_file(native.private_path(attempt/'inventory.json'))==request['inventory_sha256'],
                         'migration custody mismatch')
        initial_approval=native.pinned(attempt/'approval.json',None)
        forward.migration_approval(initial_approval,req_digest,request['inventory_sha256'],
                                   request['rehearsal']['sha256'],request['next']['manifest_digest'])
        marker_digest=native.digest_file(state/'in-progress.json')
        poststart_approval(approval,req_digest,attempt.name,marker_digest,native.digest_bytes(inventory_bytes),
                           request['current_sha256'],request['next']['manifest_digest'])
        saved={service:native.private_path(attempt/(service+'.container')).read_bytes() for service in oci.SERVICES}
        previous,original,prior,corrected,metadata=current_custody(state,request,old23,current24,
            allow_marker=True,unit_override=saved,current_path=attempt/'previous.json')
        decide(request,previous,current24,target27,now=request['observed_at'])
        rehearsal(request,current24,target27)
        forward.executor_rehearsal(request,forward.MIGRATE,current24,target27,path=attempt/'executor-rehearsal.json')
        contract.require(native.pinned(attempt/'unit-metadata.json',None)==metadata and corrected==saved,
                         'saved migration unit mismatch')
        backup=native.pinned(attempt/'backup.json',marker['backup_sha256'])
        contract.require(backup.get('cut')=='BOTH_OWNERS_STOPPED' and backup.get('request_sha256')==req_digest and
                         backup.get('previous_sha256')==request['current_sha256'], 'pre-migration backup provenance mismatch')
        oci.fields(backup['databases'],' '.join(oci.SERVICES),'complete pre-migration cold pair required')
        for service in oci.SERVICES:
            contract.require(migration.data_inventory(attempt/(service+'-database'),service)==backup['databases'][service],
                             'pre-migration backup corrupt')
        new_units={service:forward.migration_unit(saved[service],current24['images'][service]['ref'],
                                                  target27['images'][service]['ref']) for service in oci.SERVICES}
        layout={**forward._current_layout(original,prior),'units':{s:native.digest_bytes(new_units[s]) for s in oci.SERVICES}}
        forward._unit_and_config_fence({'units':layout['units']},original,new_units,metadata)
        current_path=state/'current.json'
        present=native.pinned(current_path,None)
        if native.digest_file(current_path)!=request['current_sha256']:
            completed=present.get('migration')
            if isinstance(completed,dict) and set(completed)=={'attempt','receipt_sha256'} and completed['attempt']==attempt.name:
                receipt=native.pinned(attempt/'receipt.json',completed['receipt_sha256'])
                contract.require(receipt.get('operation')==forward.MIGRATE and receipt.get('status')=='OBSERVED_MIGRATED' and
                                 receipt.get('request_sha256')==req_digest and receipt.get('previous_sha256')==request['current_sha256'] and
                                 receipt.get('target_manifest_sha256')==request['next']['manifest_digest'] and
                                 receipt.get('backup_sha256')==marker['backup_sha256'] and receipt.get('images')==target27['images'] and
                                 receipt.get('approval_sha256')==native.digest_file(native.private_path(attempt/'approval.json')) and
                                 receipt.get('inventory_sha256')==request['inventory_sha256'] and
                                 receipt.get('rehearsal_sha256')==request['rehearsal']['sha256'] and
                                 present.get('generation')==previous['generation']+1 and present.get('images')==target27['images'] and
                                 present.get('manifest_sha256')==request['next']['manifest_digest'] and
                                 present.get('runtime_layout')==previous['runtime_layout'], 'completed migration receipt mismatch')
                ports.observe(layout,target27)
                forward._unit_and_config_fence({'units':layout['units']},original,new_units,metadata)
                contract.require(native.digest_file(state/'in-progress.json')==marker_digest,'completed migration marker drift')
                (state/'in-progress.json').unlink();native.sync_directory(state)
                return {'operation':POSTSTART,'status':'RECONCILED_AFTER_INTERRUPTION','generation':present['generation'],
                        'attempt':attempt.name,'live_business_acceptance':'NOT_RUN'}
            info=present.get('poststart_reconciliation')
            contract.require(isinstance(info,dict) and set(info)=={'attempt','receipt_sha256'} and
                             isinstance(info['attempt'],str) and re.fullmatch(r'v27forward-[a-f0-9]{32}',info['attempt']),
                             'foreign post-start generation')
            finalized=native.private_path(state/info['attempt'],directory=True)
            receipt=native.pinned(finalized/'receipt.json',info['receipt_sha256'])
            contract.require(receipt.get('operation')==POSTSTART and receipt.get('status')=='OBSERVED_FORWARD_RECONCILED' and
                             receipt.get('request_sha256')==req_digest and receipt.get('original_marker_sha256')==marker_digest and
                             receipt.get('target_manifest_sha256')==request['next']['manifest_digest'] and
                             receipt.get('images')==target27['images'] and receipt.get('approval_sha256')==native.digest_bytes(approval_bytes) and
                             receipt.get('inventory_sha256')==native.digest_bytes(inventory_bytes) and
                             present.get('generation')==previous['generation']+1 and
                             present.get('images')==target27['images'] and present.get('manifest_sha256')==request['next']['manifest_digest'] and
                             present.get('runtime_layout')==previous['runtime_layout'],
                             'post-start receipt mismatch')
            cold=native.pinned(finalized/'backup.json',receipt['current_backup_sha256'])
            contract.require(cold.get('request_sha256')==req_digest and cold.get('cut')=='BOTH_OWNERS_STOPPED' and
                             cold.get('restore')=='NOT_RUN', 'post-start current backup provenance mismatch')
            oci.fields(cold['databases'],' '.join(oci.SERVICES),'complete current backup pair required')
            for service in oci.SERVICES:
                contract.require(migration.data_inventory(finalized/(service+'-database'),service)==cold['databases'][service],
                                 'post-start current backup corrupt')
            phase=native.pinned(finalized/'phase-start-intent.json',None)
            contract.require(native.digest_file(native.private_path(finalized/'original-intent.json'))==marker_digest and
                             native.digest_file(native.private_path(finalized/'request.json'))==req_digest and
                             native.digest_file(native.private_path(finalized/'approval.json'))==native.digest_bytes(approval_bytes) and
                             native.digest_file(native.private_path(finalized/'inventory.json'))==native.digest_bytes(inventory_bytes) and
                             phase.get('operation')==POSTSTART and phase.get('phase')=='START_INTENT' and
                             phase.get('recovery')=='FORWARD_ONLY' and phase.get('backup_sha256')==receipt['current_backup_sha256'],
                             'post-start custody mismatch')
            ports.observe(layout,target27)
            forward._unit_and_config_fence({'units':layout['units']},original,new_units,metadata)
            contract.require(native.digest_file(state/'in-progress.json')==marker_digest,'post-start marker drift')
            (state/'in-progress.json').unlink();native.sync_directory(state)
            return {'operation':POSTSTART,'status':'RECONCILED_AFTER_INTERRUPTION','generation':present['generation'],
                    'attempt':finalized.name,'live_business_acceptance':'NOT_RUN'}
        contract.require(present==previous,'previous generation changed')
        ports.recovery_host(layout,current24,target27)
        ports.ensure_capacity(layout)
        # This is forward-only: stop every possible owner, capture the latest actual pair and
        # check it with the target binaries. A partial migration is refused, never rolled back.
        operation=state/('v27forward-'+uuid.uuid4().hex);operation.mkdir(mode=0o700);native.sync_directory(state)
        for name,data in [('request.json',request_bytes),('approval.json',approval_bytes),('inventory.json',inventory_bytes),
                          ('original-intent.json',native.private_path(state/'in-progress.json').read_bytes())]:
            forward._write_once(operation/name,data)
        forward._write_once(operation/'phase-stop-intent.json',contract.canonical({'operation':POSTSTART,
                            'phase':'STOP_INTENT','original_marker_sha256':marker_digest,'request_sha256':req_digest}))
        ports.stop(layout)
        contract.require(native.digest_file(state/'in-progress.json')==marker_digest and
                         native.digest_file(current_path)==request['current_sha256'], 'post-start state drift after stop')
        forward._unit_and_config_fence({'units':layout['units']},original,new_units,metadata)
        ports.recovery_host(layout,current24,target27)
        ports.ensure_capacity(layout)
        copies={}
        for service in oci.SERVICES:
            copies[service]=migration.copy_database(Path(layout['databases'][service]),operation/(service+'-database'),service)
        forward._write_once(operation/'backup.json',contract.canonical({'databases':copies,'cut':'BOTH_OWNERS_STOPPED',
                            'request_sha256':req_digest,'restore':'NOT_RUN'}))
        for service in oci.SERVICES:
            migration.copy_database(operation/(service+'-database'),operation/(service+'-restore-check'),service,copies[service])
        restored={**layout,'databases':{s:str(operation/(s+'-restore-check')) for s in oci.SERVICES}}
        ports.verify_current(restored,target27)
        ports.verify_current(layout,target27)
        contract.require(native.digest_file(state/'in-progress.json')==marker_digest and
                         native.digest_file(current_path)==request['current_sha256'], 'post-start state drift before restart')
        forward._write_once(operation/'phase-start-intent.json',contract.canonical({'operation':POSTSTART,
                            'phase':'START_INTENT','recovery':'FORWARD_ONLY','backup_sha256':native.digest_file(operation/'backup.json')}))
        ports.start(layout,target27)
        ports.observe(layout,target27)
        forward._unit_and_config_fence({'units':layout['units']},original,new_units,metadata)
        contract.require(native.digest_file(state/'in-progress.json')==marker_digest and
                         native.digest_file(current_path)==request['current_sha256'], 'post-start state drift after restart')
        receipt={'operation':POSTSTART,'status':'OBSERVED_FORWARD_RECONCILED','request_sha256':req_digest,
                 'original_marker_sha256':marker_digest,'target_manifest_sha256':request['next']['manifest_digest'],
                 'current_backup_sha256':native.digest_file(operation/'backup.json'),'images':target27['images'],
                 'approval_sha256':native.digest_bytes(approval_bytes),'inventory_sha256':native.digest_bytes(inventory_bytes),
                 'live_business_acceptance':'NOT_RUN','database_restore':'NOT_RUN'}
        forward._write_once(operation/'receipt.json',contract.canonical(receipt))
        updated={'generation':previous['generation']+1,'manifest_sha256':request['next']['manifest_digest'],
                 'images':target27['images'],'previous_manifest_sha256':previous['manifest_sha256'],
                 'runtime_layout':previous['runtime_layout'],'poststart_reconciliation':{'attempt':operation.name,
                 'receipt_sha256':native.digest_file(operation/'receipt.json')}}
        native.atomic(current_path,contract.canonical(updated))
        contract.require(native.digest_file(state/'in-progress.json')==marker_digest,'post-start marker drift at commit')
        (state/'in-progress.json').unlink();native.sync_directory(state)
        return {'operation':POSTSTART,'status':'OBSERVED_FORWARD_RECONCILED','generation':updated['generation'],
                'attempt':operation.name,'live_business_acceptance':'NOT_RUN'}


if __name__ == '__main__':
    main()
