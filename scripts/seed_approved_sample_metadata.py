#!/usr/bin/env python3
"""Seed only the owner-approved #427 metadata through authenticated Studio HTTP.

Default is a read-only plan. Applying requires the existing owner's Studio cookie and an explicit
metadata rights/source confirmation; no account, credential, cover or audio is created. Every
mutation uses a deterministic operation ID, and conflicts or unknown HTTP outcomes stop the run.
"""
import argparse
import getpass
import hashlib
import json
import sys
import urllib.error
import urllib.parse
import urllib.request
import uuid
from pathlib import Path

DATASET = 'cantos-sample-catalog-v1'
SHA = 'd0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212'
WORKS = {
    'ben-suong': 'b4630614-a59d-4a5f-8d34-ae9433f01d2a',
    'nguoi-giu-den': '5de35d0f-ae81-43b5-aed6-f64057f95ef7',
    'thanh-pho-ngu-quen': '5a19cfec-a7d4-4bdd-bc35-dd27120a1885',
    'khu-vuon-tren-may': '278351ac-1a3f-4fab-81a8-ffac8dfbc122',
    'chuyen-tau-cuoi-cung': '56e6709f-c1fd-4e5f-abc5-6eeb88f7656c',
    'mua-gio-qua-nha': '8e0580bc-f44f-4443-bed8-36b9ea66154a',
}
API = 'https://api.cantos.cc'
STUDIO = 'https://studio.cantos.cc'
LIMIT = 2_097_152

class Refusal(Exception):
    pass

def identity(key, purpose):
    data = bytearray(hashlib.sha256(f'{DATASET}\0{key}\0{purpose}'.encode()).digest()[:16])
    data[6] = (data[6] & 15) | 64
    data[8] = (data[8] & 63) | 128
    return str(uuid.UUID(bytes=bytes(data)))

def operation(owner, purpose):
    return str(uuid.uuid5(uuid.NAMESPACE_URL, f'cantos/sample-metadata/v1/{owner}/{purpose}'))

def load_input(path):
    data = Path(path).read_bytes()
    if len(data) != 16102 or hashlib.sha256(data).hexdigest() != SHA:
        raise Refusal('input bytes do not match the approved sample')
    try:
        spec = json.loads(data)
        works = spec['works']
        if spec['dataset_id'] != DATASET or spec['story_count'] != 6 or spec['chapter_count'] != 12 or len(works) != 6:
            raise ValueError()
        found = set()
        for work in works:
            key = work['sample_key']
            if key in found or WORKS.get(key) != identity(key, 'work') or len(work['chapters']) != 2:
                raise ValueError()
            found.add(key)
            for index, chapter in enumerate(work['chapters']):
                if chapter['position'] != index or chapter['sample_key'] != f'{key}/chapter-{index + 1:02}':
                    raise ValueError()
                identity(chapter['sample_key'], 'chapter')
        if found != set(WORKS):
            raise ValueError()
    except (ValueError, KeyError, TypeError, json.JSONDecodeError) as exc:
        raise Refusal('input has an invalid sample mapping') from exc
    return data, works

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None

class Studio:
    def __init__(self, api, origin, cookie):
        self.api, self.origin, self.cookie = api, origin, cookie
        self.opener = urllib.request.build_opener(NoRedirect())
    def call(self, method, path, body=None, expected=200):
        payload = None if body is None else json.dumps(body, ensure_ascii=False, separators=(',', ':')).encode()
        headers = {'Accept':'application/json', 'Origin':self.origin}
        if path.startswith('/api/v1/studio/'):
            headers['Cookie'] = self.cookie
        if payload is not None:
            headers['Content-Type'] = 'application/json'
        request = urllib.request.Request(self.api + path, data=payload, method=method, headers=headers)
        try:
            with self.opener.open(request, timeout=30) as response:
                if response.status != expected or response.headers.get_content_type() != 'application/json':
                    raise Refusal(f'{method} {path}: unexpected response')
                raw = response.read(LIMIT + 1)
                if len(raw) > LIMIT:
                    raise Refusal(f'{method} {path}: response too large')
                return json.loads(raw)
        except urllib.error.HTTPError as exc:
            raise Refusal(f'{method} {path}: HTTP {exc.code}; operation stopped') from None
        except (urllib.error.URLError, TimeoutError, OSError, ValueError) as exc:
            raise Refusal(f'{method} {path}: outcome unknown; reconcile before retry') from None


def seed(client, owner, data, works, reference, statement):
    session = client.call('GET', '/api/v1/studio/session')
    if session.get('actor_id') != owner:
        raise Refusal('authenticated Studio actor does not match the requested owner')
    source = client.call('POST', '/api/v1/studio/imports', {
        'metadata': {'operation_id': operation(owner, 'source'), 'file_name':'sample-catalog.json',
            'format':'txt', 'reference':reference, 'rights_holder':None,
            'permission_evidence':reference, 'usage_scope':'Public metadata of the approved six-Work sample only'},
        'original_bytes':list(data),
    })
    source_id = source.get('id')
    if not isinstance(source_id, str) or source.get('sha256') != SHA:
        raise Refusal('source receipt does not match the approved bytes')
    rights = client.call('POST', '/api/v1/studio/sample-metadata-rights', {
        'operation_id': operation(owner, 'metadata-rights'), 'source_id':source_id,
        'evidence_reference':reference, 'rights_statement':statement,
        'confirm_metadata_rights':True, 'confirm_source':True,
    })
    rights_id = rights.get('rights_id')
    if rights.get('source_id') != source_id or not isinstance(rights_id, str):
        raise Refusal('rights receipt does not bind the imported source')
    for work in works:
        key = work['sample_key']
        work_id = WORKS[key]
        created = client.call('POST', '/api/v1/studio/works', {
            'operation_id':operation(owner, f'work/{key}'), 'id':work_id, 'kind':work['kind'],
            'details':{'title':work['title'], 'synopsis':work['description'], 'language':work['language'],
                'visibility':'public', 'rights_refs':[]},
        }, expected=201)
        receipt = created.get('work', {})
        if receipt.get('id') != work_id or receipt.get('owner_id') != owner or receipt.get('revision') != 1:
            raise Refusal('Work receipt differs from the requested owner or sample')
        for index, chapter in enumerate(work['chapters']):
            unit_id = identity(chapter['sample_key'], 'chapter')
            result = client.call('POST', f'/api/v1/studio/works/{work_id}/content-units', {
                'operation_id':operation(owner, f'unit/{chapter["sample_key"]}'), 'id':unit_id,
                'kind':'chapter', 'expected_work_revision':index + 1,
                'details':{'title':chapter['title'], 'description':chapter['description'],
                    'source':None, 'script':None, 'content_digest':None},
            }, expected=201)
            if result.get('work', {}).get('id') != work_id or result.get('work', {}).get('revision') != index + 2:
                raise Refusal('unit receipt differs from the requested sample')
        for unit in [None] + work['chapters']:
            unit_id = None if unit is None else identity(unit['sample_key'], 'chapter')
            path = f'/api/v1/studio/works/{work_id}' + ('' if unit_id is None else f'/content-units/{unit_id}') + '/sample-metadata-disclosures'
            subject = key if unit is None else unit['sample_key']
            decision = client.call('POST', path, {
                'decision':'disclose', 'operation_id':operation(owner, f'disclose/{subject}'),
                'expected_work_revision':3, 'expected_unit_revision':None if unit is None else 1,
                'source_id':source_id, 'rights_id':rights_id, 'confirm_metadata_rights':True, 'confirm_source':True,
            })
            if decision.get('work_id') != work_id or decision.get('subject_id') != (unit_id or work_id) or decision.get('decision') != 'disclosed':
                raise Refusal('disclosure receipt does not match its subject')
    # A historical replay is not proof that a later withdrawal was reversed. Read the live view.
    for work in works:
        work_id = WORKS[work['sample_key']]
        detail = client.call('GET', f'/api/v1/theatre/works/{work_id}?presentation=3')
        if (detail.get('work_id'), detail.get('title'), detail.get('description')) != (work_id, work['title'], work['description']):
            raise Refusal('current public Work differs from the approved sample')
        page = client.call('GET', f'/api/v1/theatre/works/{work_id}/units?limit=50&presentation=3')
        expected = {(identity(c['sample_key'], 'chapter'), c['title'], c['description'], c['position']) for c in work['chapters']}
        actual = {(c.get('unit_id'), c.get('title'), c.get('description'), c.get('position')) for c in page.get('items', [])}
        if (page.get('work_id') != work_id or page.get('next_cursor') is not None
                or len(page.get('items', [])) != 2 or actual != expected
                or any(c.get('kind') != 'chapter' or c.get('playback', 'missing') is not None for c in page.get('items', []))):
            raise Refusal('current public unit list differs from the approved sample')
    return {'works':len(works), 'units':sum(len(w['chapters']) for w in works), 'source_sha256':SHA}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--input', required=True, help='absolute path to the approved external JSON')
    parser.add_argument('--owner-id', required=True, help='existing authenticated Studio actor ID')
    parser.add_argument('--evidence-reference', required=True, help='owner-supplied metadata rights/source evidence reference')
    parser.add_argument('--rights-statement', required=True, help='owner statement about rights to display this sample metadata')
    parser.add_argument('--apply', action='store_true', help='apply through the authenticated production Studio API')
    parser.add_argument('--confirm-rights-and-source', action='store_true', help='confirm metadata rights and the exact source as the signed-in owner')
    args = parser.parse_args(argv)
    try:
        if not Path(args.input).is_absolute() or not args.owner_id.strip():
            raise Refusal('an absolute input path and existing owner ID are required')
        for value in [args.evidence_reference, args.rights_statement]:
            if not value.strip() or len(value.encode()) > 1024 or any(ord(c) < 32 or ord(c) == 127 for c in value):
                raise Refusal('rights statement and evidence reference must be nonblank, bounded single-line text')
        data, works = load_input(args.input)
        if not args.apply:
            print(json.dumps({'mode':'PLAN_ONLY', 'works':len(works), 'units':12, 'source_sha256':SHA}))
            return 0
        if not args.confirm_rights_and_source:
            raise Refusal('the signed-in owner must explicitly confirm metadata rights and source')
        cookie = getpass.getpass('Existing Studio Cookie header (not stored): ')
        if not cookie or '\n' in cookie or '\r' in cookie:
            raise Refusal('invalid Studio cookie header')
        print(json.dumps({'mode':'APPLIED_AND_READ_BACK', **seed(Studio(API, STUDIO, cookie), args.owner_id, data, works, args.evidence_reference, args.rights_statement)}))
        return 0
    except (Refusal, OSError) as exc:
        print(f'sample seed stopped: {exc}', file=sys.stderr)
        return 1

if __name__ == '__main__':
    sys.exit(main())
