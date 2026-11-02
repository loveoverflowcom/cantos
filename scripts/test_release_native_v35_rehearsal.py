"""Pure transition/receipt regressions with named runtime doubles, not image acceptance."""
import copy
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_native_v35_rehearsal as v35
from test_release_native_migration import releases


def pair():
    old, new = releases()
    old['compatibility']['server_schema'] = v35.SCHEMA_27
    new['compatibility']['server_schema'] = v35.SCHEMA_35
    return old, new


class FixtureDouble:
    adapter_record = {'fixture': 'runtime-double'}
    def close(self): pass


class V35Rehearsal(unittest.TestCase):
    def test_only_exact_suffix_with_unchanged_other_compatibility_is_accepted(self):
        old, new = pair()
        v35.transition(old, new)
        for side, key, value in [('old', 'server_schema', 'server-turso-epoch-1-v24'),
                                 ('new', 'server_schema', 'server-turso-epoch-1-v36'),
                                 ('new', 'identity_schema', 'identity-migrations-v5'),
                                 ('new', 'rpc', 'other'), ('new', 'security_state', 'other')]:
            before, after = copy.deepcopy(old), copy.deepcopy(new)
            (before if side == 'old' else after)['compatibility'][key] = value
            with self.subTest(side=side, key=key), self.assertRaises(ValueError):
                v35.transition(before, after)
        new['platform'] = 'linux/arm64'
        with self.assertRaisesRegex(ValueError, 'platform mismatch'):
            v35.transition(old, new)

    def test_success_and_failure_receipts_never_grant_activation_or_restore_acceptance(self):
        old, new = pair()
        for succeeds in (False, True):
            with self.subTest(succeeds=succeeds), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                args = SimpleNamespace(output=root/'out', baseline=root/'baseline',
                                       baseline_digest='b'*64, manifest_digest='c'*64, authfile=None)
                outcome = {'return_value': {'synthetic': 'copy'}} if succeeds else {'side_effect': ValueError('REVOCATION_LOST')}
                with patch.object(v35.qualify, 'validate_input', return_value=(new, {})), \
                     patch.object(v35.migration, 'baseline', return_value=old), \
                     patch.object(v35.contract, 'read_json', return_value=old), \
                     patch.object(v35.qualify, 'native_host', return_value='linux/amd64'), \
                     patch.object(v35.qualify, 'fixture_configuration', return_value={'fake': True}), \
                     patch.object(v35.qualify, 'Fixture', return_value=FixtureDouble()), \
                     patch.object(v35, 'run', **outcome):
                    record = v35.rehearse(root, args)
                    self.assertEqual(record['status'], 'PASS' if succeeds else 'FAIL')
                    self.assertEqual(record['operation'], v35.OPERATION)
                    self.assertEqual(record['checks'], v35.CHECKS if succeeds else [])
                    for key in ('activation', 'admission', 'live_readiness', 'executor_transition', 'historical_snapshot_restore'):
                        self.assertEqual(record[key], 'NOT_RUN')
                    self.assertEqual(record['recovery_acceptance'], 'NO_GO_REMAINS_SEPARATE')
                    self.assertEqual(json.loads((args.output/'attempt.json').read_text())['status'], 'NOT_RUN')
                    with self.assertRaises(FileExistsError):
                        v35.rehearse(root, args)


if __name__ == '__main__': unittest.main()
