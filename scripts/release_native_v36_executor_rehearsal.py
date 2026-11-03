"""Real systemd/Quadlet initial-upgrade rehearsal on a fresh disposable GitHub VM.

Synthetic credentials and session bytes stay in private memory. Provisional gate inputs
exist only inside the disposable fixture and are never exported as reviewed evidence.
No SSH, production host, admission, floor reset or database restoration is supported.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time
import uuid

import release_artifact as contract
import release_native as native
import release_native_executor_journey as journey
import release_native_executor_rehearsal as trace
import release_native_executor_systemd_fixture as fixture
import release_native_forward as forward
import release_native_migration as migration
import release_native_v36_live as live
import release_native_v36_policy as policy
import release_oci as oci

PHASE = 'PREFLIGHT'


def resume_inputs(request, raw, *, abort=False):
    marker = native.pinned(native.STATE/'in-progress.json',None)
    inventory = contract.canonical({'fixture':'v36-forward-current-state','observation':uuid.uuid4().hex})
    approval = contract.canonical({'schema_version':1,'operation':policy.OPERATION+('_ABORT_BEFORE_MIGRATE' if abort else '_FORWARD_RESUME'),
        'request_sha256':native.digest_bytes(raw),'attempt':marker['attempt'],
        'marker_sha256':native.digest_file(native.STATE/'in-progress.json'),
        'inventory_sha256':native.digest_bytes(inventory),'observed_at':int(time.time()),
        'target_manifest_sha256':request['next']['manifest_digest'],
        'security_plan_sha256':request['security_plan_sha256'],
        'recovery':'UNCHANGED_UNENROLLED_PAIR_ONLY' if abort else 'PRESERVE_CURRENT_PAIR_AND_EXTERNAL_FLOORS'})
    return approval,inventory


def run(root,args):
    global PHASE
    # setup's strict probe refuses every pre-existing protected path/service before effects.
    _,layout,old,new,_,_,_=fixture.setup(root,args,initial_v36=True)
    system=live.InitialSystem()
    PHASE='START_OLD_EXACT_PAIR'
    system.start(layout,old)
    system.observe(layout,old)
    PHASE='REAL_SMTP_SIGNUP'
    actor=journey.signup(system,layout)
    preserved=journey.login_session(system,layout)
    revoked=journey.login_session(system,layout)
    address=journey.server_address(system,layout)
    journey.request(address,'POST',journey.BASE+'revoke',204,
                    {k:revoked[k] for k in ('refresh_token','session_id')})
    journey.request(address,'GET',journey.BASE+'session',401,
                    headers={'Authorization':'Bearer '+revoked['access_token']})
    PHASE='FOREIGN_OWNER_CONTROL'
    trace.foreign_bind_control(system,layout,old)
    original_units={s:(native.UNITS/(s+'.container')).read_bytes() for s in oci.SERVICES}
    bad={**layout,'units':dict.fromkeys(oci.SERVICES,'0'*64)}
    try:
        system.observe(bad,old)
    except ValueError as error:
        contract.require(str(error)=='unit changed','unit drift negative control mismatch')
    else:
        raise ValueError('unit drift accepted')
    system.observe(layout,old)
    args.output.mkdir(parents=True,mode=0o700,exist_ok=False)
    current={'generation':2,'manifest_sha256':'a'*64,'images':old['images'],
             'runtime_layout':{k:layout[k] for k in ('databases','socket_directory','peer_policy')}}
    native.atomic(native.STATE/'current.json',contract.canonical(current))
    provisional=native.STATE/'fixture-only-v36-executor.json'
    forward._write_once(provisional,contract.canonical({'schema_version':1,
        'operation':policy.OPERATION+'_EXECUTOR_REHEARSAL','status':'PASS','executor_source':args.source_sha,
        'target_manifest_sha256':args.manifest_digest,
        'previous':migration.new_identity(old),'target':migration.new_identity(new),'checks':live.CHECKS,
        'evidence_level':'integration-tested','execution_environment':'linux-native-systemd-podman-exact-images',
        'activation':'NOT_RUN','fixture_only':'NEVER_EXPORTED'}))
    inventory=contract.canonical({'fixture':'disposable-real-systemd-initial-v36','images':old['images']})
    request={'schema_version':1,'operation':policy.OPERATION,'observed_at':int(time.time()),
        'inventory_sha256':native.digest_bytes(inventory),'current_sha256':native.digest_file(native.STATE/'current.json'),
        'generation':2,'current_manifest_sha256':current['manifest_sha256'],'executor_source':args.source_sha,
        'security_plan_sha256':contract.fingerprint(policy.security_layout()),
        'next':{'manifest_digest':args.manifest_digest,'source_sha':new['source_sha'],
                'configuration_digest':new['configuration_sha256'],'platform':new['platform']},
        'rehearsal':{'path':str(args.historical),'sha256':native.digest_file(args.historical)},
        'executor_rehearsal':{'path':str(provisional),'sha256':native.digest_file(provisional)},**layout}
    raw=contract.canonical(request)
    approval=contract.canonical({'schema_version':1,'operation':policy.OPERATION,
        'request_sha256':native.digest_bytes(raw),'inventory_sha256':request['inventory_sha256'],
        'target_manifest_sha256':args.manifest_digest,'rehearsal_sha256':request['rehearsal']['sha256'],
        'executor_rehearsal_sha256':request['executor_rehearsal']['sha256'],
        'security_plan_sha256':request['security_plan_sha256'],'maintenance':'BOTH_NATIVE_WRITERS_FENCED',
        'recovery':'FORWARD_ONLY_FROM_MIGRATE_INTENT'})
    live.rehearsal(request,old,new)
    PHASE='REAL_PRE_MIGRATION_ABORT'
    trace.interrupt(Path(live.__file__),'execute',
        "        phase(state, attempt, digest, 'STOP_INTENT')",
        '        ports.stop(request)',
        lambda:live.execute(request,raw,approval,inventory,old,new,system))
    abort,observed=resume_inputs(request,raw,abort=True)
    result=live.abort_before_migrate(request,raw,abort,observed,old,new,system)
    contract.require(result.get('generation')==2 and not Path(policy.SECURITY_ROOT).exists(),
                     'pre-migration abort altered generation or authority')
    journey.verify_session(system,layout,preserved)
    PHASE='REAL_MIGRATION_CRASH'
    trace.interrupt(Path(live.__file__),'execute',
        '            ports.database_command(new, service, path, check=True)',
        "        phase(state, attempt, digest, 'ENROLL_INTENT')",
        lambda:live.execute(request,raw,approval,inventory,old,new,system))
    marker=native.pinned(native.STATE/'in-progress.json',None)
    contract.require(marker['phase']=='MIGRATE_INTENT','real migration crash boundary mismatch')
    system.require_stopped(request)
    provenance={'migration_intent_sha256':native.digest_file(native.STATE/'in-progress.json')}
    try:
        live.execute(request,raw,approval,inventory,old,new,system)
    except ValueError as error:
        contract.require(str(error)=='interrupted v36 operation requires forward reconciliation','blind replay refusal mismatch')
    else:
        raise ValueError('blind migration replay accepted')
    PHASE='REAL_ENROLLMENT_COMMIT_CRASH'
    resume,observed=resume_inputs(request,raw)
    trace.interrupt(Path(live.__file__),'resume_forward',
        "            observed = ports.enroll_or_reconcile(new,service,Path(request['databases'][service]))",
        "            path = attempt/(service+'-enrollment.json')",
        lambda:live.resume_forward(request,raw,resume,observed,old,new,system))
    marker=native.pinned(native.STATE/'in-progress.json',None)
    contract.require(marker['phase']=='ENROLL_INTENT','real enrollment crash boundary mismatch')
    barrier=Path(policy.SECURITY_ROOT)/'exposure/barrier'
    contract.require(b'\nblocked ' in barrier.read_bytes(),'enrollment crash opened exposure')
    server_floor=Path(policy.security_layout()['cantos-server']['path'])/'current'
    retained_floor=server_floor.read_bytes()
    enrolled=system.owner_command(new,'cantos-server',Path(request['databases']['cantos-server']),'observe')
    provenance['enrollment_floor_sha256']=hashlib.sha256(retained_floor).hexdigest()
    PHASE='REAL_START_AND_ACCEPTED_WRITE_CRASH'
    resume,observed=resume_inputs(request,raw)
    accepted={}
    target_layout={**request,'security_operation':marker['attempt'].removeprefix('v36-')}
    def accept_after_start():
        accepted.update(journey.login_session(system,target_layout))
        contract.require(accepted.get('actor_id')==actor,'poststart actor changed')
    trace.interrupt(Path(live.__file__),'resume_forward',
        '            ports.start(layout,new)',
        '            ports.observe_target(layout,new)',
        lambda:live.resume_forward(request,raw,resume,observed,old,new,system),
        on_stopped=accept_after_start)
    contract.require(native.pinned(native.STATE/'in-progress.json',None)['phase']=='START_INTENT',
                     'poststart crash boundary mismatch')
    provenance['accepted_write_marker_sha256']=native.digest_file(native.STATE/'in-progress.json')
    PHASE='FORWARD_PRESERVE_CURRENT_WRITES'
    resume,observed=resume_inputs(request,raw)
    result=live.resume_forward(request,raw,resume,observed,old,new,system)
    contract.require(result.get('generation')==3,'forward generation mismatch')
    journey.verify_session(system,target_layout,accepted)
    journey.verify_session(system,target_layout,preserved)
    contract.require(journey.login(system,target_layout)==actor,'preserved password/actor mismatch')
    journey.request(journey.server_address(system,target_layout),'GET',journey.BASE+'session',401,
                    headers={'Authorization':'Bearer '+revoked['access_token']})
    PHASE='ORDINARY_RESTART'
    system.stop(target_layout)
    system.start(target_layout,new)
    journey.verify_session(system,target_layout,accepted)
    # Enrollment wrote a new floor once; subsequent legitimate session writes advance its
    # revision. The immutable original enrollment proof retains the same instance/epoch.
    attempt=native.STATE/result['attempt']
    initial=native.pinned(attempt/'cantos-server-enrollment.json',None)
    current=native.pinned(native.STATE/'current.json',None)
    contract.require(current['security']['operation']==target_layout['security_operation'] and
                     initial['owner']=='cantos-server','security operation changed')
    for service in oci.SERVICES:
        expected=policy.target_unit(original_units[service],service,old['images'][service]['ref'],
                                   new['images'][service]['ref'],target_layout['security_operation'])
        contract.require((native.UNITS/(service+'.container')).read_bytes()==expected,'unexpected unit delta')
    system.stop(target_layout)
    final_proof=system.owner_command(new,'cantos-server',Path(request['databases']['cantos-server']),'observe')
    contract.require(final_proof['instance']==enrolled['instance'] and final_proof['revision']>=enrolled['revision'],
                     'lost enrollment response reset the external authority')
    live.verify_backup(attempt,native.digest_bytes(raw))
    provenance['upgrade_receipt_sha256']=native.digest_file(attempt/'receipt.json')
    record={'schema_version':1,'operation':policy.OPERATION+'_EXECUTOR_REHEARSAL','status':'PASS',
        'executor_source':args.source_sha,'previous':migration.new_identity(old),'target':migration.new_identity(new),
        'target_manifest_sha256':args.manifest_digest,'checks':live.CHECKS,'evidence_level':'integration-tested',
        'execution_environment':'linux-native-systemd-podman-exact-images','activation':'NOT_RUN','admission':'NOT_RUN',
        'fixture_scope':'synthetic-accounts-real-systemd-quadlet-executor-functions',
        'provenance':provenance,'not_covered':['power-loss','every-durable-boundary','production-host','business-journeys']}
    contract.write_new(args.output/'executor-rehearsal.json',record)
    return record


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('manifest','authfile','historical','output'):
        parser.add_argument('--'+name,required=True,type=Path)
    for name in ('manifest-digest','source-sha','configuration-digest','platform'):
        parser.add_argument('--'+name,required=True)
    args=parser.parse_args()
    try:
        result=run(Path(__file__).resolve().parents[1],args)
    except (ValueError,OSError,KeyError,TypeError,subprocess.SubprocessError) as error:
        reason=str(error) if isinstance(error,ValueError) and re.fullmatch(r'[A-Za-z0-9 _:/.-]{1,160}',str(error)) else type(error).__name__
        if args.output.is_dir():
            failure={'status':'FAIL','phase':PHASE,'reason':reason,'executor_transition':'NOT_RUN','activation':'NOT_RUN'}
            contract.write_new(args.output/'failure.json',failure)
        parser.exit(1,'v36 executor rehearsal failed in '+PHASE+': '+reason+'; activation NOT_RUN\n')
    print(json.dumps(result,sort_keys=True))


if __name__=='__main__':
    main()
