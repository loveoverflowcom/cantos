"""Static crash-target and refusal controls; no actual systemd/Podman evidence."""
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_native_executor_rehearsal as rehearsal
import release_native_forward as forward
import release_native_v27_live as live


class Rehearsal(unittest.TestCase):
    def test_expected_crash_boundaries_are_unambiguous_in_the_actual_sources(self):
        cases = [(forward.__file__,
                  "        contract.require(native.digest_file(state/'in-progress.json') == request['marker_sha256'], 'original marker changed at commit')",
                  "        (state/'in-progress.json').unlink()"),
                 (live.__file__, '        ports.migrate(layout,target27)',
                  '        forward._unit_and_config_fence(prior,original,corrected,metadata)'),
                 (live.__file__, '                if not destination.exists():',
                  '                    staged[service].rename(destination);native.sync_directory(destination.parent)'),
                 (live.__file__, '        ports.observe(new_layout,target27)',
                  "        forward._unit_and_config_fence({'units':new_layout['units']},original,new_units,metadata)")]
        for source, anchor, wanted in cases:
            with self.subTest(source=source, anchor=anchor):
                path, digest, line, actual = rehearsal.target(Path(source), anchor, wanted)
                self.assertEqual(actual, wanted)
                self.assertEqual(len(digest), 64)
                self.assertEqual(path.read_text().splitlines()[line-1], wanted)

    def test_fixture_refusal_prevents_executor_calls_and_result_creation(self):
        with patch.object(rehearsal.fixture, 'setup', side_effect=ValueError('fixture refused')):
            with patch.object(rehearsal.forward, 'forward_execute') as execute:
                with self.assertRaisesRegex(ValueError, 'fixture refused'):
                    rehearsal.run(Path('/fixture'), SimpleNamespace())
                execute.assert_not_called()


if __name__ == '__main__':
    unittest.main()
