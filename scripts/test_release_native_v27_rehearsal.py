"""Transition and receipt controls with a named fake runtime; not image qualification."""
import copy
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import release_native_v27_rehearsal as v27
from test_release_native_migration import releases

class Fixture:
    adapter_record = {'fixture':'fake'}
    def close(self): pass

def pair():
    old, new = releases()
    old['compatibility']['server_schema'] = v27.SCHEMA_24
    new['compatibility']['server_schema'] = v27.SCHEMA_27
    return old, new

class V27RehearsalTests(unittest.TestCase):
    def test_only_exact_24_to_27_and_unchanged_other_contracts(self):
        old, new = pair()
        v27.transition(old, new)
        for side, field, value in [('old','server_schema','server-turso-epoch-1-v23'),
                                   ('new','server_schema','server-turso-epoch-1-v25'),
                                   ('new','rpc','different'), ('new','security_state','different')]:
            a, b = copy.deepcopy(old), copy.deepcopy(new)
            (a if side == 'old' else b)['compatibility'][field] = value
            with self.subTest(side=side, field=field), self.assertRaises(ValueError):
                v27.transition(a,b)

    def test_receipt_is_separate_and_failure_never_becomes_pass(self):
        old, new = pair()
        for outcome in [ValueError('UPGRADE_FAILED'), {'fake':'backup'}]:
            with self.subTest(outcome=type(outcome).__name__), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp).resolve()
                args = SimpleNamespace(output=root/'out', baseline=root/'baseline', baseline_digest='b'*64,
                                       manifest_digest='c'*64, authfile=None)
                run = {'side_effect':outcome} if isinstance(outcome, Exception) else {'return_value':outcome}
                with patch.object(v27.qualify, 'validate_input', return_value=(new, {})), \
                     patch.object(v27.migration, 'baseline', return_value=old), \
                     patch.object(v27.contract, 'read_json', return_value=old), \
                     patch.object(v27.qualify, 'native_host', return_value='linux/amd64'), \
                     patch.object(v27.qualify, 'fixture_configuration', return_value={'fake':True}), \
                     patch.object(v27.qualify, 'Fixture', return_value=Fixture()), \
                     patch.object(v27.historical, 'run', **run) as executed:
                    record = v27.rehearse(root,args)
                    self.assertEqual(record['operation'], 'HISTORICAL_V24_V27_REHEARSAL')
                    self.assertEqual(record['status'], 'FAIL' if isinstance(outcome, Exception) else 'PASS')
                    self.assertEqual(record['admission'], 'NOT_RUN')
                    self.assertEqual(record['activation'], 'NOT_RUN')
                    self.assertEqual(record['live_readiness'], 'NOT_RUN')
                    if not isinstance(outcome, Exception):
                        self.assertEqual(record['checks'], v27.CHECKS)
                        self.assertEqual(executed.call_args.kwargs, {'old_version':'v24','new_version':'v27'})
                    else:
                        self.assertEqual(record['checks'], [])
                    self.assertEqual(json.loads((args.output/'attempt.json').read_bytes())['status'], 'NOT_RUN')
                    with self.assertRaises(FileExistsError): v27.rehearse(root,args)

if __name__ == '__main__': unittest.main()
