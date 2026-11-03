"""Disposable exact-image ADR0057 rehearsal. No production path or activation command."""
import json
import os
from pathlib import Path
import re
import signal
import uuid

import release_artifact as contract
import release_native as native
import release_native_recovery as recovery
import release_oci as oci


class Ports:
    """Finite adapter: exact image owner commands, immutable cold copies, no SQL."""
    def __init__(self, fixture, copy, record, *, kill_after=None):
        self.f, self.copy_directory, self.record = fixture, copy, record
        self.kill_after = kill_after

    def release_identity(self):
        value = self.f.manifest
        return {key:value[key] for key in ('source_sha','configuration_sha256')} | {
            'images':{service:value['images'][service]['ref'] for service in oci.SERVICES}}

    def observe(self, service, root):
        return self.f.owner_command(service,'observe',database=root)

    def export(self, service, source, destination):
        destination.mkdir(parents=True,mode=0o700)
        destination.chmod(0o700)
        uid = 10001 if service=='cantos-server' else 10002
        os.chown(destination,uid,uid)
        filename = 'server.db' if service=='cantos-server' else 'identity.db'
        result = self.f.owner_command(service,'export',('/export/'+filename,),database=source,
            extra=('--mount',f'type=bind,src={destination},dst=/export'))
        exported = result['backup']
        contract.require(exported['bytes']==(destination/filename).stat().st_size and
                         exported['sha256']==native.digest_file(destination/filename), 'OWNER_EXPORT_BYTES_MISMATCH')

    def copy(self, source, destination):
        return self.copy_directory(source,destination,self.record(source))

    def directory_record(self, root):
        return self.record(root)

    def reconcile(self, service, root):
        return self.f.owner_command(service,'reconcile',database=root)

    def rotate(self, root, operation, expected_epoch):
        result = self.f.owner_command('cantos-identity','rotate',(operation,expected_epoch),database=root)
        if self.kill_after=='rotate': os.kill(os.getpid(),signal.SIGKILL)
        return result

    def retire(self, root):
        result = self.f.owner_command('cantos-server','retire-sessions',database=root)
        if self.kill_after=='retire': os.kill(os.getpid(),signal.SIGKILL)
        return result


def request(f, ports, sources):
    return {'schema_version':1,'operation':uuid.uuid4().hex,
            'barrier':str(f.exposure_dir/'barrier'),**ports.release_identity(),
            'sources':{service:str(path) for service,path in sources.items()}}


def closed(value):
    expected=('CANTOS_RECOVERY_BARRIER_V1\nblocked '+value['operation']+'\n').encode()
    contract.require(Path(value['barrier']).read_bytes()==expected,'RECOVERY_BARRIER_NOT_CLOSED')


def refused(f, state, value, ports):
    state.mkdir(parents=True,mode=0o700)
    try:
        recovery.reconcile(state,value,ports)
    except (ValueError,OSError):
        closed(value)
        contract.require(not (state/'staged').exists(),'REFUSED_SOURCE_CREATED_STAGED_PAIR')
        return
    raise ValueError('UNTRUSTED_SECURITY_SOURCE_ACCEPTED')


def crash_reconcile(f, state, value, copy, record, after):
    """Kill the real coordinator AFTER a real owner commit, before its durable phase ACK.

    This is not an engine power-loss test. The owner CLI has committed and fsynced its floor;
    a fresh coordinator must reconcile the actual files/receipt without duplicate effects.
    """
    child=os.fork()
    if child==0:
        try:
            recovery.reconcile(state,value,Ports(f,copy,record,kill_after=after))
        except Exception as error:
            context=getattr(f,'operation_context','UNKNOWN')
            code=str(error)
            safe=code if isinstance(error,ValueError) and re.fullmatch('[A-Za-z0-9_:-]{1,160}',code) else type(error).__name__
            native.atomic(state/'child-failure.json',contract.canonical({'context':context,'error':safe}))
        os._exit(91)
    _,status=os.waitpid(child,0)
    if not (os.WIFSIGNALED(status) and os.WTERMSIG(status)==signal.SIGKILL):
        detail='UNKNOWN'
        if (state/'child-failure.json').exists():
            failure=native.pinned(state/'child-failure.json',None)
            detail=re.sub('[^A-Z0-9_]','_',failure['context'].upper())[:80]
        raise ValueError('RECOVERY_COORDINATOR_'+after.upper()+'_CRASH_NOT_REACHED_'+detail)
    closed(value)


def run(f, copy, record):
    contract.require(os.geteuid()==0 and f.fenced_images(),'FENCED_DISPOSABLE_ROOT_REQUIRED')
    checks=f.call('recovery_accounts')['checks']
    f.stop()
    current=dict(f.databases)
    stale={service:f.work/'pre-cut-security'/service for service in oci.SERVICES}
    stale_records={service:copy(current[service],stale[service]) for service in oci.SERVICES}
    old_floors={service:(f.fences[service]/'current').read_bytes() for service in oci.SERVICES}
    f.restart()
    checks+=f.call('persisted')['checks']
    checks+=f.call('revoke')['checks']
    f.stop()
    fixture_actor=f.disable_recovery_fixture_account()
    f.restart()
    checks+=f.call('recovery_current_security')['checks']
    f.stop()

    ports=Ports(f,copy,record)
    # Neither a matching old pair nor either mixed pair is a trusted current security cut.
    for label,old_services in [('both',oci.SERVICES),('server',('cantos-server',)),('identity',('cantos-identity',))]:
        sources={service:stale[service] if service in old_services else current[service] for service in oci.SERVICES}
        value=request(f,ports,sources)
        refused(f,f.work/'recovery-negative'/('stale-'+label),value,ports)
        for service in old_services:
            contract.require(record(stale[service])==stale_records[service],'STALE_REFUSAL_CHANGED_BYTES')
        checks.append('stale-'+label+'-security-source-refused-before-staging')

    # Missing/corrupt/old/over-permissive external floor is not a reason to regenerate it.
    for kind in ('missing','corrupt','old','untrusted'):
        service='cantos-identity'
        path=f.fences[service]/'current'
        saved=path.read_bytes()
        mode=path.stat().st_mode & 0o777
        try:
            if kind=='missing': path.rename(path.with_name('held-current'))
            elif kind=='corrupt': path.write_bytes(b'not a recovery authority\n')
            elif kind=='old': path.write_bytes(old_floors[service])
            else: path.chmod(0o644)
            value=request(f,ports,current)
            refused(f,f.work/'recovery-negative'/('floor-'+kind),value,ports)
            checks.append(kind+'-security-floor-refused')
        finally:
            # Fault injection is confined to this disposable fixture; restore the exact
            # original current floor, never a synthesized revision or older authority.
            if kind=='missing': path.with_name('held-current').rename(path)
            else: path.write_bytes(saved)
            path.chmod(mode)
        contract.require(path.read_bytes()==saved,'FIXTURE_FLOOR_NOT_RESTORED_EXACTLY')

    # A malformed/untrusted data source also stays closed; original current data is untouched.
    for kind in ('corrupt','untrusted'):
        bad=f.work/'recovery-negative'/('source-'+kind)/'cantos-identity'
        copy(current['cantos-identity'],bad)
        if kind=='corrupt': (bad/'identity.db').write_bytes(b'not a Turso database')
        else: bad.chmod(0o755)
        value=request(f,ports,{**current,'cantos-identity':bad})
        refused(f,f.work/'recovery-negative'/('source-'+kind+'-attempt'),value,ports)
        checks.append(kind+'-security-source-refused')

    # Foreground Serve must refuse the raw pre-cut files even with valid production settings.
    f.recovery_operation=uuid.uuid4().hex
    recovery.barrier(f.exposure_dir/'barrier',f.recovery_operation,ready=True)
    for service in oci.SERVICES:
        f.operation_context='stale-raw-serve-refusal-'+service
        f.pod('run','--rm','--network=none',*f.options(service,database=stale[service]),
              f.image(service),*(['serve'] if service=='cantos-identity' else []),expected=1)
        contract.require(record(stale[service])==stale_records[service],'STALE_SERVE_CHANGED_BYTES')
    checks.append('raw-old-pair-cannot-serve-against-current-floor')

    state=f.work/'recovery-current-authority'
    state.mkdir(mode=0o700)
    value=request(f,ports,current)
    # Both real process crashes must be reached; exceptions or an early refusal cannot count.
    crash_reconcile(f,state,value,copy,record,'rotate')
    checks.append('real-coordinator-killed-after-identity-commit-before-phase-ack')
    crash_reconcile(f,state,value,copy,record,'retire')
    checks.append('real-coordinator-killed-after-session-retire-before-phase-ack')
    result=recovery.reconcile(state,value,ports)
    closed(value)
    checks.append('fresh-coordinator-resumes-exact-committed-receipts-with-barrier-closed')
    recovery.expose(state,value,ports)
    f.recovery_operation=value['operation']
    f.databases={service:Path(path) for service,path in result['staged'].items()}
    try:
        f.restart()
        checks+=f.call('recovery_after')['checks']
        f.stop()
        f.restart()
        checks+=f.call('recovery_after_restart')['checks']
    except Exception:
        recovery.barrier(f.exposure_dir/'barrier',f.recovery_operation)
        raise
    return {'checks':checks,'backup_files':stale_records,'fixture_actor':fixture_actor,
            'recovery_protocol':'ADR0057_CURRENT_AUTHORITY_ONLY','coordinator_process_crashes':2,
            'stale_backup_only':'UNSUPPORTED_CLOSED','production_activation':'NOT_RUN',
            'scope':'exact images, real Turso owners/RPC/HTTP; external floor and current-source reconciliation; host executor, full-host rollback and restricted activation remain separate'}
