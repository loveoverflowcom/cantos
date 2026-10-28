"""Safety boundary only; this does not assert real systemd or runtime behavior."""
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_native_executor_systemd_fixture as fixture


class RealFixturePrerequisite(unittest.TestCase):
    def test_prerequisite_refusal_precedes_source_read_and_host_effects(self):
        args = SimpleNamespace()
        with patch.object(fixture.probe, 'preflight', side_effect=ValueError('not disposable')):
            with patch.object(fixture.qualify, 'validate_input') as source:
                with self.assertRaisesRegex(ValueError, 'not disposable'):
                    fixture.run(Path('/fixture'), args)
                source.assert_not_called()


if __name__ == '__main__':
    unittest.main()
