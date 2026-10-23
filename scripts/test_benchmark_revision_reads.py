"""The removed PostgreSQL benchmark cannot provision a cluster through a direct backdoor."""
import contextlib
import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

import benchmark_revision_reads as benchmark


class RetiredRevisionBenchmarkTests(unittest.TestCase):
    def test_imported_entrypoint_refuses_before_probing_or_spawning(self):
        with patch.object(sys, "argv", ["benchmark_revision_reads.py", "--scripts", "5000"]), \
                patch.object(subprocess, "run") as run, \
                patch.object(subprocess, "Popen") as spawn, \
                contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(benchmark.main(), 2)
        run.assert_not_called()
        spawn.assert_not_called()

    def test_actual_cli_refuses_old_flags_and_optimized_python_without_tools(self):
        for optimized in ([], ["-O"]):
            for arguments in ([], ["--help"], ["--scripts", "5000"], ["--unknown"]):
                with self.subTest(optimized=optimized, arguments=arguments):
                    result = subprocess.run(
                        [sys.executable, *optimized, str(Path(benchmark.__file__)), *arguments],
                        env={"PATH": "", "DATABASE_URL": "postgresql://private.invalid/live",
                             "PGHOST": "private.invalid", "PYTHONDONTWRITEBYTECODE": "1"},
                        text=True, capture_output=True, timeout=5)
                    self.assertEqual(result.returncode, 2)
                    self.assertEqual(result.stdout, "")
                    self.assertIn("is retired", result.stderr)
                    self.assertNotIn("private.invalid", result.stderr)
                    self.assertNotIn("Traceback", result.stderr)


if __name__ == "__main__":
    unittest.main()
