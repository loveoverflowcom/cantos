"""Real private files plus a named runtime double; never systemd/image evidence."""
import copy
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_native as native
import release_native_adapter as adapter
import release_native_migration as migration
import release_native_recovery as recovery
import release_native_v36_live as live
import release_native_v36_policy as policy
import release_native_v36_rehearsal as historical
import test_release_native_v36_policy as fixtures


class HostDouble:
    def __init__(self):
        self.events=[]
        self.proofs={}
        self.running=True
        self.failure=None
        self.accept_write=False

    def observe(self,request,release): self.events.append('observe-old')
    def observe_target(self,request,release):
        self.events.append('observe-target')
        for service in native.SERVICES:
            unit=native.UNITS/(service+'.container')
            if native.digest_file(unit)!=request['units'][service]: raise ValueError('target unit drift')
    def pull(self,release):
        self.events.append('pull')
        if self.failure=='pull': raise ValueError('pull failed')
    def stop(self,request): self.events.append('stop');self.running=False
    def require_stopped(self,request):
        if self.running: raise ValueError('unexpected live owner')
    def recovery_host(self,request,old,new): self.events.append('recovery-host')
    def ensure_capacity(self,request): self.events.append('capacity')
    def database_command(self,release,service,directory,*,check=False):
        if not check:
            if self.failure=='second-live-migrate' and service=='cantos-identity' and str(directory)==migration.DATABASES[service]:
                self.failure=None
                raise ValueError('unknown migrate acknowledgement')
            path=directory/('server.db' if service=='cantos-server' else 'identity.db')
            data=path.read_bytes()
            if b'new-schema' not in data: path.write_bytes(data+b'new-schema\n')
            self.events.append('migrate-'+('live' if str(directory)==migration.DATABASES[service] else 'copy'))
    def provision(self,operation):
        for item in policy.security_layout().values(): Path(item['path']).mkdir(mode=item['mode'],exist_ok=True)
        recovery.barrier(Path(policy.SECURITY_ROOT)/'exposure/barrier',operation)
        self.events.append('provision')
    def security(self,operation):
        self.events.append('security')
        for item in policy.security_layout().values():
            if not Path(item['path']).is_dir():raise ValueError('security access missing')
    def exposure(self,operation,ready=False):
        recovery.barrier(Path(policy.SECURITY_ROOT)/'exposure/barrier',operation,ready=ready)
        self.events.append('ready' if ready else 'blocked')
    def owner_command(self,release,service,directory,action):
        floor=Path(policy.security_layout()[service]['path'])/'current'
        if action=='enroll':
            if service in self.proofs and not floor.exists():raise ValueError('missing enrolled authority')
            if service not in self.proofs:
                self.proofs[service]={'owner':service,'schema':36 if service=='cantos-server' else 7,
                    'instance':'1'*32 if service=='cantos-server' else '2'*32,'revision':1,'exposure':'NOT_AUTHORIZED'}
                if service=='cantos-server': self.proofs[service]['all_prior_sessions_retired']=False
                else:self.proofs[service]['epoch']='3'*32
                native.atomic(floor,b'committed owner floor\n')
            if self.failure=='enroll-after-commit':
                self.failure=None
                raise ValueError('lost enrollment response')
        if not floor.exists():raise ValueError('missing enrolled authority')
        return copy.deepcopy(self.proofs[service])
    def enroll_or_reconcile(self,release,service,directory):
        floor=Path(policy.security_layout()[service]['path'])/'current'
        return self.owner_command(release,service,directory,'reconcile' if floor.exists() else 'enroll')
    def start(self,request,release):
        self.events.append('start-new')
        self.running=True
        if self.accept_write:
            self.accept_write=False
            for service,path in request['databases'].items():
                file=Path(path)/('server.db' if service=='cantos-server' else 'identity.db')
                file.write_bytes(file.read_bytes()+b'accepted-after-start\n')
                self.proofs[service]['revision']+=1
            raise ValueError('lost start acknowledgement after accepted write')


class InitialFiles(unittest.TestCase):
    def setUp(self):
        temporary=tempfile.TemporaryDirectory(prefix='cantos-v36-test-')
        self.addCleanup(temporary.cleanup)
        self.root=Path(temporary.name).resolve()
        base=self.root/'native';base.mkdir(mode=0o700)
        configs=base/'config';configs.mkdir(mode=0o700)
        self.state=self.root/'state';self.state.mkdir(mode=0o700)
        units=self.root/'units';units.mkdir(mode=0o700)
        databases={s:str(base/('server-db' if s=='cantos-server' else 'identity-db')) for s in native.SERVICES}
        for service,name in databases.items():
            path=Path(name);path.mkdir(mode=0o700)
            filename='server.db' if service=='cantos-server' else 'identity.db'
            native.atomic(path/filename,b'original current data\n')
        self.addCleanup(patch.stopall)
        patch.multiple(migration,ROOT=str(base),DATABASES=databases,SOCKET=str(base/'run'),MEDIA=str(base/'storage')).start()
        patch.object(native,'UNITS',units).start()
        patch.object(policy,'SECURITY_ROOT',str(self.root/'security')).start()
        self.request,self.current,self.old,self.new=fixtures.InitialUpgradePolicy().fixture()
        self.request['observed_at']=int(time.time())
        self.request['security_plan_sha256']=contract.fingerprint(policy.security_layout())
        self.original_units={}
        for service in native.SERVICES:
            data,_,_=fixtures.UnitTransform().fixture(service)
            data=data.replace(('ghcr.io/loveoverflowcom/'+service+'@sha256:'+'1'*64).encode(),
                              self.old['images'][service]['ref'].encode())
            path=units/(service+'.container');native.atomic(path,data)
            self.original_units[service]=data
            self.request['units'][service]=native.digest_file(path)
        config=configs/'server.env';native.atomic(config,b'PRESERVED=private-config\n')
        self.request['configuration_files']={str(config):native.digest_file(config)}
        native.atomic(self.state/'current.json',contract.canonical(self.current))
        self.request['current_sha256']=native.digest_file(self.state/'current.json')
        self.inventory=contract.canonical({'scope':'synthetic-runtime-double'})
        self.request['inventory_sha256']=native.digest_bytes(self.inventory)
        historical_path=self.root/'historical.json'
        record={'operation':historical.OPERATION,'status':'PASS','baseline':migration.new_identity(self.old),
            'target':migration.new_identity(self.new),'input_manifest_sha256':self.request['next']['manifest_digest'],
            'checks':historical.CHECKS,'evidence_level':'integration-tested','execution_environment':'linux-native-exact-images',
            'host_adapter':{'status':'PASS','source_sha':self.new['source_sha'],'images':self.new['images'],
                            'checks':adapter.CHECKS,'activation':'NOT_RUN'}}
        native.atomic(historical_path,contract.canonical(record))
        self.request['rehearsal']={'path':str(historical_path),'sha256':native.digest_file(historical_path)}
        executor_path=self.root/'executor.json'
        record={'schema_version':1,'operation':policy.OPERATION+'_EXECUTOR_REHEARSAL','status':'PASS',
            'executor_source':self.request['executor_source'],'previous':migration.new_identity(self.old),
            'target_manifest_sha256':self.request['next']['manifest_digest'],
            'target':migration.new_identity(self.new),'checks':live.CHECKS,'evidence_level':'integration-tested',
            'execution_environment':'linux-native-systemd-podman-exact-images','activation':'NOT_RUN'}
        native.atomic(executor_path,contract.canonical(record))
        self.request['executor_rehearsal']={'path':str(executor_path),'sha256':native.digest_file(executor_path)}
        self.raw=contract.canonical(self.request)
        self.digest=native.digest_bytes(self.raw)
        self.approval=contract.canonical({'schema_version':1,'operation':policy.OPERATION,
            'request_sha256':self.digest,'inventory_sha256':self.request['inventory_sha256'],
            'target_manifest_sha256':self.request['next']['manifest_digest'],'rehearsal_sha256':self.request['rehearsal']['sha256'],
            'executor_rehearsal_sha256':self.request['executor_rehearsal']['sha256'],
            'security_plan_sha256':self.request['security_plan_sha256'],'maintenance':'BOTH_NATIVE_WRITERS_FENCED',
            'recovery':'FORWARD_ONLY_FROM_MIGRATE_INTENT'})
        self.host=HostDouble()

    def execute(self):
        return live.execute(self.request,self.raw,self.approval,self.inventory,self.old,self.new,self.host,state=self.state)

    def recovery_inputs(self,abort=False):
        marker=native.pinned(self.state/'in-progress.json',None)
        inventory=contract.canonical({'scope':'fresh synthetic recovery inventory'})
        value={'schema_version':1,'operation':policy.OPERATION+('_ABORT_BEFORE_MIGRATE' if abort else '_FORWARD_RESUME'),'request_sha256':self.digest,
            'attempt':marker['attempt'],'marker_sha256':native.digest_file(self.state/'in-progress.json'),
            'inventory_sha256':native.digest_bytes(inventory),'observed_at':int(time.time()),
            'target_manifest_sha256':self.request['next']['manifest_digest'],
            'security_plan_sha256':self.request['security_plan_sha256'],
            'recovery':'UNCHANGED_UNENROLLED_PAIR_ONLY' if abort else 'PRESERVE_CURRENT_PAIR_AND_EXTERNAL_FLOORS'}
        return contract.canonical(value),inventory

    def resume(self):
        value,inventory=self.recovery_inputs()
        return live.resume_forward(self.request,self.raw,value,inventory,self.old,self.new,self.host,state=self.state)

    def test_happy_path_preserves_cold_original_and_config_and_advances_once(self):
        result=self.execute()
        self.assertEqual(result['generation'],3)
        self.assertFalse((self.state/'in-progress.json').exists())
        attempt=self.state/result['attempt']
        for service in native.SERVICES:
            filename='server.db' if service=='cantos-server' else 'identity.db'
            self.assertEqual((attempt/(service+'-database')/filename).read_bytes(),b'original current data\n')
        self.assertLess(self.host.events.index('pull'),self.host.events.index('stop'))
        self.assertLess(self.host.events.index('provision'),self.host.events.index('ready'))
        self.assertLess(self.host.events.index('ready'),self.host.events.index('start-new'))
        with self.assertRaises(ValueError):self.execute()

    def test_failed_pull_stops_nothing_and_creates_no_intent_or_authority(self):
        self.host.failure='pull'
        with self.assertRaisesRegex(ValueError,'pull failed'):self.execute()
        self.assertNotIn('stop',self.host.events)
        self.assertFalse((self.state/'in-progress.json').exists())
        self.assertFalse(Path(policy.SECURITY_ROOT).exists())

    def test_partial_migration_refuses_blind_replay_then_reconciles_forward(self):
        self.host.failure='second-live-migrate'
        with self.assertRaisesRegex(ValueError,'unknown migrate'):self.execute()
        self.assertEqual(native.pinned(self.state/'in-progress.json',None)['phase'],'MIGRATE_INTENT')
        with self.assertRaisesRegex(ValueError,'interrupted'):self.execute()
        self.assertEqual(self.resume()['generation'],3)
        self.assertFalse((self.state/'in-progress.json').exists())
        self.assertTrue(all(b'original current data' in (Path(path)/('server.db' if service=='cantos-server' else 'identity.db')).read_bytes()
                            for service,path in self.request['databases'].items()))

    def test_lost_enrollment_ack_keeps_the_same_committed_floor(self):
        self.host.failure='enroll-after-commit'
        with self.assertRaisesRegex(ValueError,'lost enrollment'):self.execute()
        floor=Path(policy.security_layout()['cantos-server']['path'])/'current'
        original=floor.read_bytes()
        self.assertEqual(self.resume()['generation'],3)
        self.assertEqual(floor.read_bytes(),original)

    def test_lost_floor_after_enrollment_is_never_regenerated(self):
        self.host.failure='enroll-after-commit'
        with self.assertRaises(ValueError):self.execute()
        floor=Path(policy.security_layout()['cantos-server']['path'])/'current';floor.unlink()
        with self.assertRaisesRegex(ValueError,'missing enrolled authority'):self.resume()
        self.assertFalse(floor.exists())
        self.assertNotIn('start-new',self.host.events)

    def test_poststart_unknown_ack_preserves_new_writes_on_explicit_forward_resume(self):
        self.host.accept_write=True
        with self.assertRaisesRegex(ValueError,'lost start'):self.execute()
        self.assertIn(b'blocked ',(Path(policy.SECURITY_ROOT)/'exposure/barrier').read_bytes())
        before={s:(Path(p)/('server.db' if s=='cantos-server' else 'identity.db')).read_bytes()
                for s,p in self.request['databases'].items()}
        self.assertTrue(all(b'accepted-after-start' in data for data in before.values()))
        self.assertEqual(self.resume()['generation'],3)
        for service,path in self.request['databases'].items():
            self.assertEqual((Path(path)/('server.db' if service=='cantos-server' else 'identity.db')).read_bytes(),before[service])

    def test_wrong_approval_has_zero_runtime_effects(self):
        self.approval=self.approval.replace(b'FORWARD_ONLY_FROM_MIGRATE_INTENT',b'RESTORE_OLD_PAIR')
        with self.assertRaisesRegex(ValueError,'approval must bind'):self.execute()
        self.assertEqual(self.host.events,[])

    def test_partial_unit_install_is_reconciled_to_one_exact_target_pair(self):
        replace=native.replace_unit
        count=0
        def interrupted(path,data,metadata):
            nonlocal count
            count+=1
            replace(path,data,metadata)
            if count==1:raise ValueError('unit install acknowledgement lost')
        with patch.object(native,'replace_unit',side_effect=interrupted):
            with self.assertRaisesRegex(ValueError,'unit install'):self.execute()
        self.assertEqual(native.pinned(self.state/'in-progress.json',None)['phase'],'UNIT_INTENT')
        self.assertEqual(self.resume()['generation'],3)
        for service in native.SERVICES:
            self.assertIn(self.new['images'][service]['ref'].encode(),(native.UNITS/(service+'.container')).read_bytes())

    def test_current_commit_with_lost_marker_ack_is_finalized_without_restarting_or_migrating(self):
        atomic=native.atomic
        def interrupted(path,data,mode=0o600):
            atomic(path,data,mode)
            if path==self.state/'current.json':raise ValueError('current commit acknowledgement lost')
        with patch.object(native,'atomic',side_effect=interrupted):
            with self.assertRaisesRegex(ValueError,'current commit'):self.execute()
        before=list(self.host.events)
        self.assertEqual(self.resume()['status'],'RECONCILED_COMMITTED_V36')
        self.assertEqual(self.host.events,before+['observe-target'])
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_corrupt_cold_cut_refuses_forward_recovery_before_runtime_effects(self):
        self.host.failure='second-live-migrate'
        with self.assertRaises(ValueError):self.execute()
        marker=native.pinned(self.state/'in-progress.json',None)
        path=self.state/marker['attempt']/'cantos-server-database/server.db'
        path.write_bytes(b'foreign old cut')
        before=list(self.host.events)
        with self.assertRaisesRegex(ValueError,'cold cut drift'):self.resume()
        self.assertEqual(self.host.events,before)

    def test_explicit_abort_before_migration_preserves_the_original_pair_and_generation(self):
        phase=live.phase
        def interrupted(state,attempt,digest,name):
            result=phase(state,attempt,digest,name)
            if name=='STOP_INTENT':raise ValueError('interrupted before stop')
            return result
        with patch.object(live,'phase',side_effect=interrupted):
            with self.assertRaisesRegex(ValueError,'interrupted before stop'):self.execute()
        value,inventory=self.recovery_inputs(abort=True)
        result=live.abort_before_migrate(self.request,self.raw,value,inventory,self.old,self.new,self.host,state=self.state)
        self.assertEqual(result['generation'],2)
        self.assertFalse((self.state/'in-progress.json').exists())
        self.assertFalse(Path(policy.SECURITY_ROOT).exists())
        for service,path in self.request['databases'].items():
            filename='server.db' if service=='cantos-server' else 'identity.db'
            self.assertEqual((Path(path)/filename).read_bytes(),b'original current data\n')
            self.assertEqual((native.UNITS/(service+'.container')).read_bytes(),self.original_units[service])

    def test_old_pair_abort_is_refused_after_any_migration_intent(self):
        self.host.failure='second-live-migrate'
        with self.assertRaises(ValueError):self.execute()
        value,inventory=self.recovery_inputs(abort=True)
        before=list(self.host.events)
        with self.assertRaisesRegex(ValueError,'old image resume forbidden'):
            live.abort_before_migrate(self.request,self.raw,value,inventory,self.old,self.new,self.host,state=self.state)
        self.assertEqual(self.host.events,before)
