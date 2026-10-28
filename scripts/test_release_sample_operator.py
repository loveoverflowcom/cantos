"""Pure boundary tests; actual binary/image/sample qualification is separate."""
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_artifact as contract
import release_sample_operator_build as build
import release_sample_operator_qualify as qualify
import release_oci_fixture as driver


class OperatorBoundary(unittest.TestCase):
    def test_wrong_platform_refuses_before_compiler_or_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp).resolve()/'artifact'
            with patch.object(build.build, 'preflight', side_effect=ValueError('native Linux required')), \
                    patch.object(build.build, 'sh') as command:
                with self.assertRaisesRegex(ValueError, 'native Linux'):
                    build.produce(Path(tmp), output)
                command.assert_not_called()
                self.assertFalse(output.exists())

    def test_synthetic_fixture_never_accepts_expanded_owner_selection(self):
        request = qualify.request('publish', '00000000-0000-4000-8000-000000000001')
        self.assertNotIn('owner_id', request)
        self.assertTrue(request['confirm_source'])
        self.assertTrue(request['confirm_metadata_rights'])
        denied = qualify.request('publish', '00000000-0000-4000-8000-000000000002', False)
        self.assertFalse(denied['confirm_metadata_rights'])

    def test_tampered_binary_refuses_before_disposable_podman_fixture(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp).resolve()
            binary = root/'operator'
            binary.write_bytes(b'tampered'*8)
            base = 'docker.io/library/rust:fixture@sha256:'+'e'*64
            record = {'schema_version': 1, 'operation': 'BUILD_SAMPLE_OPERATOR', 'status': 'BUILT_NOT_QUALIFIED',
                      'source_sha': 'a'*40, 'platform': 'linux/amd64', 'configuration_sha256': 'c'*64,
                      'builder_base': base, 'builder_image_id': 'sha256:'+'d'*64,
                      'binary_sha256': 'b'*64, 'binary_bytes': len(binary.read_bytes()),
                      'admission': 'NOT_RUN', 'activation': 'NOT_RUN'}
            record_path = root/'build.json'
            record_path.write_bytes(contract.canonical(record))
            args = SimpleNamespace(binary=binary, build_record=record_path, build_record_digest=None,
                                   output=root/'out')
            manifest = {'source_sha': 'a'*40, 'platform': 'linux/amd64', 'configuration_sha256': 'c'*64}
            with patch.object(qualify.qualify, 'validate_input', return_value=(manifest, {})), \
                    patch.object(qualify.qualify, 'native_host', return_value='linux/amd64'), \
                    patch.object(qualify.image_build, 'git_bytes', return_value=('FROM '+base+' AS build\n').encode()), \
                    patch.object(qualify.qualify, 'Fixture') as podman:
                with self.assertRaisesRegex(ValueError, 'operator build identity mismatch'):
                    qualify.qualify_operator(root, args)
                podman.assert_not_called()
                self.assertFalse(args.output.exists())

    def test_sample_http_checks_reject_playable_units(self):
        journey = driver.Journey(None)
        works = [{'work_id': str(i), 'cover': None} for i in range(6)]
        def response(method, path, expected, **kwargs):
            if path == '/api/v1/theatre/works?presentation=2':
                return {'items': []}, {}
            if path == '/api/v1/theatre/works?presentation=3':
                return {'items': works}, {}
            return {'items': [{'playback': None}, {'playback': {'publication_id': 'synthetic'}}]}, {}
        with patch.object(journey, 'http', side_effect=response):
            with self.assertRaisesRegex(driver.CheckFailed, 'sample-no-playback'):
                journey.sample_metadata(6)


if __name__ == '__main__':
    unittest.main()
