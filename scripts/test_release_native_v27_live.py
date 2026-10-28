"""Synthetic decisions and filesystem failures; never actual image/host qualification."""
import copy
from pathlib import Path
import time
import unittest

import release_artifact as contract
import release_native as native
import release_native_adapter as adapter
import release_native_forward as forward
import release_native_migration as migration
import release_native_v27_live as live
from test_release_native import pair
import test_release_native_forward as fixtures


class LiveFiles(unittest.TestCase):
    setUp=fixtures.ForwardFiles.setUp

    def setUp(self):
        fixtures.ForwardFiles.setUp(self)
        fixtures.ForwardFiles.execute(self, fixtures.ForwardHost())
        self.current = native.pinned(self.state/'current.json',None)
        self.new = pair('3');self.new['compatibility'] = {**self.running['compatibility'], 'server_schema':forward.SCHEMA_27}
        self.rehearsal_path=self.root/'rehearsal.json'
        self.rehearsal={'operation':'HISTORICAL_V24_V27_REHEARSAL','status':'PASS',
                        'baseline':migration.new_identity(self.running),'target':migration.new_identity(self.new),
                        'input_manifest_sha256':'8'*64,'evidence_level':'integration-tested',
                        'execution_environment':'linux-native-exact-images','checks':live.CHECKS,
                        'host_adapter':{'status':'PASS','source_sha':self.new['source_sha'],
                                        'images':self.new['images'],'checks':adapter.CHECKS,'activation':'NOT_RUN'}}
        native.atomic(self.rehearsal_path,contract.canonical(self.rehearsal))
        self.executor_path=self.root/'v27-executor-rehearsal.json'
        executor={'schema_version':1,'operation':forward.MIGRATE+'_EXECUTOR_REHEARSAL','status':'PASS',
                  'executor_source':'6'*40,'previous':migration.new_identity(self.running),'target':migration.new_identity(self.new),
                  'checks':forward.MIGRATION_EXECUTOR_CHECKS,'evidence_level':'integration-tested',
                  'execution_environment':'linux-native-systemd-podman-exact-images','activation':'NOT_RUN'}
        native.atomic(self.executor_path,contract.canonical(executor))
        self.live_request={'schema_version':1,'operation':forward.MIGRATE,'observed_at':int(time.time()),
                           'inventory_sha256':'5'*64,'current_sha256':native.digest_file(self.state/'current.json'),
                           'generation':1,'current_manifest_sha256':'9'*64,
                           'forward_request_sha256':native.digest_bytes(self.raw), 'units':self.request['units'],
                           'next':{'manifest_digest':'8'*64,'source_sha':self.new['source_sha'],
                                   'configuration_digest':self.new['configuration_sha256'],'platform':self.new['platform']},
                           'rehearsal':{'path':str(self.rehearsal_path),'sha256':native.digest_file(self.rehearsal_path)},
                           'executor_source':'6'*40,'executor_rehearsal':{'path':str(self.executor_path),'sha256':native.digest_file(self.executor_path)}}

    def test_known_forward_generation_and_exact_compatible_transition_are_required(self):
        actual,original,prior,_,_=live.current_custody(self.state,self.live_request,self.old,self.running)
        self.assertEqual(actual,self.current);self.assertEqual(prior,self.request)
        self.assertEqual(live.decide(self.live_request,actual,self.running,self.new)['status'],'PLAN_ONLY')
        for name,value in [('generation',2),('current_manifest_sha256','0'*64),
                           ('observed_at',int(time.time())-3601),('operation','OTHER')]:
            with self.subTest(name=name),self.assertRaises(ValueError):
                live.decide(self.live_request|{name:value},actual,self.running,self.new)
        with self.assertRaises(ValueError):
            live.current_custody(self.state,self.live_request|{'forward_request_sha256':'1'*64},self.old,self.running)

    def test_rehearsal_must_pin_actual_old_pair_and_target_and_real_adapter(self):
        live.rehearsal(self.live_request,self.running,self.new)
        for field,value in [('baseline',migration.new_identity(self.old)),('checks',[]),('status','NOT_RUN'),
                            ('input_manifest_sha256','f'*64),('evidence_level','model-checked'),
                            ('host_adapter',{'status':'PASS'})]:
            with self.subTest(field=field):
                altered=copy.deepcopy(self.rehearsal);altered[field]=value
                native.atomic(self.rehearsal_path,contract.canonical(altered))
                request=copy.deepcopy(self.live_request);request['rehearsal']['sha256']=native.digest_file(self.rehearsal_path)
                with self.assertRaises(ValueError):live.rehearsal(request,self.running,self.new)
        native.atomic(self.rehearsal_path,contract.canonical(self.rehearsal))

    def test_migration_approval_never_authorizes_restore_after_start(self):
        a={'schema_version':1,'operation':forward.MIGRATE,'request_sha256':'1'*64,
           'inventory_sha256':'2'*64,'rehearsal_sha256':'3'*64,'target_manifest_sha256':'4'*64,
           'maintenance':'BOTH_NATIVE_WRITERS_FENCED','recovery':'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START'}
        forward.migration_approval(a,'1'*64,'2'*64,'3'*64,'4'*64)
        for key,value in [('recovery','RESTORE_ALWAYS'),('maintenance','UNFENCED'),('target_manifest_sha256','5'*64)]:
            with self.subTest(key=key),self.assertRaises(ValueError):
                forward.migration_approval(a|{key:value},'1'*64,'2'*64,'3'*64,'4'*64)



class LiveHost(fixtures.ForwardHost):
    def migrate(self, request, target):
        path=Path(request['databases']['cantos-server'])/'server.db'
        label='migrate-copy' if 'restore-check' in str(path) else 'migrate-live'
        path.write_bytes(b'synthetic-v27')
        self.event(label)

    def ensure_capacity(self, request):
        self.event('capacity')

    def require_stopped(self, request):
        self.event('require-stopped')

    def assert_closed(self, paths):
        self.event('assert-closed')

    def ensure_restore_capacity(self, request, attempt):
        self.event('restore-capacity')


class LiveEffects(unittest.TestCase):
    setUp=LiveFiles.setUp

    def setUp(self):
        LiveFiles.setUp(self)
        self.live_request['inventory_sha256']=native.digest_bytes(self.inventory)
        self.live_raw=contract.canonical(self.live_request)
        self.live_approval=contract.canonical({'schema_version':1,'operation':forward.MIGRATE,
               'request_sha256':native.digest_bytes(self.live_raw),'inventory_sha256':self.live_request['inventory_sha256'],
               'rehearsal_sha256':self.live_request['rehearsal']['sha256'],
               'target_manifest_sha256':self.live_request['next']['manifest_digest'],
               'maintenance':'BOTH_NATIVE_WRITERS_FENCED',
               'recovery':'RESTORE_BEFORE_START_FORWARD_ONLY_AFTER_START'})

    def execute_live(self,host):
        return live.execute(self.live_request,self.live_raw,self.live_approval,self.inventory,
                            self.old,self.running,self.new,host,state=self.state)

    def test_backup_and_restore_test_precede_migration_and_both_unit_pins_move(self):
        def accepted_before_stop():
            (Path(self.original['databases']['cantos-server'])/'server.db').write_bytes(b'accepted before fence')
        host=LiveHost(on_stop=accepted_before_stop);result=self.execute_live(host)
        self.assertEqual(result['status'],'OBSERVED_MIGRATED')
        attempt=self.state/result['attempt']
        self.assertEqual((attempt/'cantos-server-database'/'server.db').read_bytes(),b'accepted before fence')
        self.assertEqual((attempt/'cantos-server-restore-check'/'server.db').read_bytes(),b'synthetic-v27')
        self.assertEqual((Path(self.original['databases']['cantos-server'])/'server.db').read_bytes(),b'synthetic-v27')
        self.assertLess(host.events.index('migrate-copy'),host.events.index('migrate-live'))
        self.assertLess(host.events.index('migrate-live'),host.events.index('start-3'))
        for service in native.SERVICES:
            unit=(self.units/(service+'.container')).read_bytes()
            self.assertEqual(unit.count(self.new['images'][service]['ref'].encode()),2)
            self.assertEqual(unit.count(self.running['images'][service]['ref'].encode()),0)
        self.assertEqual(native.pinned(self.state/'current.json',None)['generation'],2)
        self.assertFalse((self.state/'in-progress.json').exists())
        with self.assertRaises((ValueError,OSError)):self.execute_live(LiveHost())

    def test_prestart_and_poststart_failures_never_claim_commit_or_restore_old_data(self):
        # Each failure uses an independent fixture, with no production processes or database.
        for fail in ('pull-3','stop','verify-current','migrate-copy','migrate-live','start-3'):
            with self.subTest(fail=fail):
                fixture=LiveEffects(methodName='test_backup_and_restore_test_precede_migration_and_both_unit_pins_move')
                fixture.setUp()
                try:
                    old_current=(fixture.state/'current.json').read_bytes()
                    with self.assertRaises(ValueError):fixture.execute_live(LiveHost(fail=fail))
                    marker=native.pinned(fixture.state/'in-progress.json',None)
                    self.assertEqual((fixture.state/'current.json').read_bytes(),old_current)
                    self.assertIn(marker['phase'],('PREPARE','MIGRATE_INTENT','START_INTENT'))
                    if fail=='start-3':self.assertEqual(marker['phase'],'START_INTENT')
                    if fail in ('migrate-live','start-3'):
                        self.assertEqual((Path(fixture.original['databases']['cantos-server'])/'server.db').read_bytes(),b'synthetic-v27')
                    with self.assertRaises(ValueError):fixture.execute_live(LiveHost())
                finally:
                    fixture.doCleanups()

    def test_wrong_baseline_or_unreviewed_host_changes_stop_before_mutation(self):
        self.new['compatibility']['rpc']='changed'
        host=LiveHost()
        with self.assertRaises(ValueError):self.execute_live(host)
        self.assertEqual(host.events,[])
        self.assertFalse((self.state/'in-progress.json').exists())



class LiveRecovery(unittest.TestCase):
    setUp=LiveEffects.setUp
    execute_live=LiveEffects.execute_live

    def recovery_inputs(self):
        marker=native.pinned(self.state/'in-progress.json',None)
        digest=native.digest_file(self.state/'in-progress.json')
        approval={'schema_version':1,'operation':live.ROLLBACK,'request_sha256':native.digest_bytes(self.live_raw),
                  'attempt':marker['attempt'],'marker_sha256':digest,'inventory_sha256':native.digest_bytes(self.inventory),
                  'previous_sha256':self.live_request['current_sha256'],'backup_sha256':marker.get('backup_sha256','NONE'),
                  'maintenance':'BOTH_NATIVE_WRITERS_FENCED','recovery':'RESTORE_ONLY_BEFORE_ANY_START',
                  'observed_at':int(time.time())}
        return contract.canonical(approval),marker

    def recover(self,approval,host):
        return live.rollback(self.live_request,self.live_raw,approval,self.inventory,self.old,self.running,self.new,host,state=self.state)

    def test_prestart_partial_migration_restores_complete_current_cold_pair(self):
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='migrate-live'))
        approval,marker=self.recovery_inputs();self.assertEqual(marker['phase'],'MIGRATE_INTENT')
        old_current=(self.state/'current.json').read_bytes();host=LiveHost();result=self.recover(approval,host)
        self.assertEqual(result['status'],'OBSERVED_V24_RESTORED')
        self.assertEqual((self.state/'current.json').read_bytes(),old_current)
        self.assertFalse((self.state/'in-progress.json').exists())
        for service in native.SERVICES:
            dest=Path(self.original['databases'][service]);fname='server.db' if service=='cantos-server' else 'identity.db'
            self.assertEqual((dest/fname).read_bytes(),b'current data after historical start')
            self.assertTrue(dest.with_name(dest.name+'.failed-'+marker['attempt']).exists())
        self.assertLess(host.events.index('require-stopped'),host.events.index('assert-closed'))
        self.assertLess(host.events.index('assert-closed'),host.events.index('start-2'))

    def test_after_start_intent_restore_is_refused_before_host_effect(self):
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='start-3'))
        approval,marker=self.recovery_inputs();self.assertEqual(marker['phase'],'START_INTENT')
        before=(Path(self.original['databases']['cantos-server'])/'server.db').read_bytes()
        host=LiveHost()
        with self.assertRaises(ValueError):self.recover(approval,host)
        self.assertEqual(host.events,[])
        self.assertEqual((Path(self.original['databases']['cantos-server'])/'server.db').read_bytes(),before)

    def test_a_crash_between_pair_renames_replays_without_deleting_either_copy(self):
        from unittest.mock import patch
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='migrate-live'))
        approval,marker=self.recovery_inputs();rename=Path.rename
        def fail_restore(path,target):
            if '.restore-'+marker['attempt'] in path.name:raise OSError('injected interrupted rename')
            return rename(path,target)
        with patch.object(Path,'rename',fail_restore),self.assertRaises(OSError):self.recover(approval,LiveHost())
        self.assertEqual(native.pinned(self.state/'in-progress.json',None)['phase'],'MIGRATE_INTENT')
        first=Path(self.original['databases']['cantos-server'])
        self.assertFalse(first.exists())
        self.assertTrue(first.with_name(first.name+'.failed-'+marker['attempt']).exists())
        self.assertTrue(first.with_name(first.name+'.restore-'+marker['attempt']).exists())
        self.recover(approval,LiveHost())
        self.assertTrue(first.exists())
        self.assertTrue(first.with_name(first.name+'.failed-'+marker['attempt']).exists())

    def test_unexpected_active_owner_or_corrupted_backup_blocks_restore(self):
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='migrate-live'))
        approval,marker=self.recovery_inputs();host=LiveHost(fail='require-stopped')
        with self.assertRaises(ValueError):self.recover(approval,host)
        self.assertNotIn('stop',host.events)
        saved=self.state/marker['attempt']/'cantos-server-database'/'server.db';saved.write_bytes(b'tampered')
        with self.assertRaises(ValueError):self.recover(approval,LiveHost())
        self.assertEqual((Path(self.original['databases']['cantos-server'])/'server.db').read_bytes(),b'synthetic-v27')



class LivePoststart(unittest.TestCase):
    setUp=LiveEffects.setUp
    execute_live=LiveEffects.execute_live

    def inputs(self):
        marker=native.pinned(self.state/'in-progress.json',None)
        approval={'schema_version':1,'operation':live.POSTSTART,'request_sha256':native.digest_bytes(self.live_raw),
                  'attempt':marker['attempt'],'marker_sha256':native.digest_file(self.state/'in-progress.json'),
                  'inventory_sha256':native.digest_bytes(self.inventory),'previous_sha256':self.live_request['current_sha256'],
                  'target_manifest_sha256':self.live_request['next']['manifest_digest'],'maintenance':'BOTH_NATIVE_WRITERS_FENCED',
                  'recovery':'PRESERVE_POST_START_WRITES','observed_at':int(time.time())}
        return contract.canonical(approval),marker

    def resume(self,approval,host):
        return live.poststart(self.live_request,self.live_raw,approval,self.inventory,
                              self.old,self.running,self.new,host,state=self.state)

    def test_after_uncertain_start_capture_current_data_and_never_restore_old_snapshot(self):
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='start-3'))
        approval,marker=self.inputs();self.assertEqual(marker['phase'],'START_INTENT')
        original_marker=(self.state/'in-progress.json').read_bytes()
        def accepted_after_start():
            (Path(self.original['databases']['cantos-server'])/'server.db').write_bytes(b'accepted after first start')
        host=LiveHost(on_stop=accepted_after_start);result=self.resume(approval,host)
        self.assertEqual(result['status'],'OBSERVED_FORWARD_RECONCILED')
        operation=self.state/result['attempt']
        self.assertEqual((operation/'original-intent.json').read_bytes(),original_marker)
        self.assertEqual((operation/'cantos-server-database'/'server.db').read_bytes(),b'accepted after first start')
        self.assertEqual((Path(self.original['databases']['cantos-server'])/'server.db').read_bytes(),b'accepted after first start')
        self.assertEqual((self.state/marker['attempt']/'cantos-server-database'/'server.db').read_bytes(),b'current data after historical start')
        self.assertLess(host.events.index('capacity'),host.events.index('stop'))
        self.assertEqual(native.pinned(self.state/'current.json',None)['generation'],2)
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_poststart_failure_keeps_intent_and_no_old_data_restore(self):
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='start-3'))
        approval,marker=self.inputs();host=LiveHost(fail='verify-current')
        before=(Path(self.original['databases']['cantos-server'])/'server.db').read_bytes()
        with self.assertRaises(ValueError):self.resume(approval,host)
        self.assertEqual(native.pinned(self.state/'in-progress.json',None),marker)
        self.assertEqual((Path(self.original['databases']['cantos-server'])/'server.db').read_bytes(),before)
        self.assertEqual(native.digest_file(self.state/'current.json'),self.live_request['current_sha256'])

    def test_migration_commit_crash_finishes_only_after_runtime_reobservation(self):
        from unittest.mock import patch
        unlink=Path.unlink
        def fail_marker(path,*args,**kwargs):
            if path==self.state/'in-progress.json':raise OSError('injected crash')
            return unlink(path,*args,**kwargs)
        with patch.object(Path,'unlink',fail_marker),self.assertRaises(OSError):self.execute_live(LiveHost())
        approval,_=self.inputs();host=LiveHost();result=self.resume(approval,host)
        self.assertEqual(result['status'],'RECONCILED_AFTER_INTERRUPTION')
        self.assertEqual(host.events,['observe'])
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_poststart_commit_crash_rechecks_receipt_and_backup_before_completion(self):
        from unittest.mock import patch
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='start-3'))
        approval,_=self.inputs();unlink=Path.unlink
        def fail_marker(path,*args,**kwargs):
            if path==self.state/'in-progress.json':raise OSError('injected crash')
            return unlink(path,*args,**kwargs)
        with patch.object(Path,'unlink',fail_marker),self.assertRaises(OSError):self.resume(approval,LiveHost())
        host=LiveHost();result=self.resume(approval,host)
        self.assertEqual(result['status'],'RECONCILED_AFTER_INTERRUPTION')
        self.assertEqual(host.events,['observe'])
        self.assertFalse((self.state/'in-progress.json').exists())

    def test_wrong_approval_and_corrupt_historical_backup_refuse_before_host_effect(self):
        with self.assertRaises(ValueError):self.execute_live(LiveHost(fail='start-3'))
        approval,marker=self.inputs();bad=forward.decode_unique(approval);bad['recovery']='RESTORE_OLD'
        host=LiveHost()
        with self.assertRaises(ValueError):self.resume(contract.canonical(bad),host)
        self.assertEqual(host.events,[])
        (self.state/marker['attempt']/'cantos-server-database'/'server.db').write_bytes(b'tampered')
        with self.assertRaises(ValueError):self.resume(approval,LiveHost())

if __name__ == '__main__': unittest.main()
