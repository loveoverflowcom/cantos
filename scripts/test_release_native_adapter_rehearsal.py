"""Fixture-only view normalization; no host, container or integration acceptance."""
import copy
from pathlib import Path
from types import SimpleNamespace
import unittest
import release_native_adapter_rehearsal as owner
import release_native as native

class FencedMounts(unittest.TestCase):
    def fixture(self):
        service='cantos-server'
        f=SimpleNamespace(fenced_images=lambda: True, fences={service:Path('/fixture/floors/server')},
                          exposure_dir=Path('/fixture/exposure'), recovery_operation='a'*32)
        mounts=[{'source':'/fixture/server-db','destination':'/database','read_only':False},
                {'source':'/fixture/socket','destination':'/identity','read_only':True},
                {'source':'/fixture/floors/server','destination':'/fence','read_only':False},
                {'source':'/fixture/exposure','destination':'/recovery','read_only':True}]
        request={'mounts':{service:mounts}}
        container={'Mounts':[{'Type':'bind','Source':m['source'],'Destination':m['destination'],'RW':not m['read_only']} for m in mounts],
                   'Config':{'Env':['CANTOS_SERVER_RECOVERY_FENCE=/fence/current',
                       'CANTOS_RECOVERY_BARRIER=/recovery/barrier','CANTOS_RECOVERY_OPERATION='+'a'*32]}}
        return f, request, service, container

    def test_exact_fixture_binds_are_checked_before_the_legacy_view_without_mutating_input(self):
        f,request,service,container=self.fixture()
        before=copy.deepcopy((request,container))
        base,view=owner.fenced_views(f,request,service,container)
        self.assertEqual([m['destination'] for m in base['mounts'][service]],['/database','/identity'])
        self.assertEqual([m['Destination'] for m in view['Mounts']],['/database','/identity'])
        self.assertEqual((request,container),before)
        layout={**request, 'databases':{service:'/fixture/server-db'},
                'socket_directory':'/fixture/socket','configuration_files':{}}
        with self.assertRaisesRegex(ValueError, 'unbound mount or foreign database'):
            native.validate_mount_plan(layout,service)
        base,view=owner.fenced_views(f,layout,service,container)
        self.assertEqual(native.validate_mount_plan(base,service),'/database/server.db')

    def test_missing_extra_wrong_owner_or_writable_shared_bind_never_normalizes(self):
        for mode in ('missing','extra','foreign','writable','planned_foreign','duplicate'):
            with self.subTest(mode=mode):
                f,request,service,c=self.fixture()
                if mode=='missing': c['Mounts'].pop()
                elif mode=='extra': c['Mounts'].append({'Type':'bind','Source':'/foreign','Destination':'/foreign','RW':False})
                elif mode=='foreign': c['Mounts'][2]['Source']='/fixture/floors/identity'
                elif mode=='writable': c['Mounts'][3]['RW']=True
                elif mode=='planned_foreign': request['mounts'][service][2]['source']='/foreign'
                else:
                    request['mounts'][service].append(copy.deepcopy(request['mounts'][service][2]))
                    c['Mounts'].append(copy.deepcopy(c['Mounts'][2]))
                with self.assertRaisesRegex(ValueError,'fixture security mount inventory mismatch'):
                    owner.fenced_views(f,request,service,c)

    def test_missing_duplicate_or_mismatched_security_environment_never_normalizes(self):
        for mode in ('missing','duplicate','wrong_operation','wrong_fence'):
            with self.subTest(mode=mode):
                f,request,service,c=self.fixture()
                env=c['Config']['Env']
                if mode=='missing': env.pop()
                elif mode=='duplicate': env.append(env[-1])
                elif mode=='wrong_operation': env[-1]='CANTOS_RECOVERY_OPERATION='+'b'*32
                else: env[0]='CANTOS_SERVER_RECOVERY_FENCE=/database/current'
                with self.assertRaisesRegex(ValueError,'fixture security environment mismatch'):
                    owner.fenced_views(f,request,service,c)

if __name__=='__main__': unittest.main()
