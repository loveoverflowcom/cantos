"""Pure initial-upgrade policy and unit-byte oracles, never host qualification."""
import copy
from pathlib import Path
import unittest

import release_artifact as contract
import release_native_migration as migration
import release_native_v36_policy as policy
from test_release_native_v36_rehearsal import pair


class InitialUpgradePolicy(unittest.TestCase):
    def fixture(self):
        old, new = pair()
        request = {'schema_version':1, 'operation':policy.OPERATION, 'observed_at':100,
            'host_machine_sha256':'a'*64, 'inventory_sha256':'b'*64, 'current_sha256':'c'*64,
            'generation':2, 'current_manifest_sha256':'d'*64, 'executor_source':'e'*40,
            'security_plan_sha256':contract.fingerprint(policy.security_layout()),
            'next':{'manifest_digest':'f'*64, 'source_sha':new['source_sha'],
                    'configuration_digest':new['configuration_sha256'], 'platform':new['platform']},
            'databases':migration.DATABASES, 'socket_directory':migration.SOCKET,
            'peer_policy':'trusted_proxy', 'configuration_files':{migration.ROOT+'/config/server.env':'1'*64},
            'units':{s:'2'*64 for s in policy.oci.SERVICES},
            'mounts':{s:[{'source':migration.DATABASES[s],'destination':'/database','read_only':False},
                        {'source':migration.SOCKET,'destination':'/identity','read_only':s=='cantos-server'}]
                      for s in policy.oci.SERVICES},
            'health_url':'http://127.0.0.1:8080/api/v1/theatre/works',
            'rehearsal':{'path':'/operator/exact-images.json','sha256':'3'*64},
            'executor_rehearsal':{'path':'/operator/executor.json','sha256':'4'*64}}
        current={'generation':2,'manifest_sha256':'d'*64,'images':old['images'],
                 'runtime_layout':{k:request[k] for k in ('databases','socket_directory','peer_policy')}}
        return request,current,old,new

    def test_exact_plan_remains_below_every_host_effect_and_admission_gate(self):
        request,current,old,new=self.fixture()
        result=policy.decide(request,current,old,new,100)
        self.assertEqual(result['status'],'PLAN_ONLY')
        for key in ('activation','admission','host_observation','executor_transition','security_provisioning'):
            self.assertEqual(result[key],'NOT_RUN')
        self.assertEqual(result['recovery'],'FORWARD_ONLY_FROM_MIGRATE_INTENT')

    def test_stale_future_foreign_generation_security_plan_and_target_are_refused(self):
        for name,value in [('observed_at',-3501),('observed_at',101),('generation',1),('generation',True),
                           ('current_manifest_sha256','0'*64),('security_plan_sha256','0'*64),('operation','OTHER')]:
            with self.subTest(name=name,value=value):
                request,current,old,new=self.fixture();request[name]=value
                with self.assertRaises(ValueError): policy.decide(request,current,old,new,100)
        request,current,old,new=self.fixture();new['compatibility']['identity_schema']='identity-migrations-v6'
        with self.assertRaisesRegex(ValueError,'only reviewed'): policy.decide(request,current,old,new,100)

    def test_other_owner_unbound_or_new_security_mount_is_not_an_old_profile(self):
        for mode in ('foreign_db','security','unbound_config','writable_flag'):
            request,current,old,new=self.fixture();request=copy.deepcopy(request)
            if mode=='foreign_db': request['mounts']['cantos-server'][0]['source']=migration.DATABASES['cantos-identity']
            elif mode=='security': request['mounts']['cantos-server'].append({'source':policy.SECURITY_ROOT+'/server','destination':'/floor','read_only':False})
            elif mode=='unbound_config': request['configuration_files']={'/root/foreign.env':'1'*64}
            else: request['mounts']['cantos-server'][0]['read_only']=0
            with self.subTest(mode=mode),self.assertRaises(ValueError): policy.decide(request,current,old,new,100)

    def test_unknown_or_migration_intent_never_permits_old_image_resume(self):
        for phase in policy.PHASES:
            self.assertEqual(policy.may_resume_old(phase),phase in ('PREPARE','STOP_INTENT','COLD_CUT','COPY_REHEARSAL'))
        with self.assertRaisesRegex(ValueError,'unknown migration phase'): policy.may_resume_old('UNKNOWN')

    def test_access_plan_has_only_each_owners_floor_and_shared_read_only_exposure(self):
        layout=policy.security_layout()
        for service,uid in [('cantos-server',10001),('cantos-identity',10002)]:
            self.assertEqual((layout[service]['uid'],layout[service]['gid'],layout[service]['mode'],layout[service]['file_mode']),
                             (uid,uid,0o700,0o600))
        self.assertTrue(layout['exposure']['read_only'])
        self.assertFalse(layout['operations']['service_mount'])
        before=contract.fingerprint(layout);layout['cantos-server']['uid']=0
        self.assertNotEqual(before,contract.fingerprint(layout))


class UnitTransform(unittest.TestCase):
    def fixture(self,service,newline='\n'):
        old='ghcr.io/loveoverflowcom/'+service+'@sha256:'+'1'*64
        new='ghcr.io/loveoverflowcom/'+service+'@sha256:'+'2'*64
        lines=['[Unit]','Description=Keep exact bytes','[Service]',
               'ExecStartPre=/usr/bin/podman image exists '+old,'[Container]',
               'Image='+old,'EnvironmentFile=/root/cantos-native/config/owner.env',
               'Network=existing','User='+('10001:10001' if service=='cantos-server' else '10002:10002'),
               'Volume=/root/cantos-native/existing:/existing','[Install]','WantedBy=multi-user.target']
        return (newline.join(lines)+newline).encode(),old,new

    def test_only_two_image_pins_and_five_exact_approved_lines_change_for_each_owner(self):
        for service in policy.oci.SERVICES:
            for newline in ('\n','\r\n'):
                data,old,new=self.fixture(service,newline)
                result=policy.target_unit(data,service,old,new,'a'*32)
                extra=[line for line in result.decode().splitlines() if line.startswith('Environment=CANTOS_') or
                       line.startswith('Volume='+policy.SECURITY_ROOT)]
                self.assertEqual(len(extra),5)
                self.assertEqual(sum(line.startswith('Volume=') for line in extra),2)
                self.assertIn('Volume='+policy.SECURITY_ROOT+'/exposure:'+policy.BARRIER_DESTINATION+':ro',extra)
                own=service.removeprefix('cantos-')
                self.assertIn('Volume='+policy.SECURITY_ROOT+'/'+own+':'+policy.FLOOR_DESTINATION,extra)
                stripped=b''.join(line for line in result.splitlines(keepends=True) if line.decode().rstrip('\r\n') not in extra)
                self.assertEqual(stripped.replace(new.encode(),old.encode()),data)
                other='IDENTITY' if service=='cantos-server' else 'SERVER'
                self.assertNotIn(('CANTOS_'+other+'_RECOVERY_FENCE').encode(),result)

    def test_existing_recovery_config_malformed_operation_or_swapped_image_stays_unchanged(self):
        for service in policy.oci.SERVICES:
            data,old,new=self.fixture(service)
            for extra in [b'Environment=CANTOS_RECOVERY_OPERATION=x\n',b'Volume=/var/lib/cantos-security/server:/floor\n']:
                with self.assertRaises(ValueError): policy.target_unit(data+extra,service,old,new,'a'*32)
            for operation in ['0'*32,'A'*32,'a'*31,'a'*33]:
                with self.assertRaises(ValueError): policy.target_unit(data,service,old,new,operation)
            with self.assertRaises(ValueError): policy.target_unit(data,service,old,new.replace(service,'foreign'),'a'*32)

    def test_production_prestart_never_migrates_an_enrolled_database_without_its_floor(self):
        for service in policy.oci.SERVICES:
            data,old,new=self.fixture(service)
            uid=10001 if service=='cantos-server' else 10002
            if service=='cantos-server':
                command=(f'ExecStartPre=/usr/bin/podman run --rm --network none --user {uid}:{uid} '
                         '-e CANTOS_SERVER_DATABASE=/var/lib/cantos/db/server.db '
                         '-v /root/cantos-native/server-db:/var/lib/cantos/db:Z '
                         '--entrypoint /usr/local/bin/cantos-migrate '+old)
            else:
                command=(f'ExecStartPre=/usr/bin/podman run --rm --network none --user {uid}:{uid} '
                         '--env-file /root/cantos-native/config/identity.env '
                         '-v /root/cantos-native/identity-db:/var/lib/cantos/db:Z '+old+' migrate')
            data=data.replace(('ExecStartPre=/usr/bin/podman image exists '+old).encode(),command.encode())
            result=policy.target_unit(data,service,old,new,'a'*32)
            prestarts=[line for line in result.decode().splitlines() if line.startswith('ExecStartPre=')]
            self.assertEqual(prestarts,['ExecStartPre=/usr/bin/podman image exists '+new])
            self.assertNotIn(b'podman run',result)

    def test_unreviewed_prestart_is_refused_before_producing_target_bytes(self):
        data,old,new=self.fixture('cantos-server')
        altered=data.replace(b'podman image exists',b'podman run --privileged')
        with self.assertRaisesRegex(ValueError,'pre-start'):
            policy.target_unit(altered,'cantos-server',old,new,'a'*32)


if __name__=='__main__': unittest.main()
