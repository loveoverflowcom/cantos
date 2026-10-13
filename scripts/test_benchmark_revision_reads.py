"""Determinism/accounting guards for the safe synthetic benchmark (PG assertions live in harness)."""
import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

import benchmark_revision_reads as bench


class BenchmarkTests(unittest.TestCase):
    def test_nearest_rank_retains_outliers(self):
        samples = [{"ms": i} for i in range(1, 101)]
        summary = bench.summarize(samples, 2)
        self.assertEqual((summary["p50_ms"], summary["p95_ms"], summary["p99_ms"]), (50, 95, 99))
        self.assertEqual(summary["requests_per_second"], 50)
        self.assertEqual(summary["max_ms"], 100)

    def test_generator_preserves_vietnamese_and_source_fixture(self):
        original = bench.FIXTURE.read_bytes()
        document = bench.make_document(512)
        scenes = document["episode"]["acts"][0]["scenes"]
        lines = [line for scene in scenes for line in scene["dialogues"]]
        self.assertEqual(len(lines), 512)
        self.assertEqual(len({line["id"] for line in lines}), 512)
        self.assertEqual(document["work"]["title"], "Ánh đèn cuối sân khấu")
        self.assertEqual(document, bench.make_document(512))
        self.assertEqual(bench.FIXTURE.read_bytes(), original)

    def test_owner_skew_and_foreign_owners_are_distinct(self):
        owners = [bench.owner(i, 100) for i in range(100)]
        self.assertEqual(owners.count("owner0"), 50)
        self.assertNotEqual(owners[-1], owners[0])
        self.assertEqual(len(set(owners)), 32)
        self.assertEqual(bench.script_id(0), "00000000-0000-0000-0000-000000000001")

    def test_existing_database_and_pg_service_are_never_inherited(self):
        inherited = {"DATABASE_URL": "postgresql://example.invalid/production", "PGSERVICE": "production",
                     "PGPASSWORD": "not-a-real-secret", "PGOPTIONS": "-c search_path=private"}
        with patch.dict(os.environ, inherited), patch.object(bench, "free_port", return_value=54321), \
                tempfile.TemporaryDirectory() as directory:
            instance = bench.Benchmark(Path(directory), None)
        for key in inherited:
            self.assertNotIn(key, instance.env)
        self.assertIn("@127.0.0.1:", instance.admin_url)
        self.assertTrue(instance.admin_url.endswith("/postgres"))

    def test_extracted_sql_keeps_access_locks_and_complete_projection(self):
        queries = bench.source_queries()
        self.assertIn("AND NOT s.revoked", queries["auth"])
        self.assertIn("coalesce(to_jsonb(s)->>'auth_kind','development')", queries["auth"])
        self.assertIn("FOR SHARE OF s", queries["auth"])
        self.assertIn("AND actor_id=$2", queries["member"])
        self.assertTrue(queries["head_lock"].endswith("FOR SHARE"))
        self.assertIn("canonical_export, content_digest, export_digest", queries["revision"])
        self.assertIn("accepted_by=$2 AND operation_id=$3", queries["replay"])
        self.assertIn("owner_id=$1 AND kind=$2 AND id=$3", queries["evidence"])

    def test_phase_databases_clone_the_same_seed_instead_of_prior_writes(self):
        with patch.object(bench, "free_port", return_value=54321), tempfile.TemporaryDirectory() as directory:
            instance = bench.Benchmark(Path(directory), None)
            calls = []

            def capture_sql(query, app=False):
                calls.append((instance.admin_url, query))
                return '{"scripts":100,"revisions":2495,"members":200,"evidence_links":12475}'

            with patch.object(instance, "sql", side_effect=capture_sql):
                first = instance.clone_snapshot("baseline")
                second = instance.clone_snapshot("member_cover")
            self.assertEqual(first, second)
            self.assertTrue(calls[0][0].endswith("/postgres"))
            self.assertTrue(calls[2][0].endswith("/postgres"))
            self.assertEqual(calls[0][1], "CREATE DATABASE cantos_bench_baseline TEMPLATE postgres")
            self.assertEqual(calls[2][1], "CREATE DATABASE cantos_bench_member_cover TEMPLATE postgres")
            self.assertTrue(instance.app_url.endswith("/cantos_bench_member_cover"))

    def test_sql_literal_quotes_instead_of_executing_path_content(self):
        self.assertEqual(bench.sql_literal("a'b"), "'a''b'")
        self.assertEqual(bench.sql_literal("$HOME`id`"), "'$HOME`id`'")

    def test_tokens_and_cpu_units_are_reproducible(self):
        self.assertEqual(bench.token("reader"), hashlib.sha256(b"cantos-benchmark-only:reader").hexdigest())
        self.assertEqual(bench.cpu_seconds("1:02.50"), 62.5)
        self.assertEqual(bench.cpu_seconds("1:02:03"), 3723)

    def test_optimized_python_cannot_disable_the_oracle_and_report_pass(self):
        result = subprocess.run([sys.executable, "-O", str(Path(bench.__file__)), "--help"],
                                text=True, capture_output=True, timeout=10)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("benchmark assertions are required; run Python without -O", result.stderr)
        self.assertNotIn("PASS:", result.stdout)

    def test_main_pairs_seed_inputs_in_every_comparison_phase(self):
        # Exercise the real orchestrator; PG/process seams are doubles, not integration evidence.
        from contextlib import ExitStack
        workload = MagicMock()
        replacements = {
            "start": MagicMock(), "seed": MagicMock(return_value={}),
            "sql": MagicMock(return_value="{}"), "clone_snapshot": MagicMock(return_value={}),
            "start_host": MagicMock(), "stop_host": MagicMock(), "close": MagicMock(),
            "check_semantics": MagicMock(return_value=[]), "plans": MagicMock(),
            "storage": MagicMock(return_value={}), "workload": workload,
            "membership_write_cost": MagicMock(return_value={}),
        }
        with tempfile.TemporaryDirectory() as directory, ExitStack() as stack:
            stack.enter_context(patch.object(bench, "OUTPUT", Path(directory)))
            stack.enter_context(patch.object(bench, "free_port", return_value=54321))
            stack.enter_context(patch.object(bench, "run", return_value="{}"))
            stack.enter_context(patch.object(bench.shutil, "which", return_value="test-double"))
            stack.enter_context(patch.object(bench.shutil, "disk_usage", return_value=SimpleNamespace(free=6 * 1024 ** 3)))
            stack.enter_context(patch.object(sys, "argv", ["benchmark_revision_reads.py", "--scripts", "100",
                                                         "--samples", "100", "--repetitions", "2"]))
            stack.enter_context(patch("builtins.print"))
            stack.enter_context(patch.multiple(bench.Benchmark, **replacements))
            bench.main()
        paired = {}
        for call in workload.call_args_list:
            label, *inputs = call.args
            paired.setdefault(label, []).append((inputs, call.kwargs))
        self.assertEqual(len(paired["baseline"]), 16)  # 2 repetitions × (cold + 7 warm).
        self.assertEqual(paired["baseline"], paired["member_cover"])
        self.assertEqual(paired["baseline"], paired["baseline_return"])


if __name__ == "__main__":
    unittest.main()
