"""Only probe decisions with synthetic host observations; never host/runtime evidence."""
import json
import unittest
from unittest.mock import patch

import release_native_executor_probe as probe


class Probe(unittest.TestCase):
    def setUp(self):
        patches = [patch.object(probe.platform, 'system', return_value='Linux'),
                   patch.object(probe.platform, 'machine', return_value='x86_64'),
                   patch.object(probe.os, 'geteuid', return_value=0),
                   patch.dict(probe.os.environ, {'GITHUB_ACTIONS': 'true', 'RUNNER_ENVIRONMENT': 'github-hosted'}),
                   patch.object(probe.Path, 'is_dir', return_value=True),
                   patch.object(probe.Path, 'is_file', return_value=True),
                   patch.object(probe.Path, 'exists', return_value=False),
                   patch.object(probe.Path, 'is_symlink', return_value=False),
                   patch.object(probe.shutil, 'which', return_value='/fixture/binary'),
                   patch.object(probe.os, 'access', return_value=True)]
        for item in patches:
            item.start()
            self.addCleanup(item.stop)

    @staticmethod
    def response(*args):
        if args[:2] == ('systemctl', 'show') and '--property=SystemState' in args:
            return b'running\n'
        if args[:2] == ('systemctl', 'show'):
            return b'not-found\n'
        if args[0] == 'podman' and 'info' in args:
            return json.dumps({'host': {'cgroupVersion': 'v2', 'security': {'rootless': False}}}).encode()
        if args[0] == 'podman' and 'ps' in args:
            return b'[]'
        raise AssertionError('unexpected command')

    def test_prerequisites_are_not_transition_evidence(self):
        with patch.object(probe, 'command', side_effect=self.response):
            self.assertEqual(probe.preflight(), {'status': 'PREREQUISITES_ONLY',
                'executor_transition': 'NOT_RUN', 'activation': 'NOT_RUN'})

    def test_existing_reserved_path_refuses_before_any_command(self):
        with patch.object(probe.Path, 'exists', return_value=True), patch.object(probe, 'command') as command:
            with self.assertRaisesRegex(ValueError, 'path already exists'):
                probe.preflight()
            command.assert_not_called()

    def test_existing_service_container_or_non_hosted_runner_refuses(self):
        def occupied(*args):
            if args[0] == 'podman' and 'ps' in args:
                return b'[{"Names":["cantos-server"]}]'
            return self.response(*args)
        with patch.object(probe, 'command', side_effect=occupied):
            with self.assertRaisesRegex(ValueError, 'container name already exists'):
                probe.preflight()
        with patch.dict(probe.os.environ, {'RUNNER_ENVIRONMENT': 'self-hosted'}), patch.object(probe, 'command') as command:
            with self.assertRaisesRegex(ValueError, 'disposable GitHub Linux runner'):
                probe.preflight()
            command.assert_not_called()


if __name__ == '__main__':
    unittest.main()
