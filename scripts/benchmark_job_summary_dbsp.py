#!/usr/bin/env python3
"""Bounded synthetic benchmark of the job summary: original/improved PostgreSQL and the DBSP view.

Creates a new disposable loopback PostgreSQL cluster per run, builds the release test binary
with the experimental `dbsp-experimental` feature, runs `bench_job_summary_read_models` and keeps
its raw JSON with provenance under ignored target/dbsp-evidence. Never reads DATABASE_URL or an
inherited cluster; all data is synthetic. No production workload or capacity is implied.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import time

from test_postgres import free_port, isolated_environment

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "dbsp-evidence"
TEST = ["cargo", "test", "--release", "-p", "cantos-api", "--features", "dbsp-experimental",
        "--test", "durable_postgres", "--locked", "--offline"]


def capture(command, env=None):
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True,
                            timeout=60)
    return result.stdout.strip() if result.returncode == 0 else None


def provenance():
    return {
        "source_sha": capture(["git", "rev-parse", "HEAD"]),
        "dirty_paths": (capture(["git", "status", "--porcelain"]) or "").splitlines(),
        "rustc": capture(["rustc", "--version"]),
        "postgres": capture(["postgres", "--version"]),
        "machine": platform.machine(),
        "system": platform.platform(),
        "cpu": capture(["sysctl", "-n", "machdep.cpu.brand_string"]),
        "logical_cpus": os.cpu_count(),
        "memory_bytes": capture(["sysctl", "-n", "hw.memsize"]),
        "build": "release test profile, dbsp 0.198.0, one DBSP worker, in-memory ephemeral",
        "view_config": "poll 20 ms, 1000 transactions per step, fallback limit 8",
        "cluster": "initdb --no-locale UTF8; fsync, synchronous_commit and full_page_writes on",
    }


def spread(values):
    """Median and range across runs × repetitions; every repetition is kept."""
    return {"median": statistics.median(values), "min": min(values), "max": max(values),
            "repetitions": len(values)}


def summarize(record):
    """Safe, small summary of a record: medians/ranges of each latency series and the
    single-valued observations per run. Contains no identifiers or row data."""
    runs = record["runs"]
    summary = {"provenance": record["provenance"], "command": record["command"],
               "fixture": runs[0]["fixture"], "series": {}, "per_run": []}
    for section in ("statements", "api"):
        for key in sorted(runs[0][section]):
            items = [item for run in runs for item in run[section][key]]
            entry = {metric: spread([item[metric] for item in items])
                     for metric in ("p50_ms", "p95_ms", "p99_ms")}
            entry["samples"] = sum(item["samples"] for item in items)
            if "per_second" in items[0]:
                entry["per_second"] = spread([item["per_second"] for item in items])
            summary["series"][f"{section}/{key}"] = entry
    for phase in runs[0]["write_cost"]:
        items = [run["write_cost"][phase] for run in runs]
        summary["series"][f"write_cost/{phase}"] = {
            **{metric: spread([item[metric] for item in items])
               for metric in ("p50_ms", "p95_ms", "p99_ms", "wal_bytes_per_commit")},
            "outbox_bytes_growth": [item["outbox_bytes_growth"] for item in items]}
    for run in runs:
        summary["per_run"].append({key: run[key] for key in (
            "bootstrap", "freshness_commit_to_served", "burst_2000_commits",
            "hot_job_under_writes", "serve_with_view_stopped", "partial_index_candidate",
            "view_after_reads", "view_final", "postgres_version")})
    return summary


def run_once(directory, env):
    cluster = directory / "cluster"
    data = cluster / "pgdata"
    cluster.mkdir(parents=True)
    port = free_port()
    url = f"postgresql://cantos_test_admin@127.0.0.1:{port}/postgres"
    started = False
    try:
        subprocess.run(["initdb", "-D", str(data), "--username=cantos_test_admin", "--auth=trust",
                        "--no-locale", "--encoding=UTF8"], cwd=ROOT, env=env, check=True,
                       capture_output=True, timeout=120)
        options = (f"-h 127.0.0.1 -p {port} -k /tmp -c max_wal_size=256MB "
                   "-c fsync=on -c synchronous_commit=on -c full_page_writes=on")
        subprocess.run(["pg_ctl", "-D", str(data), "-l", str(cluster / "postgres.log"), "-o",
                        options, "-w", "start"], cwd=ROOT, env=env, check=True,
                       capture_output=True, timeout=120)
        started = True
        subprocess.run(["psql", "-X", url, "-v", "ON_ERROR_STOP=1", "-c",
                        "CREATE ROLE cantos_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE"],
                       cwd=ROOT, env=env, check=True, capture_output=True, timeout=60)
        report = directory / "benchmark.json"
        run_env = dict(env, CANTOS_TEST_CLUSTER_URL=url, CANTOS_BENCH_OUTPUT=str(report))
        result = subprocess.run(TEST + ["--", "--ignored", "--exact",
                                        "dbsp_summary_benchmark::bench_job_summary_read_models",
                                        "--nocapture"],
                                cwd=ROOT, env=run_env, text=True, capture_output=True,
                                timeout=1800)
        (directory / "test.log").write_text(result.stdout + result.stderr)
        if result.returncode:
            raise RuntimeError(f"FAIL: benchmark exited {result.returncode}; see {directory}")
        return json.loads(report.read_text())
    finally:
        if started:
            subprocess.run(["pg_ctl", "-D", str(data), "-m", "fast", "-w", "stop"], cwd=ROOT,
                           env=env, capture_output=True, timeout=120)
        shutil.rmtree(data, ignore_errors=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runs", type=int, default=1, choices=range(1, 6),
                        help="fresh clusters to run, each with three interleaved repetitions")
    parser.add_argument("--summarize", type=Path, metavar="RECORD",
                        help="only rewrite summary.json beside an existing record.json")
    args = parser.parse_args()
    if args.summarize:
        record = json.loads(args.summarize.read_text())
        target = args.summarize.with_name("summary.json")
        target.write_text(json.dumps(summarize(record), indent=2) + "\n")
        print(f"summary {target}")
        return
    for tool in ("initdb", "pg_ctl", "psql", "cargo"):
        if not shutil.which(tool):
            raise RuntimeError(f"unavailable: {tool}")
    env = isolated_environment()
    build = subprocess.run(TEST + ["--no-run"], cwd=ROOT, env=env, text=True,
                           capture_output=True, timeout=3600)
    if build.returncode:
        print(build.stdout + build.stderr)
        raise RuntimeError("FAIL: release benchmark build")
    session = OUTPUT / time.strftime("%Y%m%dT%H%M%S")
    record = {"provenance": provenance(), "command": "python3 scripts/benchmark_job_summary_dbsp.py "
              f"--runs {args.runs}", "runs": []}
    for index in range(args.runs):
        record["runs"].append(run_once(session / f"run-{index + 1}", env))
    (session / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    (session / "summary.json").write_text(json.dumps(summarize(record), indent=2) + "\n")
    print(f"PASS: {args.runs} benchmark run(s); record and summary in {session}")


if __name__ == "__main__":
    main()
