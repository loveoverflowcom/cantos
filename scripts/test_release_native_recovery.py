"""Synthetic port model: protocol ordering/faults only; no engine/HTTP acceptance."""
import copy
import json
from pathlib import Path
import shutil
import tempfile
import unittest
import release_native_recovery as recovery
import release_oci as oci


class ModelPorts:
    def __init__(self, root):
        self.events, self.fault, self.floor = [], None, {}
        self.identity = {'source_sha':'a'*40, 'configuration_sha256':'b'*64,
            'images':{s:'ghcr.io/loveoverflowcom/'+s+'@sha256:'+'c'*64 for s in oci.SERVICES}}
        for index, service in enumerate(oci.SERVICES, 1):
            path=root/service; path.mkdir(mode=0o700)
            self.write(path, {'owner':service,'schema':36 if service=='cantos-server' else 6,
                'instance':str(index).zfill(32),'revision':2,'epoch':'e'*32,'operations':{},
                'disabled':True,'logged_out':True,'refresh_revoked':True,'exposure':'NOT_AUTHORIZED'})
            self.floor[service]=2
    def event(self, name):
        self.events.append(name)
        if self.fault==name:
            self.fault=None
            raise RuntimeError('injected-lost-reply')
    def release_identity(self): return copy.deepcopy(self.identity)
    def read(self,path): return json.loads((Path(path)/'state.json').read_text())
    def write(self,path,value): (Path(path)/'state.json').write_text(json.dumps(value,sort_keys=True))
    def public(self,value):
        keys=('owner','schema','instance','revision','exposure')
        result={key:value[key] for key in keys}
        if value['owner']=='cantos-identity': result['epoch']=value['epoch']
        else: result['all_prior_sessions_retired']=value.get('all_prior_sessions_retired',False)
        return result
    def observe(self,service,path):
        value=self.read(path)
        if value['owner']!=service or value['revision']!=self.floor[service]: raise ValueError('freshness-refused')
        self.event('observe-'+service)
        return self.public(value)
    def export(self,service,source,destination):
        self.observe(service,source)
        destination.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        shutil.copytree(source,destination); self.event('export-'+service)
    def copy(self,source,destination):
        destination.parent.mkdir(parents=True,exist_ok=True,mode=0o700)
        shutil.copytree(source,destination); self.event('copy-'+destination.name)
    def directory_record(self,path): return {'state.json':(Path(path)/'state.json').read_text()}
    def reconcile(self,service,path): return self.observe(service,path)
    def rotate(self,path,op,expected):
        value=self.read(path)
        if op in value['operations']:
            if value['operations'][op]!=expected: raise ValueError('operation-conflict')
        else:
            if value['epoch']!=expected: raise ValueError('epoch-conflict')
            value['operations'][op]=expected; value['epoch']='f'*32; value['revision']+=1
            self.floor['cantos-identity']=value['revision']; self.write(path,value)
        self.event('rotate'); return self.public(value)
    def retire(self,path):
        value=self.read(path)
        if not value.get('all_prior_sessions_retired'):
            value['all_prior_sessions_retired']=True; value['revision']+=1
            self.floor['cantos-server']=value['revision']; self.write(path,value)
        self.event('retire'); return self.public(value)


class RecoveryProtocol(unittest.TestCase):
    def fixture(self,directory):
        root=Path(directory).resolve(); state=root/'operator'; state.mkdir(mode=0o700)
        exposure=root/'exposure'; exposure.mkdir(mode=0o755); ports=ModelPorts(root)
        request={'schema_version':1,'operation':'d'*32,'barrier':str(exposure/'barrier'),
            **ports.release_identity(),'sources':{s:str(root/s) for s in oci.SERVICES}}
        return state,request,ports
    def closed(self,request): self.assertIn('\nblocked ',Path(request['barrier']).read_text())
    def test_complete_stays_closed_preserves_security_and_replays_without_effects(self):
        with tempfile.TemporaryDirectory() as directory:
            state,request,ports=self.fixture(directory)
            result=recovery.reconcile(state,request,ports); self.closed(request)
            self.assertEqual(result['exposure'],'BLOCKED')
            self.assertLess(ports.events.index('rotate'),ports.events.index('retire'))
            for path in result['staged'].values():
                value=ports.read(path)
                self.assertTrue(value['disabled'] and value['logged_out'] and value['refresh_revoked'])
            ports.events.clear(); self.assertEqual(recovery.reconcile(state,request,ports),result)
            self.assertNotIn('rotate',ports.events); self.assertNotIn('retire',ports.events)
    def test_lost_replies_resume_each_effect_without_duplicate_security_transition(self):
        for effect in ['export-cantos-server','export-cantos-identity','copy-cantos-server','copy-cantos-identity','rotate','retire']:
            with self.subTest(effect=effect), tempfile.TemporaryDirectory() as directory:
                state,request,ports=self.fixture(directory); ports.fault=effect
                with self.assertRaisesRegex(RuntimeError,'injected-lost-reply'): recovery.reconcile(state,request,ports)
                self.closed(request); result=recovery.reconcile(state,request,ports); self.closed(request)
                for value in result['owners'].values(): self.assertEqual(value['revision'],3)
    def test_stale_both_and_each_mixed_cut_are_refused(self):
        for stale in [oci.SERVICES,('cantos-server',),('cantos-identity',)]:
            with self.subTest(stale=stale), tempfile.TemporaryDirectory() as directory:
                state,request,ports=self.fixture(directory)
                for service in stale:
                    path=Path(request['sources'][service]); value=ports.read(path)
                    value.update(revision=1,disabled=False,logged_out=False); ports.write(path,value)
                with self.assertRaisesRegex(ValueError,'freshness-refused'): recovery.reconcile(state,request,ports)
                self.closed(request); self.assertNotIn('rotate',ports.events)
    def test_missing_corrupt_changed_request_or_release_never_reopens(self):
        for mode in ['missing','corrupt','request','images']:
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as directory:
                state,request,ports=self.fixture(directory)
                path=Path(request['sources']['cantos-identity'])/'state.json'
                if mode=='missing': path.unlink()
                elif mode=='corrupt': path.write_text('{')
                else:
                    ports.fault='rotate'
                    with self.assertRaises(RuntimeError): recovery.reconcile(state,request,ports)
                    if mode=='request': request['operation']='1'*32
                    else: ports.identity['source_sha']='2'*40
                with self.assertRaises((ValueError,FileNotFoundError,json.JSONDecodeError)): recovery.reconcile(state,request,ports)
                self.closed(request)


if __name__=='__main__': unittest.main()
