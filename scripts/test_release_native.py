"""Pure refusals and isolated real-file recovery rehearsal with a named FakeHost.

No Podman/systemd/VPS or qualification PASS is synthesized by these tests.
"""
import copy
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_native as native
import release_oci as oci
from test_release_oci import fixture


def pair(digit):
    return {'source_sha': digit*40, 'configuration_sha256': digit*64, 'platform': 'linux/amd64',
            'compatibility': {'server': 'v24'}, 'images': {s: {'ref': f'ghcr.io/loveoverflowcom/{s}@sha256:{digit*64}',
                                                          'config_digest': 'sha256:'+('7' if digit == '1' else '8')*64}
                                                    for s in native.SERVICES}}


def request():
    return {'schema_version': 1, 'host_machine_sha256': 'a'*64, 'observed_at': int(time.time()),
            'host_state': 'NATIVE_READY', 'generation': 4, 'current_manifest_sha256': '1'*64,
            'previous': {'manifest_digest': '1'*64}, 'next': {'manifest_digest': '2'*64},
            'databases': {'cantos-server': '/var/lib/cantos/server', 'cantos-identity': '/var/lib/cantos/identity'},
            'socket_directory': '/run/cantos/identity', 'units': {s:'b'*64 for s in native.SERVICES},
            'configuration_files': {'/etc/cantos/native/env':'c'*64},
            'mounts': {s:[{'source':'/var/lib/cantos/'+('server' if s=='cantos-server' else 'identity'), 'destination':'/data','read_only':False},
                         {'source':'/run/cantos/identity','destination':'/rpc','read_only':False}] for s in native.SERVICES}, 'health_url': 'http://127.0.0.1:8080/health'}


class FakeHost:
    def __init__(self, fail=None, on_stop=None):
        self.events, self.fail, self.on_stop = [], fail, on_stop

    def event(self, name):
        self.events.append(name)
        if self.fail == name:
            raise ValueError('injected '+name)

    def observe(self, *_):
        self.event('observe')

    def recovery_host(self, *_):
        self.event('recovery-observe')

    def pull(self, release):
        self.event('pull-'+release['source_sha'][0])

    def stop(self, request):
        self.event('stop')
        if self.on_stop:
            self.on_stop()

    def start(self, request, release):
        self.event('start-'+release['source_sha'][0])


def rehash(args, manifest, admission):
    directory=args.manifest.parent
    evidence=json.loads((directory/'evidence.json').read_text())
    for gate in oci.GATES:
        path=directory/'gate-records'/(gate+'.json')
        evidence['gates'][gate]['record_sha256']=native.digest_bytes(path.read_bytes())
    (directory/'evidence.json').write_bytes(contract.canonical(evidence)+b'\n')
    manifest['evidence']=contract.file_record(directory/'evidence.json',directory)
    args.manifest.write_bytes(contract.canonical(manifest)+b'\n')
    args.manifest_digest=native.digest_bytes(args.manifest.read_bytes())
    admission['manifest_sha256']=args.manifest_digest
    args.admission.write_bytes(contract.canonical(admission)+b'\n')
    args.admission_digest=native.digest_bytes(args.admission.read_bytes())


def annotated_fixture(tmp):
    root,args,manifest,admission=fixture(Path(tmp))
    for gate in oci.GATES:
        path=args.manifest.parent/'gate-records'/(gate+'.json')
        record=json.loads(path.read_text())
        record.update(evidence_level='integration-tested',execution_environment='linux-native-exact-images')
        path.write_bytes(contract.canonical(record)+b'\n')
    rehash(args,manifest,admission)
    pins={key:str(getattr(args,key)) for key in ['manifest','manifest_digest','admission','admission_digest',
                                               'source_sha','configuration_digest','platform']}
    return root,args,manifest,admission,pins


class Decisions(unittest.TestCase):
    def test_valid_pair_and_build_config_change_is_not_runtime_config_change(self):
        self.assertEqual(native.decide(request(), pair('1'), pair('2'), time.time())['activation'], 'NOT_RUN')

    def test_no_unqualified_host_schema_platform_stale_or_wrong_generation(self):
        for change, message in [({'host_state':'LEGACY'}, 'native upgrade only'),
                                ({'host_state':'NEW'}, 'native upgrade only'),
                                ({'host_state':'UNKNOWN'}, 'native upgrade only'),
                                ({'observed_at':int(time.time())-3601}, 'fresh request'),
                                ({'generation':True}, 'generation'),
                                ({'current_manifest_sha256':'3'*64}, 'current release'),
                                ({'health_url':'https://example.com/'}, 'loopback')]:
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, message):
                native.decide(request() | change, pair('1'), pair('2'), time.time())
        for change, message in [({'compatibility':{'server':'v25'}}, 'schema/RPC'), ({'platform':'linux/arm64'}, 'platform')]:
            with self.assertRaisesRegex(ValueError, message):
                native.decide(request(), pair('1'), pair('2') | change, time.time())

    def test_paths_cannot_overlap_escape_roots_or_choose_remote_health(self):
        for paths in [('server','server'), ('server','server/child')]:
            r = request(); r['databases'] = dict(zip(native.SERVICES, ['/var/lib/cantos/'+p for p in paths]))
            with self.assertRaisesRegex(ValueError, 'overlap|differ'):
                native.decide(r, pair('1'), pair('2'), time.time())
        r=request();r['configuration_files']={'/etc/passwd':'c'*64}
        with self.assertRaisesRegex(ValueError,'Cantos configuration'):
            native.decide(r,pair('1'),pair('2'),time.time())

    def test_image_replacement_preserves_every_other_byte_and_refuses_duplicates(self):
        text=b'[Unit]\nDescription=existing\n[Container]\nImage=old\nVolume=/data:/data\n# Image=old\nEnvironment=LABEL=Image=old\n'
        self.assertEqual(native.unit_image(text,'old','new'),text.replace(b'\nImage=old\n',b'\nImage=new\n'))
        for data in [text+b'Image=old\n',text.replace(b'Image=old',b'Image=foreign'),b'[Unit]\nImage=old\n']:
            with self.assertRaises(ValueError):native.unit_image(data,'old','new')

    def test_container_database_env_and_full_mount_inventory_are_bound(self):
        r=request();service='cantos-server'
        c={'ImageName':pair('1')['images'][service]['ref'],'Image':pair('1')['images'][service]['config_digest'],
           'Path':'/usr/local/bin/cantos-server','Args':[], 'Config':{'User':'10001:10001','Env':['CANTOS_SERVER_DATABASE=/data/server.db','CANTOS_IDENTITY_SOCKET=/rpc/identity.sock',
                                                   'CANTOS_IDENTITY_PEER_UID=10002','CANTOS_IDENTITY_PEER_POLICY=direct_peer']},
           'Mounts':[{'Type':'bind','Source':m['source'],'Destination':m['destination'],'RW':not m['read_only']} for m in r['mounts'][service]]}
        native.check_container(r,service,c,[pair('1')['images'][service]])
        for change in ['destination','named-volume','db-env','readonly']:
            altered=copy.deepcopy(c)
            if change=='destination':altered['Mounts'][0]['Destination']='/unused'
            if change=='named-volume':altered['Mounts'].append({'Type':'volume','Source':'data','Destination':'/real','RW':True})
            if change=='db-env':altered['Config']['Env']=['CANTOS_SERVER_DATABASE=/real/server.db']
            if change=='readonly':altered['Mounts'][0]['RW']=False
            with self.subTest(change=change),self.assertRaisesRegex(ValueError,'mount|database'):
                native.check_container(r,service,altered,[pair('1')['images'][service]])

    def test_native_release_requires_scoped_real_image_claims_not_models_or_mocks(self):
        # These are synthetic record-validation fixtures, never actual qualified images.
        for field,value in [('evidence_level','model-checked'),('evidence_level',None),
                            ('execution_environment','fake-host'),('execution_environment',None)]:
            with self.subTest(field=field,value=value),tempfile.TemporaryDirectory() as tmp:
                root,args,manifest,admission,pins=annotated_fixture(tmp)
                native.release(root,pins)
                path=args.manifest.parent/'gate-records'/'native-e2e.json'
                record=json.loads(path.read_text());record[field]=value;path.write_bytes(contract.canonical(record))
                rehash(args,manifest,admission)
                pins.update(manifest_digest=args.manifest_digest,admission_digest=args.admission_digest)
                with self.assertRaisesRegex(ValueError,'exact-image execution evidence'):
                    native.release(root,pins)

    def test_native_release_refuses_not_run_and_rehashed_source_schema_drift(self):
        for kind in ['NOT_RUN','schema-drift']:
            with self.subTest(kind=kind),tempfile.TemporaryDirectory() as tmp:
                root,args,manifest,admission,pins=annotated_fixture(tmp)
                if kind=='NOT_RUN':
                    path=args.manifest.parent/'gate-records'/'image-pair.json'
                    record=json.loads(path.read_text());record['status']='NOT_RUN';path.write_bytes(contract.canonical(record))
                else:manifest['compatibility']['server_schema']='server-turso-epoch-1-v24'
                rehash(args,manifest,admission)
                pins.update(manifest_digest=args.manifest_digest,admission_digest=args.admission_digest)
                with self.assertRaisesRegex(ValueError,'qualification record|actual source registry'):
                    native.release(root,pins)

    def test_environment_directive_whitespace_is_parsed_and_unsafe_forms_refused(self):
        for line in [' EnvironmentFile=/etc/cantos/native.env','EnvironmentFile = /etc/cantos/native.env']:
            self.assertEqual(native.environment_files(('[Container]\n'+line+'\n').encode()),['/etc/cantos/native.env'])
        for line in ['EnvironmentFile=-/etc/cantos/native.env','EnvironmentFile=/etc/cantos/%n.env',
                     'EnvironmentFile=/etc/passwd','EnvironmentFile=']:
            with self.assertRaisesRegex(ValueError,'environment file'):native.environment_files((line+'\n').encode())

    def test_effective_env_output_requires_full_consumption(self):
        bound={'/etc/cantos/a.env':'a'*64}
        self.assertEqual(native.effective_environment_files('/etc/cantos/a.env (ignore_errors=no)',bound),list(bound))
        for value in ['/etc/cantos/unbound.env','garbage','prefix /etc/cantos/a.env (ignore_errors=no)',
                      '/etc/cantos/a.env (ignore_errors=no) trailing','/etc/cantos/b.env (ignore_errors=no)']:
            with self.subTest(value=value),self.assertRaises(ValueError):native.effective_environment_files(value,bound)

    def test_podman_is_always_local_and_receives_no_provider_environment(self):
        with patch.object(native.subprocess,'run',return_value=SimpleNamespace(returncode=0,stdout=b'{}')) as run:
            native.System().command(['podman','info','--format','json'])
        self.assertEqual(run.call_args.args[0],['podman','--remote=false','info','--format','json'])
        self.assertEqual(set(run.call_args.kwargs['env']),{'PATH','HOME','LANG'})

    def test_every_qualification_record_is_required_and_exact_pair_bound(self):
        # Existing admission owner on real Git/tar files: each missing/NOT_RUN/foreign gate refuses.
        for gate in oci.GATES:
            with self.subTest(gate=gate), tempfile.TemporaryDirectory() as tmp:
                root,args,manifest,_=fixture(Path(tmp))
                path=args.manifest.parent/'gate-records'/f'{gate}.json'
                record=json.loads(path.read_text());record['images']=pair('2')['images'];path.write_bytes(contract.canonical(record))
                # A hash mismatch cannot be relabelled as acceptance.
                with self.assertRaises(ValueError):oci.verify(root,args)
        with tempfile.TemporaryDirectory() as tmp:
            root,args,manifest,_=fixture(Path(tmp))
            (args.manifest.parent/'gate-records'/'image-pair.json').unlink()
            with self.assertRaises((ValueError,OSError)):oci.verify(root,args)


class FilesAndRecovery(unittest.TestCase):
    def setUp(self):
        self.tmp=tempfile.TemporaryDirectory();self.addCleanup(self.tmp.cleanup)
        self.root=Path(self.tmp.name).resolve();self.state=self.root/'state';self.state.mkdir(mode=0o700)
        self.units=self.root/'units';self.units.mkdir();self.patch=patch.object(native,'UNITS',self.units);self.patch.start();self.addCleanup(self.patch.stop)
        self.old,self.new=pair('1'),pair('2');self.request=request()
        configs=self.root/'env';configs.write_bytes(b'mail=disabled\n');configs.chmod(0o600)
        self.request['configuration_files']={str(configs):native.digest_bytes(configs.read_bytes())}
        self.request['databases']={}
        for s in native.SERVICES:
            db=self.root/s;db.mkdir(mode=0o700);self.request['databases'][s]=str(db)
            for name in ['store.db','store.db-wal','store.db-shm']:
                p=db/name;p.write_bytes((s+name).encode());p.chmod(0o600)
            unit=self.units/(s+'.container');unit.write_text('[Container]\nImage='+self.old['images'][s]['ref']+'\n');unit.chmod(0o600)
            self.request['units'][s]=native.digest_bytes(unit.read_bytes())
        native.atomic(self.state/'current.json',contract.canonical({'generation':4,'manifest_sha256':'1'*64,'images':self.old['images']}))

    def upgrade(self, host=None):
        return native.upgrade(self.request,self.old,self.new,'2'*64,host or FakeHost(),self.state)

    def attempt(self):
        return next(self.state.glob('attempt-*'))

    def test_cold_backup_includes_sidecars_hashes_and_modes_before_swap(self):
        host=FakeHost();result=self.upgrade(host)
        self.assertEqual(host.events,['observe','pull-2','stop','start-2'])
        self.assertEqual((self.units/'cantos-server.container').stat().st_mode & 0o777,0o600)
        self.assertEqual(result['operation'],'ACTIVATED');self.assertFalse((self.state/'in-progress.json').exists())
        for s in native.SERVICES:
            self.assertEqual(native.tree_inventory(Path(self.request['databases'][s])),native.tree_inventory(self.attempt()/(s+'-database')))
        self.assertEqual(json.loads((self.state/'current.json').read_text())['generation'],5)

    def crash(self, stage):
        with self.assertRaisesRegex(ValueError,'injected'):self.upgrade(FakeHost(fail=stage))
        self.assertTrue((self.state/'in-progress.json').exists())
        with self.assertRaisesRegex(ValueError,'reconcile'):self.upgrade()
        self.assertEqual(json.loads((self.state/'current.json').read_text())['generation'],4)

    def test_failed_pull_blocks_replay(self):self.crash('pull-2')
    def test_stop_failure_blocks_replay(self):self.crash('stop')
    def test_start_failure_blocks_replay(self):self.crash('start-2')

    def test_unit_drift_after_stop_refuses_before_replacement_and_retains_backup(self):
        p=self.units/'cantos-server.container'
        with self.assertRaisesRegex(ValueError,'unit drift'):
            self.upgrade(FakeHost(on_stop=lambda:p.write_bytes(p.read_bytes()+b'# foreign\n')))
        self.assertTrue((self.attempt()/'backup.json').exists())
        self.assertTrue((self.state/'in-progress.json').exists())

    def test_generation_and_existing_marker_refuse_before_host_effects(self):
        self.request['generation']=3;host=FakeHost()
        with self.assertRaisesRegex(ValueError,'generation'):self.upgrade(host)
        self.assertEqual(host.events,[])

    def test_explicit_rollback_keeps_post_activation_data_and_monotonic_generation(self):
        self.upgrade();attempt=self.attempt()
        data=Path(self.request['databases']['cantos-server'])/'store.db';data.write_bytes(b'new legitimate rows')
        result=native.rollback(self.request,self.old,self.new,attempt.name,FakeHost(),self.state)
        self.assertEqual((self.units/'cantos-server.container').stat().st_mode & 0o777,0o600)
        self.assertEqual(result['operation'],'ROLLED_BACK');self.assertEqual(data.read_bytes(),b'new legitimate rows')
        self.assertEqual(json.loads((self.state/'current.json').read_text())['generation'],6)
        with self.assertRaisesRegex(ValueError,'already rolled back'):
            native.rollback(self.request,self.old,self.new,attempt.name,FakeHost(),self.state)

    def test_foreign_process_retaining_sidecar_refuses(self):
        proc=self.root/'proc';fd=proc/'123'/'fd';fd.mkdir(parents=True)
        (fd/'4').symlink_to(Path(self.request['databases']['cantos-server'])/'store.db-wal')
        with self.assertRaisesRegex(ValueError,'owner still holds'):
            native.no_open_database_files(self.request['databases'].values(),proc)

    def test_rollback_rechecks_config_after_stop(self):
        self.upgrade();attempt=self.attempt()
        config=Path(next(iter(self.request['configuration_files'])))
        with self.assertRaisesRegex(ValueError,'recovery config'):
            native.rollback(self.request,self.old,self.new,attempt.name,FakeHost(on_stop=lambda:config.write_bytes(b'drift')),self.state)
        self.assertTrue((self.state/'in-progress.json').exists())
        self.assertEqual(json.loads((self.state/'current.json').read_text())['generation'],5)

    def test_rollback_rechecks_units_after_pull(self):
        self.upgrade();attempt=self.attempt();unit=self.units/'cantos-server.container'
        class DriftingHost(FakeHost):
            def pull(host,release):
                super().pull(release);unit.write_bytes(unit.read_bytes()+b'# drift\n')
        with self.assertRaisesRegex(ValueError,'recovery unit drift'):
            native.rollback(self.request,self.old,self.new,attempt.name,DriftingHost(),self.state)
        self.assertTrue((self.state/'in-progress.json').exists())

    def test_open_database_inode_cannot_hide_behind_another_namespace_path(self):
        source=Path(self.request['databases']['cantos-server'])/'store.db'
        alias=self.root/'outside-alias';os.link(source,alias)
        proc=self.root/'proc';fd=proc/'123'/'fd';fd.mkdir(parents=True);(fd/'4').symlink_to(alias)
        with self.assertRaisesRegex(ValueError,'owner still holds'):
            native.no_open_database_files(self.request['databases'].values(),proc)

    def test_database_hashing_is_streamed_and_still_checks_actual_bytes(self):
        source=Path(self.request['databases']['cantos-server'])
        with patch.object(Path,'read_bytes',side_effect=AssertionError('unbounded database read')):
            inventory=native.tree_inventory(source)
        expected=hashlib.sha256(b'cantos-serverstore.db').hexdigest()
        self.assertEqual(inventory['store.db']['sha256'],expected)

    def test_backup_refuses_symlink_and_hardlink(self):
        source=Path(self.request['databases']['cantos-server']);target=source/'escape';target.symlink_to('/etc/passwd')
        with self.assertRaisesRegex(ValueError,'foreign or linked'):native.cold_copy(source,self.root/'copy')
        target.unlink();os.link(source/'store.db',source/'linked')
        with self.assertRaisesRegex(ValueError,'foreign or linked'):native.tree_inventory(source)

    def test_lock_contends_and_marker_survives_failed_backup(self):
        with native.locked(self.state),self.assertRaises(BlockingIOError):
            with native.locked(self.state):pass
        with patch.object(native,'cold_copy',side_effect=OSError('disk full')):
            with self.assertRaises(OSError):self.upgrade()
        self.assertTrue((self.state/'in-progress.json').exists())
        self.assertEqual(json.loads((self.state/'current.json').read_text())['generation'],4)


if __name__=='__main__':unittest.main()
