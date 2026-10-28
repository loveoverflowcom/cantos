"""Synthetic process controls for rehearsal machinery, not executor evidence."""
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest

import release_native_executor_trace as trace


class Trace(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name).resolve()
        self.source = root/'target.py'
        self.marker = root/'marker'
        self.source.write_text('from pathlib import Path\ndef target(path):\n'
                               '    Path(path).write_text("before")\n'
                               '    Path(path).write_text("after")\n')
        module_spec = importlib.util.spec_from_file_location('rehearsal_trace_target', self.source)
        self.module = importlib.util.module_from_spec(module_spec)
        module_spec.loader.exec_module(self.module)
        self.digest = hashlib.sha256(self.source.read_bytes()).hexdigest()
        self.line = '    Path(path).write_text("after")'

    def invoke(self):
        self.module.target(self.marker)

    def test_stops_at_pinned_line_and_reaps_its_child(self):
        result = trace.interrupt_at_line(self.source, self.digest, 'target', 4, self.line, self.invoke, timeout=5)
        self.assertEqual(result['status'], 'INTERRUPTED')
        self.assertEqual(self.marker.read_text(), 'before')

    def test_foreign_source_line_or_unreached_target_refused(self):
        for digest, line in [('0'*64, self.line), (self.digest, 'different')]:
            with self.subTest(digest=digest, line=line), self.assertRaises(ValueError):
                trace.interrupt_at_line(self.source, digest, 'target', 4, line, self.invoke, timeout=5)
        with self.assertRaises(RuntimeError):
            trace.interrupt_at_line(self.source, self.digest, 'missing', 4, self.line, self.invoke, timeout=5)


if __name__ == '__main__':
    unittest.main()
