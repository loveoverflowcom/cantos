"""Safety and pinned crash-target checks; not real systemd qualification."""
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import release_native_v36_executor_rehearsal as rehearsal
import release_native_v36_live as live
from test_release_native_v36_rehearsal import pair


class DisposableBoundary(unittest.TestCase):
    def test_initial_fixture_target_is_the_manifest_not_the_running_baseline(self):
        old,new=pair()
        layout={'fixture':'synthetic host boundary'}
        # setup returns system/layout/previous/running/TARGET/history/marker. For this
        # initial lane the first two release values both name the old pair.
        with patch.object(rehearsal.fixture,'setup',return_value=(None,layout,old,old,new,{},b'marker')):
            with patch.object(rehearsal.live,'InitialSystem') as factory:
                system=factory.return_value
                system.start.side_effect=RuntimeError('validated target reached old-start boundary')
                with self.assertRaisesRegex(RuntimeError,'validated target reached'):
                    rehearsal.run(Path('/fixture'),SimpleNamespace())
                system.start.assert_called_once_with(layout,old)

    def test_untrusted_runner_refuses_before_source_reads_or_host_effects(self):
        with patch.object(rehearsal.fixture.probe,'preflight',side_effect=ValueError('not disposable')):
            with patch.object(rehearsal.fixture.qualify,'validate_input') as read:
                with self.assertRaisesRegex(ValueError,'not disposable'):
                    rehearsal.run(Path('/fixture'),SimpleNamespace())
                read.assert_not_called()

    def test_each_real_interruption_anchor_identifies_one_exact_source_boundary(self):
        for anchor,wanted in [
            ("        phase(state, attempt, digest, 'STOP_INTENT')",'        stopped = ports.stop(request)'),
            ('            ports.database_command(new, service, path, check=True)',"        phase(state, attempt, digest, 'ENROLL_INTENT')"),
            ("            observed = ports.enroll_or_reconcile(new,service,Path(request['databases'][service]))","            path = attempt/(service+'-enrollment.json')"),
            ('            ports.start(layout,new)','            ports.observe_target(layout,new)')]:
            path,digest,line,actual=rehearsal.trace.target(Path(live.__file__),anchor,wanted)
            self.assertEqual(actual,wanted)
            self.assertEqual(len(digest),64)
            self.assertEqual(path.read_text().splitlines()[line-1],wanted)
