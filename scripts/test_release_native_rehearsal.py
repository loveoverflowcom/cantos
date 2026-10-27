"""Historical receipt failure/interrupt regressions with a named fake runtime; no image acceptance."""
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_native_rehearsal as rehearsal
from test_release_native_migration import releases


class FailedRuntime:
    def __init__(self, *_):
        self.closed = False

    def close(self):
        self.closed = True


class ReceiptFailures(unittest.TestCase):
    def execute(self, root, failure):
        old, new = releases()
        args = SimpleNamespace(output=root/'receipt', baseline=root/'baseline.json', baseline_digest='b'*64,
                               manifest_digest='c'*64, authfile=None)
        runtime = FailedRuntime()
        with patch.object(rehearsal.qualify, 'validate_input', return_value=(new, {})), \
                patch.object(rehearsal.migration, 'baseline', return_value=old), \
                patch.object(rehearsal.contract, 'read_json', return_value=old), \
                patch.object(rehearsal.qualify, 'native_host', return_value='linux/amd64'), \
                patch.object(rehearsal.qualify, 'fixture_configuration', return_value={'fake': 'unit test only'}), \
                patch.object(rehearsal.qualify, 'Fixture', return_value=runtime), \
                patch.object(rehearsal, 'run', side_effect=failure):
            if isinstance(failure, KeyboardInterrupt):
                with self.assertRaises(KeyboardInterrupt):
                    rehearsal.rehearse(root, args)
            else:
                result = rehearsal.rehearse(root, args)
                self.assertEqual(result['status'], 'FAIL')
                self.assertNotIn('evidence_level', result)
                self.assertEqual(result['baseline'], old)
                self.assertEqual(result['target'], rehearsal.migration.new_identity(new))
                self.assertEqual(result['admission'], 'NOT_RUN')
            with self.assertRaises(FileExistsError):
                rehearsal.rehearse(root, args)
        self.assertTrue(runtime.closed)
        return args.output

    def test_engine_image_or_http_failure_never_becomes_pass_or_relabels_old_receipts(self):
        for failure in [ValueError('UPGRADE_FAILED'), OSError('private payload is never reported')]:
            with self.subTest(failure=type(failure).__name__), tempfile.TemporaryDirectory() as temporary:
                output = self.execute(Path(temporary).resolve(), failure)
                record = json.loads((output/'historical-upgrade.json').read_bytes())
                self.assertEqual(record['checks'], [])
                self.assertIn(record['failure'], ('UPGRADE_FAILED', 'OSError'))
                self.assertFalse((output/'gate-records').exists())

    def test_process_interruption_keeps_only_unfinished_attempt_and_refuses_replay(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = self.execute(Path(temporary).resolve(), KeyboardInterrupt())
            self.assertEqual(json.loads((output/'attempt.json').read_bytes())['status'], 'NOT_RUN')
            self.assertFalse((output/'historical-upgrade.json').exists())


if __name__ == '__main__':
    unittest.main()
