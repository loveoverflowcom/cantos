"""Local doubles for retry and scope; no production HTTP is contacted."""
import importlib.util
import json
import os
import pathlib
import unittest

path = pathlib.Path(__file__).with_name('seed_approved_sample_metadata.py')
spec = importlib.util.spec_from_file_location('sample_seed', path)
seed = importlib.util.module_from_spec(spec)
spec.loader.exec_module(seed)

class FakeStudio:
    def __init__(self, actor, works):
        self.actor, self.works = actor, works
        self.operations, self.calls = {}, []
        self.withdrawn = False
    def call(self, method, path, body=None, expected=200):
        self.calls.append((method, path))
        if path == '/api/v1/studio/session': return {'actor_id': self.actor}
        if body and ('operation_id' in body or path == '/api/v1/studio/imports'):
            op = body['metadata']['operation_id'] if path == '/api/v1/studio/imports' else body['operation_id']
            serialized = json.dumps(body, sort_keys=True)
            if op in self.operations and self.operations[op] != serialized:
                raise seed.Refusal('operation reused')
            self.operations[op] = serialized
        if path == '/api/v1/studio/imports': return {'id':'src_fixture', 'sha256':seed.SHA}
        if path == '/api/v1/studio/sample-metadata-rights': return {'rights_id':'rights_fixture','source_id':body['source_id']}
        if path == '/api/v1/studio/works': return {'work':{'id':body['id'], 'owner_id':self.actor, 'revision':1}}
        if path.endswith('/content-units'):
            return {'work':{'id':path.split('/')[5], 'revision':body['expected_work_revision'] + 1}}
        if path.endswith('/sample-metadata-disclosures'):
            parts = path.split('/')
            return {'work_id':parts[5], 'subject_id':parts[7] if len(parts)>8 else parts[5], 'decision':'disclosed'}
        work_id = path.split('/')[5].split('?')[0]
        work = next(w for w in self.works if seed.WORKS[w['sample_key']] == work_id)
        if self.withdrawn: raise seed.Refusal('withdrawn')
        if '/units?' not in path:
            return {'work_id':work_id, 'title':work['title'], 'description':work['description']}
        return {'work_id':work_id, 'next_cursor':None, 'items':[
            {'unit_id':seed.identity(c['sample_key'],'chapter'), 'title':c['title'], 'description':c['description'], 'position':c['position'], 'kind':'chapter', 'playback':None}
            for c in work['chapters']]}

class SeedTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = os.environ.get('CANTOS_APPROVED_SAMPLE_INPUT')
        if not path:
            raise unittest.SkipTest('requires separately supplied approved sample input')
        cls.data, cls.works = seed.load_input(path)

    def test_replay_uses_same_operations_and_rechecks_current_public_view(self):
        fake = FakeStudio('owner-fixture', self.works)
        result = seed.seed(fake, 'owner-fixture', self.data, self.works, 'fixture', 'fixture assertion')
        first = dict(fake.operations)
        self.assertEqual((result['works'], result['units']), (6, 12))
        self.assertEqual(result, seed.seed(fake, 'owner-fixture', self.data, self.works, 'fixture', 'fixture assertion'))
        self.assertEqual(first, fake.operations)
        self.assertTrue(all('/publications/' not in path and '/media/' not in path for _, path in fake.calls))
        fake.withdrawn = True
        with self.assertRaises(seed.Refusal):
            seed.seed(fake, 'owner-fixture', self.data, self.works, 'fixture', 'fixture assertion')

    def test_wrong_owner_and_changed_attestation_stop(self):
        fake = FakeStudio('another-owner', self.works)
        with self.assertRaises(seed.Refusal):
            seed.seed(fake, 'owner-fixture', self.data, self.works, 'fixture', 'fixture assertion')
        self.assertEqual(fake.calls, [('GET','/api/v1/studio/session')])
        fake = FakeStudio('owner-fixture', self.works)
        seed.seed(fake, 'owner-fixture', self.data, self.works, 'fixture', 'fixture assertion')
        with self.assertRaises(seed.Refusal):
            seed.seed(fake, 'owner-fixture', self.data, self.works, 'fixture', 'different assertion')

if __name__ == '__main__': unittest.main()
