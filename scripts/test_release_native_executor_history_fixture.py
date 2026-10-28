"""Synthetic guard checks; no real host or rehearsal claim."""
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import release_native_executor_history_fixture as fixture


class HistoryFixture(unittest.TestCase):
    def test_non_runner_refuses_before_creating_any_attempt(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            root.chmod(0o700)
            marker = root/'disposable-fixture-marker'
            marker.write_bytes(b'0'*32)
            marker.chmod(0o600)
            with patch.object(fixture.native, 'STATE', root), patch.object(fixture.migration, 'transition'), \
                    patch.dict(os.environ, {'GITHUB_ACTIONS': 'false', 'RUNNER_ENVIRONMENT': 'github-hosted'}):
                with self.assertRaisesRegex(ValueError, 'owned disposable fixture marker'):
                    fixture.create({}, {}, {}, {}, 'a'*40, 'b'*64, b'0'*32)
            self.assertEqual({path.name for path in root.iterdir()}, {'disposable-fixture-marker'})


if __name__ == '__main__':
    unittest.main()
