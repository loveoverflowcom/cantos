"""Pure and file-fixture recovery contracts; never a live-host qualification."""
import copy
import time
import unittest

import release_native as native
import release_native_forward as forward
import release_native_migration as migration
from test_release_native_migration import adoption_request, releases


class ForwardDecisions(unittest.TestCase):
    def setUp(self):
        self.old, self.running = releases()
        self.original = adoption_request()
        self.now = int(time.time())
        self.attempt = 'attempt-'+'a'*32
        self.marker = {'attempt':self.attempt, 'operation':migration.OPERATION, 'phase':'START_INTENT',
                       'request_sha256':'b'*64, 'backup_sha256':'c'*64}
        self.request = {'schema_version':1, 'operation':forward.FORWARD, 'observed_at':self.now,
                        'attempt':self.attempt, 'marker_sha256':'d'*64, 'request_sha256':'b'*64,
                        'prior_intent_sha256':'e'*64, 'prior_receipt_sha256':'f'*64,
                        'prior_alignment_sha256':'1'*64, 'inventory_sha256':'2'*64,
                        'units':dict.fromkeys(native.SERVICES,'3'*64), 'executor_source':'4'*40,
                        'executor_rehearsal':{'path':'/private/executor.json','sha256':'5'*64}}
        self.intent = {'operation':'AUTHORIZED_FORWARD_PRESTART_PIN_RECOVERY', 'database_restore':'FORBIDDEN',
                       'request_sha256':'b'*64, 'start_intent_sha256':'d'*64}
        self.receipt = {'operation':self.intent['operation'], 'attempt':self.attempt,
                        'status':'NATIVE_HEALTH_AND_INGRESS_RESTORED', 'phase':'START_INTENT_RETAINED',
                        'database_restore':'NOT_RUN', 'generation':'NOT_COMMITTED', 'journal_preserved':True,
                        'backup_sha256':'c'*64, 'source_sha':self.running['source_sha']}
        self.alignment = {'status':'APPLIED', 'only_change':'EXACT_PRESTART_IMAGE_DIGESTS',
                          'units':self.request['units'].copy()}

    def test_only_existing_start_intent_and_fresh_exact_inventory_are_accepted(self):
        result = forward.forward_request(self.request, self.original, self.marker, self.old, self.running, self.now)
        self.assertEqual((result['status'],result['generation'],result['database_restore']),
                         ('PLAN_ONLY','NOT_COMMITTED','NOT_RUN'))
        for field, value in [('phase','MIGRATE_INTENT'),('attempt','attempt-'+'9'*32),
                             ('request_sha256','4'*64),('backup_sha256','bad')]:
            marker=copy.deepcopy(self.marker); marker[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):
                forward.forward_request(self.request,self.original,marker,self.old,self.running,self.now)
        for field,value in [('observed_at',self.now-3601),('operation','OTHER'),('attempt','../foreign'),
                            ('units',dict.fromkeys(native.SERVICES,'bad'))]:
            request=copy.deepcopy(self.request);request[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):
                forward.forward_request(request,self.original,self.marker,self.old,self.running,self.now)

    def test_previous_receipt_is_bound_and_never_claims_generation(self):
        forward.prior_evidence(self.request,self.marker,self.intent,self.receipt,self.alignment,self.running)
        for target,field,value in [('intent','database_restore','DONE'),('intent','start_intent_sha256','0'*64),
                                   ('receipt','generation','COMMITTED'),('receipt','journal_preserved',False),
                                   ('receipt','backup_sha256','0'*64),('receipt','source_sha','0'*40),
                                   ('alignment','only_change','ALL'),('alignment','status','NOT_RUN')]:
            objects=[copy.deepcopy(self.intent),copy.deepcopy(self.receipt),copy.deepcopy(self.alignment)]
            objects[['intent','receipt','alignment'].index(target)][field]=value
            with self.subTest(target=target,field=field),self.assertRaises(ValueError):
                forward.prior_evidence(self.request,self.marker,*objects,self.running)

    def test_only_image_and_prestart_digest_change(self):
        old,new='old-ref','new-ref'
        original=b'[Service]\nExecStartPre=/safe/tool old-ref\n[Container]\nImage=old-ref\nVolume=/private:/db\n'
        before=native.unit_image(original,old,new)
        current=before.replace(old.encode(),new.encode())
        self.assertEqual(forward.forward_unit(original,before,current,old,new),native.digest_bytes(current))
        later=forward.migration_unit(current,new,'next-ref')
        self.assertEqual(later,current.replace(new.encode(),b'next-ref'))
        for bad in [current+b'Environment=X\n',current.replace(b'/safe/tool',b'/other/tool'),
                    current.replace(b'ExecStartPre=',b'ExecStartPost=')]:
            with self.assertRaises(ValueError):forward.forward_unit(original,before,bad,old,new)
        for bad in [current.replace(b'ExecStartPre=',b'ExecStartPost='),current+b'Image=new-ref\n',
                    current.replace(b'new-ref',b'unknown',1)]:
            with self.assertRaises(ValueError):forward.migration_unit(bad,new,'next-ref')

    def test_forward_approval_binds_fence_and_current_candidate(self):
        approval={'schema_version':1,'operation':forward.FORWARD,'request_sha256':'4'*64,
                  'inventory_sha256':self.request['inventory_sha256'],'manifest_sha256':'5'*64,
                  'maintenance':'BOTH_NATIVE_WRITERS_FENCED','recovery':'PRESERVE_POST_START_WRITES'}
        forward.forward_approval(approval,self.request,'4'*64,'5'*64)
        for field,value in [('request_sha256','9'*64),('inventory_sha256','9'*64),
                            ('manifest_sha256','9'*64),('maintenance','UNFENCED'),('recovery','RESTORE_OLD')]:
            altered=approval|{field:value}
            with self.subTest(field=field),self.assertRaises(ValueError):
                forward.forward_approval(altered,self.request,'4'*64,'5'*64)

    def test_only_v24_to_v27_with_other_contracts_unchanged(self):
        before=copy.deepcopy(self.running);after=copy.deepcopy(before)
        after['compatibility']['server_schema']=forward.SCHEMA_27
        forward.migration_transition(before,after)
        for change in [('server_schema',migration.SCHEMA_24),('rpc','different'),('security_state','different')]:
            bad=copy.deepcopy(after);bad['compatibility'][change[0]]=change[1]
            with self.subTest(change=change),self.assertRaises(ValueError):forward.migration_transition(before,bad)
        with self.assertRaises(ValueError):forward.migration_transition(self.old,after)



# The following file-system tests use a named host double. They exercise durability and
# sequencing, not Podman, Turso integrity, production access or a real operator admission.

import json
from pathlib import Path
import tempfile
from unittest.mock import patch
import release_artifact as contract
from test_release_native import FakeHost


class ForwardHost(FakeHost):
    def ensure_capacity(self, request):
        self.event('capacity')

    def verify_current(self, layout, running):
        self.event('verify-current')


class ForwardFiles(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name).resolve()
        self.state = self.root/'state'; self.state.mkdir(mode=0o700)
        self.units = self.root/'units'; self.units.mkdir(mode=0o700)
        self.patch_units = patch.object(native,'UNITS',self.units); self.patch_units.start(); self.addCleanup(self.patch_units.stop)
        # Effect sequencing uses private temporary paths in place of the known production layout;
        # pure validation separately checks that the real path profile cannot be widened.
        self.patch_decide = patch.object(migration,'decide',return_value={}); self.patch_decide.start(); self.addCleanup(self.patch_decide.stop)
        self.old, self.running = releases(); self.original = adoption_request()
        self.original['next'] = {'source_sha':self.running['source_sha'],
                                 'configuration_digest':self.running['configuration_sha256'],
                                 'platform':self.running['platform'],'manifest_digest':'9'*64}
        self.original['databases'] = {}
        self.config = self.root/'native.env'; self.config.write_bytes(b'synthetic config');self.config.chmod(0o600)
        self.original['configuration_files'] = {str(self.config):native.digest_file(self.config)}
        self.attempt = self.state/('attempt-'+'a'*32);self.attempt.mkdir(mode=0o700)
        (self.attempt/'configuration').mkdir(mode=0o700)
        native.atomic(self.attempt/'configuration'/'0',self.config.read_bytes())
        config_record={str(self.config):{**native.unit_metadata(self.config),'sha256':native.digest_file(self.config),'copy':'0'}}
        native.atomic(self.attempt/'configuration-metadata.json',contract.canonical(config_record))
        former=self.attempt/'forward-recovery';former.mkdir(mode=0o700)
        units={}; metadata={}; historical={}
        for service in native.SERVICES:
            db=self.root/(service+'-live');db.mkdir(mode=0o700);self.original['databases'][service]=str(db)
            fname='server.db' if service=='cantos-server' else 'identity.db'
            (db/fname).write_bytes(b'current data after historical start');(db/(fname+'.owner')).write_bytes(b'')
            (db/(fname+'.owner')).chmod(0o600)
            backup=self.attempt/(service+'-database');migration.copy_database(db,backup,service)
            historical[service]=migration.data_inventory(backup,service)
            oldref=self.old['images'][service]['ref'];newref=self.running['images'][service]['ref']
            unit=(f'[Service]\nExecStartPre=/safe/tool {oldref}\n[Container]\nImage={oldref}\n').encode()
            before=native.unit_image(unit,oldref,newref);corrected=before.replace(oldref.encode(),newref.encode())
            (self.attempt/(service+'.container')).write_bytes(unit);(former/(service+'.before')).write_bytes(before)
            path=self.units/(service+'.container');path.write_bytes(corrected);path.chmod(0o600)
            self.original['units'][service]=native.digest_bytes(unit); units[service]=native.digest_bytes(corrected)
            metadata[service]=native.unit_metadata(path)
        native.atomic(self.attempt/'unit-metadata.json',contract.canonical(metadata))
        original_bytes=contract.canonical(self.original);native.atomic(self.attempt/'request.json',original_bytes)
        original_digest=native.digest_bytes(original_bytes)
        original_approval={'schema_version':1,'operation':migration.OPERATION,'request_sha256':original_digest,
                           'inventory_sha256':self.original['inventory_sha256'],'rehearsal_sha256':self.original['rehearsal']['sha256'],
                           'target_manifest_sha256':'9'*64,'maintenance':'BOTH_NATIVE_WRITERS_FENCED',
                           'recovery':'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START'}
        native.atomic(self.attempt/'adoption-approval.json',contract.canonical(original_approval))
        native.atomic(self.attempt/'backup.json',contract.canonical({'baseline':self.old,'request_sha256':original_digest,
                                                                     'databases':historical,'cut':'BOTH_OWNERS_STOPPED'}))
        marker={'attempt':self.attempt.name,'operation':migration.OPERATION,'phase':'START_INTENT',
                'request_sha256':original_digest,'backup_sha256':native.digest_file(self.attempt/'backup.json')}
        native.atomic(self.state/'in-progress.json',contract.canonical(marker));self.marker_bytes=(self.state/'in-progress.json').read_bytes()
        intent={'operation':'AUTHORIZED_FORWARD_PRESTART_PIN_RECOVERY','database_restore':'FORBIDDEN',
                'request_sha256':original_digest,'start_intent_sha256':native.digest_bytes(self.marker_bytes)}
        receipt={'operation':intent['operation'],'attempt':self.attempt.name,'status':'NATIVE_HEALTH_AND_INGRESS_RESTORED',
                 'phase':'START_INTENT_RETAINED','database_restore':'NOT_RUN','generation':'NOT_COMMITTED',
                 'journal_preserved':True,'backup_sha256':marker['backup_sha256'],'source_sha':self.running['source_sha']}
        alignment={'status':'APPLIED','only_change':'EXACT_PRESTART_IMAGE_DIGESTS','units':units}
        for name,value in [('intent.json',intent),('receipt.json',receipt),('unit-alignment.json',alignment)]:
            native.atomic(former/name,contract.canonical(value))
        self.inventory=contract.canonical({'synthetic':'inventory'})
        executor_path=self.root/'forward-executor-rehearsal.json'
        executor={'schema_version':1,'operation':forward.FORWARD+'_EXECUTOR_REHEARSAL','status':'PASS',
                  'executor_source':'4'*40,'previous':migration.new_identity(self.running),'target':migration.new_identity(self.running),
                  'checks':forward.FORWARD_EXECUTOR_CHECKS,'evidence_level':'integration-tested',
                  'execution_environment':'linux-native-systemd-podman-exact-images','activation':'NOT_RUN'}
        native.atomic(executor_path,contract.canonical(executor))
        self.request={'schema_version':1,'operation':forward.FORWARD,'observed_at':int(time.time()),
                      'attempt':self.attempt.name,'marker_sha256':native.digest_bytes(self.marker_bytes),
                      'request_sha256':original_digest,'prior_intent_sha256':native.digest_file(former/'intent.json'),
                      'prior_receipt_sha256':native.digest_file(former/'receipt.json'),
                      'prior_alignment_sha256':native.digest_file(former/'unit-alignment.json'),
                      'inventory_sha256':native.digest_bytes(self.inventory),'units':units,
                      'executor_source':'4'*40,'executor_rehearsal':{'path':str(executor_path),'sha256':native.digest_file(executor_path)}}
        self.raw=contract.canonical(self.request)
        self.approval=contract.canonical({'schema_version':1,'operation':forward.FORWARD,
                     'request_sha256':native.digest_bytes(self.raw),'inventory_sha256':self.request['inventory_sha256'],
                     'manifest_sha256':'9'*64,'maintenance':'BOTH_NATIVE_WRITERS_FENCED','recovery':'PRESERVE_POST_START_WRITES'})

    def execute(self,host):
        return forward.forward_execute(self.request,self.raw,self.approval,self.inventory,self.old,self.running,host,state=self.state)

    def test_fresh_cold_pair_and_original_intent_are_retained_before_truthful_commit(self):
        def accepted_before_fence():
            (Path(self.original['databases']['cantos-server'])/'server.db').write_bytes(b'write accepted before stop')
        host=ForwardHost(on_stop=accepted_before_fence);result=self.execute(host)
        self.assertEqual(result['status'],'OBSERVED_FORWARD_RECONCILED')
        self.assertFalse((self.state/'in-progress.json').exists())
        operation=self.state/result['attempt']
        self.assertEqual((operation/'original-intent.json').read_bytes(),self.marker_bytes)
        self.assertEqual((operation/'cantos-server-database'/'server.db').read_bytes(),b'write accepted before stop')
        self.assertEqual((self.attempt/'cantos-server-database'/'server.db').read_bytes(),b'current data after historical start')
        self.assertEqual(host.events.count('verify-current'),2)
        self.assertLess(host.events.index('stop'),host.events.index('verify-current'))
        self.assertLess(host.events.index('verify-current'),host.events.index('start-2'))
        with self.assertRaises((ValueError,OSError)):self.execute(ForwardHost())

    def test_each_failure_retains_original_marker_and_never_restores_prior_bytes(self):
        for fail in ('observe','stop','recovery-observe','verify-current','start-2'):
            with self.subTest(fail=fail):
                if fail != 'observe':
                    self.tearDown();self.setUp()
                host=ForwardHost(fail=fail)
                with self.assertRaises(ValueError):self.execute(host)
                self.assertEqual((self.state/'in-progress.json').read_bytes(),self.marker_bytes)
                self.assertFalse((self.state/'current.json').exists())
                for service in native.SERVICES:
                    filename='server.db' if service=='cantos-server' else 'identity.db'
                    self.assertEqual((Path(self.original['databases'][service])/filename).read_bytes(),
                                     b'current data after historical start')

    def test_current_written_before_unlink_replays_only_after_running_reobservation(self):
        unlink=Path.unlink
        def fail_marker(path, *args, **kwargs):
            if path == self.state/'in-progress.json': raise OSError('injected crash')
            return unlink(path,*args,**kwargs)
        with patch.object(Path,'unlink',fail_marker),self.assertRaises(OSError):self.execute(ForwardHost())
        self.assertTrue((self.state/'current.json').exists())
        self.assertEqual((self.state/'in-progress.json').read_bytes(),self.marker_bytes)
        retry=ForwardHost();result=self.execute(retry)
        self.assertEqual(result['status'],'RECONCILED_AFTER_INTERRUPTION')
        self.assertEqual(retry.events,['observe'])
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_configuration_or_unit_drift_refuses_before_any_host_effect(self):
        for name in ['configuration','unit']:
            with self.subTest(name=name):
                if name == 'configuration': self.config.write_bytes(b'changed')
                else: (self.units/'cantos-server.container').write_bytes(b'foreign unit')
                host=ForwardHost()
                with self.assertRaises(ValueError):self.execute(host)
                self.assertEqual(host.events,[])
                self.assertEqual((self.state/'in-progress.json').read_bytes(),self.marker_bytes)
                if name == 'configuration': self.config.write_bytes(b'synthetic config')




class HostBoundaryTests(unittest.TestCase):
    def test_duplicate_operator_json_is_refused(self):
        with self.assertRaises(ValueError):forward.decode_unique(b'{"operation":"x","operation":"y"}')

    def test_foreign_database_bind_refuses_even_without_an_open_file(self):
        request={'databases':{'cantos-server':'/root/cantos-native/server-db',
                              'cantos-identity':'/root/cantos-native/identity-db'}}
        system=forward.ForwardSystem();identity='a'*64
        def command(argv):
            if argv[1]=='ps': return json.dumps([{'Id':identity,'Names':['other']}]).encode()
            if argv[1:3]==['container','inspect']:
                return json.dumps([{'Mounts':[{'Type':'bind','Source':'/root/cantos-native/server-db/subdir'}]}]).encode()
            raise AssertionError('unexpected command')
        with patch.object(migration.AdoptionSystem,'recovery_host'),patch.object(system,'command',side_effect=command):
            with self.assertRaisesRegex(ValueError,'another container'):
                system.recovery_host(request,{}, {})




class ForwardResume(unittest.TestCase):
    setUp=ForwardFiles.setUp
    execute=ForwardFiles.execute

    def inputs(self):
        attempts=sorted(self.state.glob('forward-*'))
        self.assertEqual(len(attempts),1)
        prior=attempts[0].name
        inventory=contract.canonical({'synthetic':'fresh inventory'})
        approval=contract.canonical({'schema_version':1,'operation':forward.RESUME_FORWARD,
                 'request_sha256':native.digest_bytes(self.raw),'prior_attempt':prior,
                 'marker_sha256':self.request['marker_sha256'],'inventory_sha256':native.digest_bytes(inventory),
                 'manifest_sha256':'9'*64,'maintenance':'BOTH_NATIVE_WRITERS_FENCED',
                 'recovery':'PRESERVE_POST_START_WRITES','observed_at':int(time.time())})
        return prior,approval,inventory

    def resume(self,prior,approval,inventory,host):
        return forward.forward_execute(self.request,self.raw,self.approval,self.inventory,self.old,self.running,host,
               state=self.state,resume_attempt=prior,resume_approval_bytes=approval,resume_inventory_bytes=inventory)

    def test_interrupted_after_stop_resumes_only_with_fresh_approval_and_current_data(self):
        with self.assertRaises(ValueError):self.execute(ForwardHost(fail='verify-current'))
        self.assertEqual((self.state/'in-progress.json').read_bytes(),self.marker_bytes)
        prior,approval,inventory=self.inputs()
        def before_stop():
            (Path(self.original['databases']['cantos-server'])/'server.db').write_bytes(b'accepted after interruption')
        host=ForwardHost(on_stop=before_stop);result=self.resume(prior,approval,inventory,host)
        self.assertEqual(result['status'],'OBSERVED_FORWARD_RECONCILED')
        attempt=self.state/result['attempt']
        self.assertEqual((attempt/'cantos-server-database'/'server.db').read_bytes(),b'accepted after interruption')
        self.assertEqual((self.attempt/'cantos-server-database'/'server.db').read_bytes(),b'current data after historical start')
        self.assertEqual(native.pinned(attempt/'prior-attempt.json',None)['attempt'],prior)
        self.assertLess(host.events.index('recovery-observe'),host.events.index('stop'))
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_resume_rejects_stale_approval_and_modified_prior_intent_without_host_effect(self):
        with self.assertRaises(ValueError):self.execute(ForwardHost(fail='verify-current'))
        prior,approval,inventory=self.inputs();bad=forward.decode_unique(approval);bad['observed_at']=int(time.time())-3601
        host=ForwardHost()
        with self.assertRaises(ValueError):self.resume(prior,contract.canonical(bad),inventory,host)
        self.assertEqual(host.events,[])
        (self.state/prior/'phase-stop-intent.json').write_bytes(b'{}')
        with self.assertRaises(ValueError):self.resume(prior,approval,inventory,host)
        self.assertEqual(host.events,[])
        self.assertEqual((self.state/'in-progress.json').read_bytes(),self.marker_bytes)



class ExecutorGate(unittest.TestCase):
    setUp=ForwardFiles.setUp

    def test_model_results_or_mismatched_executor_and_pair_never_qualify(self):
        path=Path(self.request['executor_rehearsal']['path'])
        original=native.pinned(path,self.request['executor_rehearsal']['sha256'])
        forward.executor_rehearsal(self.request,forward.FORWARD,self.running,self.running)
        for field,value in [('status','NOT_RUN'),('evidence_level','model-checked'),('checks',[]),
                            ('executor_source','0'*40),('activation','DONE'),
                            ('execution_environment','fake-host'),('previous',{})]:
            with self.subTest(field=field):
                changed=copy.deepcopy(original);changed[field]=value
                native.atomic(path,contract.canonical(changed))
                request=copy.deepcopy(self.request);request['executor_rehearsal']['sha256']=native.digest_file(path)
                with self.assertRaises(ValueError):forward.executor_rehearsal(request,forward.FORWARD,self.running,self.running)
        native.atomic(path,contract.canonical(original))



class PriorCustody(unittest.TestCase):
    setUp=ForwardFiles.setUp
    execute=ForwardFiles.execute

    def test_retained_configuration_copy_drift_refuses_before_host_effect(self):
        (self.attempt/'configuration'/'0').write_bytes(b'tampered')
        host=ForwardHost()
        with self.assertRaises(ValueError):self.execute(host)
        self.assertEqual(host.events,[])
        self.assertEqual((self.state/'in-progress.json').read_bytes(),self.marker_bytes)

    def test_completed_receipt_cannot_be_replayed_with_corrupt_current_backup(self):
        unlink=Path.unlink
        def fail_marker(path,*args,**kwargs):
            if path==self.state/'in-progress.json':raise OSError('injected crash')
            return unlink(path,*args,**kwargs)
        with patch.object(Path,'unlink',fail_marker),self.assertRaises(OSError):self.execute(ForwardHost())
        current=native.pinned(self.state/'current.json',None)
        folder=self.state/current['forward_reconciliation']['attempt']
        (folder/'cantos-server-database'/'server.db').write_bytes(b'tampered')
        host=ForwardHost()
        with self.assertRaises(ValueError):self.execute(host)
        self.assertEqual(host.events,[])
        self.assertEqual((self.state/'in-progress.json').read_bytes(),self.marker_bytes)

if __name__ == '__main__': unittest.main()
