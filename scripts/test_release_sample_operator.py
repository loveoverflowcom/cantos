"""Pure boundary tests; actual binary/image/sample qualification is separate."""
import hashlib
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
    def test_pull_failure_reports_only_a_fixed_class(self):
        fixture = SimpleNamespace(podman=['podman'], authfile=Path('/private/auth'), root=Path('/'), env={},
                                  image=lambda _: 'example/image@sha256:'+'a'*64)
        for error, category in ((b'no space left on device private-token', 'STORAGE_EXHAUSTED'),
                                (b'unauthorized private-token', 'REGISTRY_AUTH'),
                                (b'Error: the specified runroot is longer than 50 characters private-token', 'RUNROOT_TOO_LONG'),
                                (b'unknown private-token', 'UNCLASSIFIED')):
            with patch.object(qualify.subprocess, 'run', return_value=SimpleNamespace(returncode=125, stderr=error)):
                with self.assertRaises(ValueError) as caught:
                    qualify.pull_exact(fixture, 'server')
                self.assertEqual(str(caught.exception), 'PODMAN_PULL_'+category)
                self.assertNotIn('private-token', str(caught.exception))

    def test_fixture_runroot_fits_podman_4_9_limit(self):
        # The supported runner uses Podman 4.9, which rejects runroot above 50 bytes.
        # Reach the real qualifier's workspace allocation; no Podman or DB is started.
        class ReachedFixture(Exception):
            pass

        root = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory() as tmp:
            binary = Path(tmp)/'operator'
            binary.write_bytes(b'x'*64)
            base = 'docker.io/library/rust:fixture@sha256:'+'e'*64
            record = {'schema_version': 1, 'operation': 'BUILD_SAMPLE_OPERATOR', 'status': 'BUILT_NOT_QUALIFIED',
                      'source_sha': 'a'*40, 'platform': 'linux/amd64', 'configuration_sha256': 'c'*64,
                      'builder_base': base, 'builder_image_id': 'sha256:'+'d'*64,
                      'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(), 'binary_bytes': 64,
                      'admission': 'NOT_RUN', 'activation': 'NOT_RUN'}
            manifest = {'source_sha': 'a'*40, 'platform': 'linux/amd64', 'configuration_sha256': 'c'*64}
            args = SimpleNamespace(binary=binary, build_record=Path(tmp)/'build.json', build_record_digest=None,
                                   output=Path(tmp)/'out', authfile=Path(tmp)/'auth')

            def observe_fixture(source_root, work, candidate, authfile):
                self.assertEqual(source_root, root)
                self.assertLessEqual(len(str(work/'run').encode()), 50)
                raise ReachedFixture()

            with patch.object(qualify.qualify, 'validate_input', return_value=(manifest, {})), \
                    patch.object(qualify.qualify, 'native_host', return_value='linux/amd64'), \
                    patch.object(qualify.contract, 'read_json', return_value=record), \
                    patch.object(qualify.image_build, 'git_bytes', return_value=('FROM '+base+' AS build\n').encode()), \
                    patch.object(qualify.tempfile, 'tempdir', '/tmp'), \
                    patch.object(qualify.qualify, 'Fixture', side_effect=observe_fixture):
                with self.assertRaises(ReachedFixture):
                    qualify.qualify_operator(root, args)

    def test_wrong_platform_refuses_before_compiler_or_output(self):
        with tempfile.TemporaryDirectory() as tmp:
            output = Path(tmp).resolve()/'artifact'
            with patch.object(build.build, 'preflight', side_effect=ValueError('native Linux required')), \
                    patch.object(build.build, 'sh') as command:
                with self.assertRaisesRegex(ValueError, 'native Linux'):
                    build.produce(Path(tmp), output)
                command.assert_not_called()
                self.assertFalse(output.exists())

    def test_build_and_qualification_share_one_builder_and_size_rule(self):
        base = 'docker.io/library/rust:fixture@sha256:'+'e'*64
        self.assertIs(qualify.operator_build, build)
        self.assertEqual(build.pinned_builder('FROM '+base+' AS build\nFROM runtime AS final\n'), base)
        for containerfile in ('FROM docker.io/library/rust:latest AS build\n',
                              'FROM '+base+' AS build\nFROM '+base+' AS build\n', ''):
            with self.assertRaisesRegex(ValueError, 'exact pinned Rust builder required'):
                build.pinned_builder(containerfile)
        for size, admitted in ((63, False), (64, True), (512*1024**2, True), (512*1024**2+1, False)):
            self.assertEqual(size in build.BINARY_BYTES, admitted, size)

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
