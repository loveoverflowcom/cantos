import json
import importlib.util
import subprocess
import tempfile
import re
import unittest
from pathlib import Path
from reference_protobuf import (
    ROOT, FIELDS, DIRECTORY, ARTWORK_FIELDS, ARTWORK_DIRECTORY, ENUMS,
    ArtworkWireError, decode_artwork, encode, varint,
)


def schema_messages(schema):
    """Read balanced message bodies, including the reviewed nested oneof block."""
    result = {}
    for match in re.finditer(r'message\s+(\w+)\s*\{', schema):
        start = match.end()
        depth = 1
        cursor = start
        while depth:
            depth += (schema[cursor] == '{') - (schema[cursor] == '}')
            cursor += 1
        result[match.group(1)] = schema[start:cursor - 1]
    return result


class ProtobufReferenceTests(unittest.TestCase):
    def test_hand_reviewed_scalar_and_presence_bytes(self):
        self.assertEqual(varint(0), b'\x00')
        self.assertEqual(varint(128), b'\x80\x01')
        self.assertEqual(varint((1 << 64)-1), b'\xff'*9+b'\x01')
        self.assertEqual(encode('ValidateScriptRequest', {'script_json':''}), b'\x0a\x00')
        self.assertEqual(encode('ValidateScriptRequest', {}), b'')
        self.assertEqual(encode('ValidateScriptRequest', {'script_json':None}), b'')
        self.assertEqual(encode('ScriptValidationResponse', {'issues':[]}), b'')
        for value in [-1, 1 << 64, True]:
            with self.assertRaises(ValueError): varint(value)

    def test_schema_field_numbers_match_reviewed_oracle(self):
        schema=(ROOT/'packages/contracts/protobuf/studio_v1.proto').read_text()
        messages=re.findall(r'message\s+(\w+)\s*\{(.*?)\}',schema,re.S)
        self.assertEqual({name for name,_ in messages},set(FIELDS))
        for name,body in messages:
            actual=[]
            for label,kind,field,number in re.findall(r'(optional|repeated)\s+(\w+)\s+(\w+)\s*=\s*(\d+)\s*;',body):
                actual.append((int(number),field,'repeated:'+kind if label=='repeated' else kind))
            self.assertEqual(actual,FIELDS[name])
            self.assertIn('reserved ',body)

    def test_artwork_schema_matches_independently_reviewed_fields_enums_and_oneof(self):
        schema = (ROOT/'packages/contracts/protobuf/artwork_v1.proto').read_text()
        self.assertIn('syntax = "proto2";', schema)
        self.assertIn('package cantos.studio.artwork.v1;', schema)
        messages = schema_messages(schema)
        self.assertEqual(set(messages), set(ARTWORK_FIELDS))
        for name, body in messages.items():
            actual = []
            for label, kind, field, number in re.findall(r'(?:(optional|repeated)\s+)?(\w+)\s+(\w+)\s*=\s*(\d+)\s*;', body):
                actual.append((int(number), field, 'repeated:'+kind if label == 'repeated' else kind))
                if not label:
                    self.assertEqual(name, 'SetWorkArtworkRequest')
                    self.assertIn(field, ('artwork', 'clear_artwork'))
            self.assertEqual(actual, ARTWORK_FIELDS[name], name)
            self.assertTrue(all(number <= 15 for number, _, _ in actual))
        self.assertRegex(messages['SetWorkArtworkRequest'], r'oneof\s+change\s*\{')
        for name, values in ENUMS.items():
            body = re.search(r'enum\s+'+name+r'\s*\{(.*?)\}', schema, re.S).group(1)
            numbers = [int(number) for number in re.findall(r'\w+\s*=\s*(\d+)\s*;', body)]
            self.assertEqual(numbers, [0] + list(values.values()), name)

    def test_external_schemas_are_registered_for_warm_cargo_regeneration(self):
        build = (ROOT/'scripts/protobuf-oracle/rust/build.rs').read_text()
        for name in ('studio_v1.proto', 'artwork_v1.proto'):
            self.assertIn(f'println!("cargo:rerun-if-changed=../../../packages/contracts/protobuf/{name}");', build)
        main = (ROOT/'scripts/protobuf-oracle/rust/src/main.rs').read_text()
        self.assertIn('generated_artwork.rs', main)
        self.assertIn('cantos.studio.artwork.v1.rs', main)

    def test_every_reviewed_pilot_golden_matches_independent_wire_spec(self):
        corpus=json.loads((DIRECTORY/'goldens.json').read_text())
        self.assertEqual(corpus['contract'],'cantos.studio.wire.v1')
        self.assertEqual(len(corpus['cases']),13)
        for case in corpus['cases']:
            self.assertEqual(encode(case['message'],case['value']).hex(),case['protobuf_hex'],case['name'])

    def test_hand_reviewed_artwork_enum_presence_uint32_and_oneof_bytes(self):
        self.assertEqual(encode('ImageDeclarations', {'provenance':'', 'rights_refs':[]}).hex(), '0a00')
        self.assertEqual(encode('ClearArtwork', {}).hex(), '')
        self.assertEqual(encode('SetWorkArtworkRequest', {'operation_id':'', 'expected_work_revision':1, 'artwork':None}).hex(), '0a0010012200')
        self.assertEqual(encode('SetWorkArtworkRequest', {'operation_id':'', 'expected_work_revision':1}).hex(), '0a001001')
        self.assertEqual(encode('ImageVariantMetadata', {'content_type':'image/jpeg', 'width_px':(1<<32)-1}).hex(), '180220ffffffff0f')
        self.assertEqual(encode('ImageIntentResponse', {'state':'expired'}).hex(), '1803')
        self.assertEqual(encode('WorkArtworkResponse', {'changed_at_ms':0}).hex(), '2000')
        self.assertEqual(encode('WorkArtworkResponse', {'changed_at_ms':None}).hex(), '')
        with self.assertRaisesRegex(ValueError, 'uint32'):
            encode('ImageVariantMetadata', {'width_px':1<<32})

    def test_artwork_goldens_agree_in_both_directions_without_normalizing_unicode(self):
        corpus = json.loads((ARTWORK_DIRECTORY/'goldens.json').read_text())
        self.assertEqual(corpus['contract'], 'cantos.studio.artwork.v1')
        self.assertEqual({case['message'] for case in corpus['cases']}, set(ARTWORK_FIELDS))
        for case in corpus['cases']:
            with self.subTest(case=case['name']):
                wire = encode(case['message'], case.get('wire_value', case['value']))
                self.assertEqual(wire.hex(), case['protobuf_hex'])
                if case['application_valid']:
                    expected = dict(case['value'])
                    if case['message'] == 'WorkArtworkResponse':
                        expected.setdefault('artwork', None)
                        expected.setdefault('changed_at_ms', None)
                    self.assertEqual(decode_artwork(case['message'], wire), expected)
                else:
                    with self.assertRaisesRegex(ArtworkWireError, '^Shape$'):
                        decode_artwork(case['message'], wire)
        self.assertIn('Ngày', corpus['cases'][0]['value']['provenance'])

    def test_hostile_artwork_corpus_agrees_on_exact_independent_categories(self):
        corpus = json.loads((ARTWORK_DIRECTORY/'hostile.json').read_text())
        self.assertEqual(corpus['contract'], 'cantos.studio.artwork.v1')
        for case in corpus['cases']:
            with self.subTest(case=case['name']):
                data = bytes.fromhex(case['protobuf_hex'])
                if case['error'] is None:
                    decode_artwork(case['message'], data)
                else:
                    with self.assertRaisesRegex(ArtworkWireError, '^'+case['error']+'$'):
                        decode_artwork(case['message'], data)

    def test_wire_admission_finishes_before_nested_dto_presence(self):
        with self.assertRaisesRegex(ArtworkWireError, '^Shape$'):
            decode_artwork('SetWorkArtworkRequest', bytes.fromhex('0a0010011a00'))
        with self.assertRaisesRegex(ArtworkWireError, '^Malformed$'):
            decode_artwork('SetWorkArtworkRequest', bytes.fromhex('0a0010011a0000'))
        with self.assertRaisesRegex(ArtworkWireError, '^Shape$'):
            decode_artwork('CreateImageIntentRequest', bytes.fromhex('0a0012001a00200128013200'))
        with self.assertRaisesRegex(ArtworkWireError, '^Malformed$'):
            decode_artwork('CreateImageIntentRequest', bytes.fromhex('0a0012001a0020012801320000'))

    def test_kotlin_fixture_generation_is_test_only_and_uses_reviewed_values(self):
        generator = ROOT/'scripts/protobuf-oracle/generate_kotlin_vectors.py'
        spec = importlib.util.spec_from_file_location('kotlin_oracle_vectors', generator)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        self.assertEqual(module.initializer('SetWorkArtworkRequest', {'operation_id':'', 'expected_work_revision':1, 'artwork':None}), 'SetWorkArtworkRequest("", 1uL.toLong(), ArtworkChange.Clear(ClearArtwork))')
        self.assertIn('ImageMime.Jpeg', module.initializer('ImageVariantMetadata', {'content_type':'image/jpeg'}))
        embed = ROOT/'apps/cantos-theatre/mobile/tooling/embed_artwork_fixtures.py'
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)/'CanonicalArtworkVectors.kt'
            subprocess.run(['python3', str(embed), str(ARTWORK_DIRECTORY/'goldens.json'), str(ARTWORK_DIRECTORY/'hostile.json'), str(output)], check=True)
            content = output.read_text(encoding='utf-8')
            self.assertIn('package com.cantos.theatre.artworkfixtures', content)
            self.assertIn('@ProtoOneOf val change', content)
            self.assertIn('ArtworkChange.Clear(ClearArtwork)', content)
            self.assertIn('listOf(', content)
            self.assertIn('.joinToString("")', content)
            self.assertEqual(content.count('        ArtworkGolden('), 38)
            self.assertEqual(content.count('        ArtworkHostile('), 42)
        build = (ROOT/'apps/cantos-theatre/mobile/shared/build.gradle.kts').read_text()
        common_main = build.split('        commonMain {', 1)[1].split('        commonTest {', 1)[0]
        common_test = build.split('        commonTest {', 1)[1].split('        val desktopMain', 1)[0]
        self.assertNotIn('generateArtworkVectors', common_main)
        self.assertIn('kotlin.srcDir(generateArtworkVectors)', common_test)

    def test_studio_pilot_codec_is_fixture_only_and_never_ships_in_common_main(self):
        mobile = ROOT/'apps/cantos-theatre/mobile'
        shared = mobile/'shared/src'
        fixtures = shared/'commonTest/kotlin/com/cantos/theatre/studiopilotfixtures'
        for name in ['StudioPilotCodec.kt', 'StudioPilotWire.kt', 'StudioPilotValues.kt',
                     'StrictProtobufEnvelope.kt', 'StudioPilotCodecTest.kt']:
            self.assertTrue((fixtures/name).is_file(), name)
        for source in sorted(shared.glob('*Main/**/*.kt')):
            content = source.read_text(encoding='utf-8')
            self.assertNotIn('kotlinx.serialization.protobuf', content, source)
            self.assertNotIn('StudioPilot', content, source)
        build = (mobile/'shared/build.gradle.kts').read_text()
        common_main = build.split('        commonMain {', 1)[1].split('        commonTest {', 1)[0]
        common_test = build.split('        commonTest {', 1)[1].split('        val desktopMain', 1)[0]
        self.assertNotIn('kotlinx-serialization-protobuf', common_main)
        self.assertIn('kotlinx-serialization-protobuf', common_test)
        self.assertIn('kotlin.srcDir(generateProtobufVectors)', common_test)
        embed = mobile/'tooling/embed_protobuf_fixtures.py'
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)/'CanonicalProtobufVectors.kt'
            subprocess.run(['python3', str(embed), str(DIRECTORY/'goldens.json'), str(DIRECTORY/'hostile.json'), str(output)], check=True)
            self.assertIn('package com.cantos.theatre.studiopilotfixtures', output.read_text(encoding='utf-8'))

    def test_depth_field_and_envelope_budgets_are_independent_of_schema_shape(self):
        cases = json.loads((ARTWORK_DIRECTORY/'goldens.json').read_text())['cases']
        deepest = next(case for case in cases if case['name']=='intent-admitted-with-asset')
        data = bytes.fromhex(deepest['protobuf_hex'])
        decode_artwork(deepest['message'], data, max_depth=4)
        with self.assertRaisesRegex(ArtworkWireError, '^TooDeep$'):
            decode_artwork(deepest['message'], data, max_depth=3)
        # Every budget uses the same legal input, so Shape never masks the bound.
        with self.assertRaisesRegex(ArtworkWireError, '^TooLarge$'):
            decode_artwork(deepest['message'], data, max_fields=1)
        with self.assertRaisesRegex(ArtworkWireError, '^TooLarge$'):
            decode_artwork(deepest['message'], data, max_bytes=len(data)-1)
        exact = encode('ImageDeclarations', {'provenance':'', 'rights_refs':[''] * 255})
        decode_artwork('ImageDeclarations', exact, max_rights=256)
        with self.assertRaisesRegex(ArtworkWireError, '^TooLarge$'):
            decode_artwork('ImageDeclarations', exact + bytes.fromhex('1200'), max_rights=256)
        envelope = encode('ImageDeclarations', {'provenance':'x' * 32764, 'rights_refs':[]})
        self.assertEqual(len(envelope), 32768)
        decode_artwork('ImageDeclarations', envelope)
        with self.assertRaisesRegex(ArtworkWireError, '^TooLarge$'):
            decode_artwork('ImageDeclarations', envelope + bytes.fromhex('1200'))
        decode_artwork('ImageDeclarations', bytes.fromhex('0a00'), max_fields=1)
        with self.assertRaisesRegex(ArtworkWireError, '^TooLarge$'):
            decode_artwork('ImageDeclarations', bytes.fromhex('0a00'), max_fields=0)


if __name__=='__main__': unittest.main()
