"""Negative release/isolation evidence; synthetic bytes, no SDK, signer or Firebase."""
import hashlib
import tempfile
from pathlib import Path
import unittest
import zipfile

import android_boundary as boundary
import android_release_evidence as evidence
import inspect_android_release as inspector
from test_inspect_android_release import make_dex


class AndroidReleaseEvidenceTests(unittest.TestCase):
    def test_copied_fixture_class_and_renamed_resource_fail_the_source_boundary(self):
        with tempfile.TemporaryDirectory() as temporary:
            mobile = Path(temporary)
            sandbox = mobile / 'preview-sandbox/src/commonMain'
            sandbox.mkdir(parents=True)
            (sandbox / 'Seed.kt').write_text('package com.cantos.theatre.app\nclass PreviewSeed\n')
            resources = sandbox / 'composeResources'
            resources.mkdir()
            (resources / 'artwork.bin').write_bytes(b'unique sandbox artwork')
            production = mobile / 'shared/src/commonMain/kotlin'
            production.mkdir(parents=True)
            (production / 'Ordinary.kt').write_text('package com.cantos.theatre.app\nclass LiveCatalog\n')
            self.assertEqual(boundary.source_findings(mobile), [])
            (production / 'Renamed.kt').write_text('package com.cantos.theatre.app\nclass PreviewSeed\n')
            (production / 'ordinary.dat').write_bytes(b'unique sandbox artwork')
            findings = boundary.source_findings(mobile)
            self.assertTrue(any('sandbox class' in f for f in findings), findings)
            self.assertTrue(any('copied sandbox resource bytes' in f for f in findings), findings)

    def test_transitive_or_copied_compiled_test_classes_fail_before_r8(self):
        with tempfile.TemporaryDirectory() as temporary:
            jar = Path(temporary) / 'innocent-library.jar'
            with zipfile.ZipFile(jar, 'w') as archive:
                archive.writestr('com/cantos/theatre/app/LiveCatalog.class', b'class fixture')
            self.assertEqual(boundary.input_findings([jar])[0], [])
            for name in ('com/cantos/theatre/app/MockCatalog.class', 'kotlin/test/Asserter.class',
                         'io/ktor/client/engine/mock/MockEngine.class', 'org/junit/Test.class', 'junit/framework/TestCase.class'):
                with self.subTest(name=name):
                    with zipfile.ZipFile(jar, 'w') as archive:
                        archive.writestr(name, b'forbidden compiled fixture')
                    self.assertEqual(len(boundary.input_findings([jar])[0]), 1)
            with self.assertRaisesRegex(ValueError, 'no compiled'):
                boundary.input_findings([])

    def test_audit_bytes_and_missing_marker_are_rejected_independent_of_filename(self):
        elements = [(0, 'manifest', {}), (2, 'meta-data', {'name': 'CantosReleaseAudit', 'value': 'true'})]
        self.assertEqual(inspector.audit_findings(elements, True), [])
        self.assertIn('audit bytes cannot distribute', inspector.audit_findings(elements, False)[0])
        self.assertTrue(inspector.audit_findings([], False))
        elements[-1][2]['value'] = 'false'
        self.assertEqual(inspector.audit_findings(elements, False), [])
        self.assertTrue(inspector.audit_findings(elements, True))

    def test_stale_or_missing_source_marker_is_rejected_with_the_same_package_version(self):
        elements = [(2, 'meta-data', {'name': 'CantosBuildInputs', 'value': 'a' * 64})]
        self.assertEqual(inspector.build_input_findings(elements, 'a' * 64), [])
        self.assertEqual(inspector.build_input_findings(elements, 'b' * 64),
                         ['stale artifact: build inputs differ from current source/config'])
        self.assertTrue(inspector.build_input_findings([], 'a' * 64))

    def test_mapping_must_match_actual_dex_marker_for_apk_and_aab(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            mapping = root / 'mapping.txt'
            mapping.write_text('# pg_map_id: abc1234\ncom.cantos.theatre.app.TheatreActivity -> a.b:\n')
            for name in ('classes.dex', 'base/dex/classes.dex'):
                apk = root / 'artifact.zip'
                with zipfile.ZipFile(apk, 'w') as archive:
                    archive.writestr(name, make_dex(['La/b;']) + b'~~R8{"pg-map-id":"abc1234"}')
                self.assertEqual(inspector.mapping_classes(mapping, apk),
                                 {'Lcom/cantos/theatre/app/TheatreActivity;': 'La/b;'})
                mapping.write_text('# pg_map_id: wrong\n')
                with self.assertRaisesRegex(inspector.InspectionError, 'does not match'):
                    inspector.mapping_classes(mapping, apk)
                mapping.write_text('# pg_map_id: abc1234\ncom.cantos.theatre.app.TheatreActivity -> a.b:\n')

    def test_inspection_hashes_must_match_the_snapshotted_bytes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            digest = hashlib.sha256(b'original').hexdigest()
            (root / 'inspection.txt').write_text(f'apk app.apk sha256={digest} entries=2\naab app.aab sha256={digest} entries=2\n')
            records = {'inspection.txt': {'path': 'inspection.txt'},
                       'app.apk': {'sha256': digest}, 'app.aab': {'sha256': digest}}
            evidence.validate_inspected(root, records)
            records['app.apk']['sha256'] = hashlib.sha256(b'replaced after inspection').hexdigest()
            with self.assertRaisesRegex(ValueError, 'between inspection and snapshot'):
                evidence.validate_inspected(root, records)
        self.assertEqual(inspector.class_findings({'Lcom/cantos/app/Seed$Nested;'}, {'Lcom/cantos/app/Seed;'}),
                         ['defines sandbox class Lcom/cantos/app/Seed$Nested;'])

    def test_source_or_graph_mismatch_refuses_release_pair_and_empty_files_cannot_pass(self):
        audit = {'identity': {'source_sha': 'a' * 40, 'input_sha256': 'b' * 64}, 'graph': 'exact graph'}
        evidence.validate_pair(audit, dict(audit))
        for change, expected in [({'identity': {}}, 'source/config'), ({'graph': 'different'}, 'dependency graphs')]:
            with self.assertRaisesRegex(ValueError, expected):
                evidence.validate_pair(audit, dict(audit, **change))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'empty.apk').touch()
            with self.assertRaisesRegex(ValueError, 'missing/nonregular'):
                evidence.snapshot(root, root, root / 'copy', {'app.apk': 'empty.apk'})
            (root / 'input.apk').write_bytes(b'audited bytes')
            record = evidence.snapshot(root, root, root / 'copy', {'app.apk': 'input.apk'})['app.apk']
            self.assertEqual(record['sha256'], hashlib.sha256(b'audited bytes').hexdigest())
            (root / 'input.apk').write_bytes(b'changed build output')
            self.assertEqual((root / record['path']).read_bytes(), b'audited bytes')


if __name__ == '__main__':
    unittest.main()
