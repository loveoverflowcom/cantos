import json
import re
import unittest
from reference_protobuf import ROOT, FIELDS, DIRECTORY, encode, varint

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

    def test_external_schema_is_registered_for_warm_cargo_regeneration(self):
        build = (ROOT/'scripts/protobuf-oracle/rust/build.rs').read_text()
        self.assertIn('println!("cargo:rerun-if-changed=../../../packages/contracts/protobuf/studio_v1.proto");', build)

    def test_every_reviewed_golden_matches_independent_wire_spec(self):
        corpus=json.loads((DIRECTORY/'goldens.json').read_text())
        self.assertEqual(corpus['contract'],'cantos.studio.wire.v1')
        self.assertEqual(len(corpus['cases']),13)
        for case in corpus['cases']:
            self.assertEqual(encode(case['message'],case['value']).hex(),case['protobuf_hex'],case['name'])

if __name__=='__main__': unittest.main()
