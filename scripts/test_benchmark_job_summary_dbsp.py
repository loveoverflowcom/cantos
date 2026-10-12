"""The benchmark summary keeps every repetition's range and no row data; no cluster is started."""
import unittest

import benchmark_job_summary_dbsp as benchmark


def series(p50):
    return {"samples": 10, "p50_ms": p50, "p95_ms": p50 * 2, "p99_ms": p50 * 3, "max_ms": p50 * 4}


def run(p50):
    return {
        "fixture": {"total_steps": 3},
        "statements": {"original/steps_4": [series(p50), series(p50 + 2)]},
        "api": {"postgres/steps_4/concurrency_8": [dict(series(p50), per_second=100.0 * p50)]},
        "write_cost": {"capture_on_1": dict(series(p50), wal_bytes_per_commit=10.0,
                                            outbox_bytes_growth=8192)},
        **{key: {} for key in ("bootstrap", "freshness_commit_to_served", "burst_2000_commits",
                               "hot_job_under_writes", "serve_with_view_stopped",
                               "partial_index_candidate", "view_after_reads", "view_final")},
        "postgres_version": "PostgreSQL 17",
    }


class SummaryTests(unittest.TestCase):
    def test_medians_and_ranges_span_every_run_and_repetition(self):
        record = {"provenance": {"source_sha": "abc"}, "command": "c", "runs": [run(1.0), run(5.0)]}
        summary = benchmark.summarize(record)
        original = summary["series"]["statements/original/steps_4"]
        self.assertEqual(original["p50_ms"], {"median": 4.0, "min": 1.0, "max": 7.0,
                                              "repetitions": 4})
        self.assertEqual(original["samples"], 40)
        throughput = summary["series"]["api/postgres/steps_4/concurrency_8"]["per_second"]
        self.assertEqual((throughput["min"], throughput["max"]), (100.0, 500.0))
        self.assertEqual(summary["series"]["write_cost/capture_on_1"]["outbox_bytes_growth"],
                         [8192, 8192])
        self.assertEqual(len(summary["per_run"]), 2)
        self.assertEqual(summary["provenance"], {"source_sha": "abc"})


if __name__ == "__main__":
    unittest.main()
