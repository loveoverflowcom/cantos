#!/usr/bin/env python3
"""Bounded synthetic benchmark of the implemented revision API; never uses an existing DB.

Requires local PostgreSQL binaries, cargo and macOS/POSIX ps. Creates a fresh cluster,
uses the migration binary and restricted app role, and stops all owned processes on exit.
Raw fixtures, plans and request/resource samples stay under ignored target/read-benchmarks.
No production endpoint, traffic forecast, OS cache eviction or DBSP runtime is implied.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import closing
import copy
import csv
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import random
import re
import resource
import shutil
import socket
import subprocess
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "target" / "read-benchmarks"
FIXTURE = ROOT / "packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
EVIDENCE_REFS = [("rights", "rights-demo"), ("rights", "rights-adaptation-demo"),
                 ("rights", "rights-asset-demo"), ("source", "source-demo"),
                 ("asset", "asset-demo")]
CANDIDATE = "CREATE INDEX bench_member_cover ON script_members(script_id,actor_id) INCLUDE(role)"
TABLES = "actors sessions scripts script_members script_evidence script_revisions revision_evidence".split()


def sql_literal(value):
    return "'" + str(value).replace("'", "''") + "'"


def percentile(values, percent):
    """Nearest-rank percentiles; retain all samples, including cold-start outliers."""
    return sorted(values)[max(0, math.ceil(len(values) * percent / 100) - 1)]


def summarize(samples, elapsed):
    values = [sample["ms"] for sample in samples]
    return {"samples": len(values), "seconds": elapsed, "requests_per_second": len(values) / elapsed,
            **{f"p{p}_ms": percentile(values, p) for p in (50, 95, 99)},
            "max_ms": max(values)}


def script_id(number):
    return str(uuid.UUID(int=number + 1))


def owner(number, scripts):
    # Half the scripts belong to one owner; the other half to 31 smaller owners.
    return "owner0" if number < scripts // 2 else f"owner{1 + number % 31}"


def token(actor):
    # Public synthetic tokens, valid only in a newly created disposable trust cluster.
    return hashlib.sha256(f"cantos-benchmark-only:{actor}".encode()).hexdigest()


def make_document(dialogues):
    document = json.loads(FIXTURE.read_text())
    if dialogues > 4:
        scene = document["episode"]["acts"][0]["scenes"][0]
        template = scene["dialogues"][0]
        for number in range(5, dialogues + 1):
            line = copy.deepcopy(template)
            line["id"] = f"bench-dialogue-{number}"
            line["text"] = f"Ánh đèn cuối sân khấu, lời thoại tổng hợp số {number}."
            scene["dialogues"].append(line)
    return document


# Every Rust file that owns a query on the revision read path.
REVISION_READ_SOURCES = (
    "services/cantos-server/src/platform/postgres/session.rs",
    "services/cantos-server/src/platform/postgres/migrations.rs",
    "services/cantos-server/src/contexts/editorial/postgres.rs",
)

# HTTP policy/wiring moved out of mod.rs; keep future measurements attributable to the
# actual request path without rewriting any historical benchmark report/hash inventory.
REVISION_HTTP_SOURCES = tuple(
    "services/cantos-server/src/adapters/inbound/http/" + path for path in (
        "mod.rs", "assembly.rs", "router.rs", "state.rs", "policy.rs", "errors.rs",
        "studio/mod.rs", "studio/admission.rs", "studio/editorial.rs", "wire.rs",
    )
)


def source_queries():
    """Extract the real SQL, failing closed on code drift instead of benchmarking a stale copy."""
    source = "\n".join((ROOT / path).read_text() for path in REVISION_READ_SOURCES)
    columns = re.search(r'const REVISION_COLUMNS: &str = ("(?:\\.|[^"\\])*");', source)
    if columns is None:
        raise RuntimeError("revision projection changed; update the read inventory/harness")
    columns = json.loads(columns[1])
    queries = {}
    for label, prefix in [("auth", "SELECT a.id,coalesce(to_jsonb(s)->>'auth_kind','development') FROM sessions"),
                          ("member", "SELECT role FROM script_members"),
                          ("evidence", "SELECT id FROM script_evidence WHERE"),
                          ("migration", "SELECT version,checksum FROM public.cantos_migrations ORDER BY version"),
                          ("head_lock", "SELECT owner_id,head_revision FROM scripts"),
                          ("revision", "SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=$1 AND revision=$2"),
                          ("replay", "SELECT {REVISION_COLUMNS} FROM script_revisions WHERE script_id=$1 AND accepted_by=$2")]:
        literals = re.findall(r'"(?:\\.|[^"\\])*"', source)
        # Only decode the selected query: unrelated Rust literals may contain physical
        # newlines (valid Rust, invalid JSON). Selected-query drift still fails closed.
        matches = [json.loads(item) for item in literals if item.startswith('"' + prefix)]
        if not matches or len(set(matches)) != 1:
            raise RuntimeError(f"SQL extraction ambiguous/missing: {label}")
        queries[label] = matches[0].replace("{REVISION_COLUMNS}", columns).replace("{lock}", "SHARE")
    return queries


def run(command, env=None, stdin=None, timeout=180):
    result = subprocess.run(command, cwd=ROOT, env=env, input=stdin, text=True,
                            capture_output=True, timeout=timeout)
    if result.returncode:
        raise RuntimeError(f"{command[0]} failed: {result.stdout}\n{result.stderr}")
    return result.stdout.strip()


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def cpu_seconds(value):
    fields = value.split(":")
    return sum(float(field) * 60 ** i for i, field in enumerate(reversed(fields)))


class Resources:
    """Sample only owned host/PG process trees. Summed RSS double-counts shared PG pages."""
    def __init__(self, host_pid, pg_pid):
        self.roots = {"axum": host_pid, "postgres": pg_pid}
        self.samples = []
        self.error = None
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self.collect)

    def collect(self):
        try:
            self.sample_until_stopped()
        except Exception as error:
            self.error = error

    def sample_until_stopped(self):
        while not self.stop.is_set():
            rows = {}
            for line in run(["ps", "-axo", "pid,ppid,time,rss"]).splitlines()[1:]:
                pid, parent, cpu, rss = line.split()
                rows[int(pid)] = (int(parent), cpu_seconds(cpu), int(rss))
            sample = {"time": time.monotonic()}
            for label, root in self.roots.items():
                members = {root}
                while True:
                    expanded = members | {pid for pid, row in rows.items() if row[0] in members}
                    if expanded == members:
                        break
                    members = expanded
                sample[label] = {str(pid): rows[pid][1:] for pid in members if pid in rows}
            self.samples.append(sample)
            self.stop.wait(0.1)

    def __enter__(self):
        self.client_start = resource.getrusage(resource.RUSAGE_SELF)
        self.thread.start()
        return self

    def __exit__(self, *_):
        self.stop.set()
        self.thread.join()
        if self.error is not None:
            raise self.error

    def summary(self):
        result = {}
        for label in self.roots:
            previous = {}
            cpu = 0.0
            for sample in self.samples:
                for pid, (value, _) in sample[label].items():
                    if pid in previous:
                        cpu += max(0, value - previous[pid])
                    previous[pid] = value
            result[label] = {"cpu_seconds_sampled": cpu,
                             "peak_summed_rss_kib": max((sum(row[1] for row in s[label].values())
                                                        for s in self.samples), default=0)}
        usage = resource.getrusage(resource.RUSAGE_SELF)
        result["python_client"] = {"cpu_seconds": usage.ru_utime + usage.ru_stime
                                  - self.client_start.ru_utime - self.client_start.ru_stime,
                                  "process_peak_rss_bytes_on_macos": usage.ru_maxrss}
        result["resource_samples"] = len(self.samples)
        return result


class Benchmark:
    def __init__(self, output, args):
        self.output, self.args = output, args
        self.env = dict(os.environ)
        for key in list(self.env):
            if key.startswith("PG") or key in ("DATABASE_URL", "CANTOS_TEST_CLUSTER_URL"):
                self.env.pop(key)
        self.pg_port, self.http_port = free_port(), free_port()
        self.admin_url = f"postgresql://cantos_bench_admin@127.0.0.1:{self.pg_port}/postgres"
        self.snapshot_url = self.admin_url
        self.app_url = self.admin_url.replace("cantos_bench_admin", "cantos_app")
        self.data = output / "pgdata"
        self.host = None
        self.host_log = None
        self.started = False
        self.queries = source_queries()
        self.results = []

    def sql(self, query, app=False):
        return run(["psql", "-X", self.app_url if app else self.admin_url, "-Atq",
                    "-v", "ON_ERROR_STOP=1", "-c", query], env=self.env)

    def start(self):
        run(["initdb", "-D", str(self.data), "--username=cantos_bench_admin", "--auth=trust",
             "--no-locale", "--encoding=UTF8"], env=self.env)
        # Durability stays on. Bound memory/connections; local workload targets, not production tuning.
        with (self.data / "postgresql.conf").open("a") as config:
            config.write("\nshared_buffers='128MB'\nwork_mem='4MB'\nmax_connections=32\n"
                         "track_io_timing=on\njit=off\n")
        run(["pg_ctl", "-D", str(self.data), "-l", str(self.output / "postgres.log"),
             "-o", f"-h 127.0.0.1 -p {self.pg_port} -k /tmp", "-w", "start"], env=self.env)
        self.started = True
        self.sql("CREATE ROLE cantos_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE")
        run([str(ROOT / "target/debug/cantos-migrate")], env=dict(self.env, DATABASE_URL=self.admin_url))
        self.sql("GRANT USAGE ON SCHEMA public TO cantos_app; "
                 f"GRANT SELECT ON {','.join(TABLES)} TO cantos_app; "
                 "GRANT INSERT ON scripts,script_revisions,revision_evidence TO cantos_app; "
                 "GRANT UPDATE(head_revision) ON scripts TO cantos_app; "
                 "GRANT UPDATE(revoked) ON sessions TO cantos_app")

    def start_host(self):
        self.host_log = (self.output / "host.log").open("a")
        self.sql("GRANT SELECT ON cantos_schema,cantos_migrations TO cantos_app")
        self.host = subprocess.Popen([str(ROOT / "target/debug/cantos-server")], cwd=ROOT,
                                     env=dict(self.env, DATABASE_URL=self.app_url, CANTOS_ENV="development",
                                              CANTOS_BIND_ADDRESS=f"127.0.0.1:{self.http_port}",
                                              CANTOS_WEB_ORIGIN=f"http://127.0.0.1:{self.http_port}",
                                              CANTOS_WEB_DIST=str(self.output / "no-static-assets")),
                                     stdout=self.host_log, stderr=self.host_log)
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if self.host.poll() is not None:
                raise RuntimeError("host exited before readiness")
            try:
                with socket.create_connection(("127.0.0.1", self.http_port), timeout=0.2):
                    return
            except OSError:
                time.sleep(0.02)
        raise RuntimeError("host readiness timed out")

    def stop_host(self):
        if self.host is not None:
            self.host.terminate()
            try:
                self.host.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.host.kill()
                self.host.wait(timeout=5)
            self.host = None
        if self.host_log is not None:
            self.host_log.close()
            self.host_log = None

    def close(self):
        self.stop_host()
        if self.started:
            run(["pg_ctl", "-D", str(self.data), "-m", "fast", "-w", "stop"], env=self.env)
            self.started = False

    def clone_snapshot(self, label):
        """Each comparison starts at exactly the same seeded committed rows, not grown history."""
        self.stop_host()
        self.admin_url = self.snapshot_url
        database = "cantos_bench_" + label
        if label not in ("baseline", "member_cover", "baseline_return"):
            raise ValueError("unknown benchmark phase")
        self.sql(f"CREATE DATABASE {database} TEMPLATE postgres")
        self.admin_url = self.snapshot_url.rsplit("/", 1)[0] + "/" + database
        self.app_url = self.admin_url.replace("cantos_bench_admin", "cantos_app")
        return json.loads(self.sql("SELECT json_build_object('scripts',(SELECT count(*) FROM scripts),"
                                   "'revisions',(SELECT count(*) FROM script_revisions),"
                                   "'members',(SELECT count(*) FROM script_members),"
                                   "'evidence_links',(SELECT count(*) FROM revision_evidence))"))

    def seed(self, count):
        started = time.monotonic()
        self.count = count
        self.exports = []
        self.digests = []
        for name, size in [("small", 4), ("payload", 512)]:
            path = self.output / f"{name}.json"
            path.write_text(json.dumps(make_document(size), ensure_ascii=False))
            command = [str(ROOT / "target/debug/validate-script"), str(path)]
            export = run(command + ["--export"], env=self.env)
            digest = run(command, env=self.env)
            self.exports.append(export)
            self.digests.append((digest, "sir-e1:sha256:" + hashlib.sha256(
                b"cantos/script-export/e1\n" + export.encode()).hexdigest()))
        actors = [f"owner{i}" for i in range(32)] + ["reader", "editor", "outsider"]
        self.sql("INSERT INTO actors(id) VALUES " + ",".join(f"({sql_literal(a)})" for a in actors))
        self.sql("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES " + ",".join(
            f"(decode('{hashlib.sha256(token(a).encode()).hexdigest()}','hex'),'{a}',"
            "CURRENT_TIMESTAMP+interval '8 hours')" for a in actors))
        self.sql("INSERT INTO script_evidence VALUES " + ",".join(
            f"('{a}','{kind}','{ref}','synthetic benchmark; no publication clearance')"
            for a in actors for kind, ref in EVIDENCE_REFS))
        # Safe bulk scale fixture: admitted complete exports, independent storage identities.
        # This is operator seeding, not evidence of API import/seed throughput.
        for table, rows in self.seed_rows(count):
            path = self.output / f"seed-{table}.csv"
            with path.open("w", newline="") as stream:
                csv.writer(stream).writerows(rows)
        copies = "\n".join(f"\\copy {table} FROM {sql_literal(self.output / ('seed-' + table + '.csv'))} CSV"
                           for table in ("scripts", "script_members", "script_revisions", "revision_evidence"))
        run(["psql", "-X", self.admin_url, "-q", "-v", "ON_ERROR_STOP=1"], env=self.env,
            stdin="BEGIN;\n" + copies + "\nCOMMIT;\n", timeout=180)
        self.sql("VACUUM ANALYZE")
        return {"scripts": count, "owners": 32, "members_per_script": 2,
                "revisions": 2000 + (count - 1) * 5, "history_skew": "script 0:2000; others:5",
                "owner_skew": "owner0:50%; remaining scripts across 31 owners",
                "export_bytes": [len(value.encode()) for value in self.exports],
                "payload_skew": "10% of scripts carry 512 dialogues; others 4",
                "seed_seconds": time.monotonic() - started}

    def seed_rows(self, count):
        yield "scripts", ((script_id(i), owner(i, count), 2000 if i == 0 else 5) for i in range(count))
        yield "script_members", ((script_id(i), actor, role) for i in range(count)
                                  for actor, role in [("reader", "reader"), ("editor", "editor")])

        def revisions():
            for i in range(count):
                export = self.exports[int(i % 10 == 9)]
                digest, export_hash = self.digests[int(i % 10 == 9)]
                for revision in range(1, (2000 if i == 0 else 5) + 1):
                    operation = str(uuid.UUID(int=(i + 1) * 10000 + revision))
                    yield (script_id(i), revision, revision - 1, operation, owner(i, count),
                           "2026-01-01T00:00:00.000000Z", "0.1.0", "\\x" + export.encode().hex(),
                           digest, export_hash)
        yield "script_revisions", revisions()
        yield "revision_evidence", ((script_id(i), revision, owner(i, count), kind, ref)
                                    for i in range(count)
                                    for revision in range(1, (2000 if i == 0 else 5) + 1)
                                    for kind, ref in EVIDENCE_REFS)

    def request(self, connection, number, actor, revision=None, body=None):
        path = f"/api/v1/studio/scripts/{script_id(number)}/"
        path += "revisions" if body is not None else ("head" if revision is None else f"revisions/{revision}")
        headers = {"Origin": f"http://127.0.0.1:{self.http_port}", "Content-Type": "application/json"}
        if actor is not None:
            headers["Cookie"] = f"cantos_session={token(actor)}"
        connection.request("POST" if body is not None else "GET", path,
                           body=None if body is None else json.dumps(body).encode(), headers=headers)
        response = connection.getresponse()
        raw = response.read()
        if response.getheader("Cache-Control") != "no-store":
            raise AssertionError("read/write response lost no-store")
        return response.status, json.loads(raw), len(raw)

    def connection(self):
        return http.client.HTTPConnection("127.0.0.1", self.http_port, timeout=16)

    def oracle(self, number, revision=None):
        script = sql_literal(script_id(number))
        revision = str(revision) if revision is not None else f"(SELECT head_revision FROM scripts WHERE id={script})"
        # Independent PG projection: no call to the application row decoder/hash validator.
        query = "SELECT json_build_object('script_id',script_id,'revision',revision," \
                "'accepted_by',accepted_by,'accepted_at',to_char(accepted_at AT TIME ZONE 'UTC'," \
                "'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"'),'content_digest',content_digest," \
                "'export_digest',export_digest,'script_json',convert_from(canonical_export,'UTF8')) " \
                f"FROM script_revisions WHERE script_id={script} AND revision={revision}"
        return json.loads(self.sql(query, app=True))

    def check_semantics(self):
        checks = []
        with closing(self.connection()) as connection:
            for i in (0, 9, self.count - 1):
                for revision in (None, 1):
                    expected = self.oracle(i, revision)
                    for actor in (owner(i, self.count), "reader", "editor"):
                        status, response, _ = self.request(connection, i, actor, revision)
                        assert status == 200 and response == expected
                    status, response, _ = self.request(connection, i, "outsider", revision)
                    assert status == 404 and response == {"code": "not_found", "current_revision": None, "issues": []}
                    checks.append(f"exact PG response and owner/reader/editor/outsider: {i}/{revision}")
            status, response, _ = self.request(connection, 0, None)
            assert status == 401 and response == {"code": "unauthenticated", "current_revision": None, "issues": []}
            assert self.request(connection, 0, "reader", revision=999999)[0:2] == (404, {"code": "not_found", "current_revision": None, "issues": []})
            self.sql(f"DELETE FROM script_members WHERE script_id='{script_id(0)}' AND actor_id='reader'")
            assert self.request(connection, 0, "reader")[0:2] == (404, {"code": "not_found", "current_revision": None, "issues": []})
            self.sql(f"INSERT INTO script_members VALUES('{script_id(0)}','reader','reader')")
            # An actor owning other scripts still cannot read a foreign owner's local entity IDs.
            assert self.request(connection, 0, owner(self.count - 1, self.count))[0:2] == (404, {"code": "not_found", "current_revision": None, "issues": []})
            head = self.oracle(0)["revision"]
            body = {"operation_id": str(uuid.UUID(int=900000000)), "expected_revision": head,
                    "script_json": self.exports[0]}
            assert self.request(connection, 0, "reader", body=body)[0:2] == (403, {"code": "forbidden", "current_revision": None, "issues": []})
            assert self.oracle(0)["revision"] == head
        checks += ["anonymous401", "missing history404", "membership revocation404",
                   "foreign owner404", "reader write403 and head unchanged"]
        return checks

    def plans(self, label):
        plans = {}
        actor = "reader"
        parameters = {
            "auth": [f"decode('{hashlib.sha256(token(actor).encode()).hexdigest()}','hex')"],
            "head_lock": [sql_literal(script_id(0))],
            "member": [sql_literal(script_id(0)), sql_literal(actor)],
            "revision": [sql_literal(script_id(0)), "1"],
            "replay": [sql_literal(script_id(0)), "'owner0'", sql_literal(str(uuid.UUID(int=10001)))],
            "evidence": ["'owner0'", "'rights'", "'rights-demo'"],
        }
        for name, params in parameters.items():
            query = self.queries[name]
            for i, value in enumerate(params, 1):
                query = query.replace(f"${i}", value)
            output = self.sql("BEGIN; EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) " + query + "; ROLLBACK", app=True)
            plans[name] = json.loads(output)
        query = self.queries["member"].replace("$1", sql_literal(script_id(0))).replace("$2", "'outsider'")
        plans["member_denied"] = json.loads(self.sql("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) " + query, app=True))
        plans["migration"] = json.loads(self.sql("EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) " + self.queries["migration"]))
        (self.output / f"plans-{label}.json").write_text(json.dumps(plans, indent=2) + "\n")
        return plans

    def storage(self):
        return json.loads(self.sql("SELECT json_object_agg(relname,bytes) FROM (SELECT relname,"
                                   "pg_total_relation_size(oid) AS bytes FROM pg_class WHERE relname IN ("
                                   + ",".join(sql_literal(t) for t in TABLES + ["bench_member_cover"]) + ")) s"))

    def workload(self, label, mode, concurrency, repetition, cold=False):
        samples = []
        # Per-worker write script eliminates unintended stale writes; GET 90% hot, 10% spread.
        heads = [self.oracle(self.count - 1 - worker)["revision"] for worker in range(concurrency)] if "mixed" in mode else []
        total = self.args.cold_samples if cold else self.args.samples
        per_worker = total // concurrency
        def worker(worker_id):
            rng = random.Random(self.args.seed + worker_id + repetition * 100)
            rows = []
            with closing(self.connection()) as connection:
                for n in range(per_worker + (0 if cold else 32)):
                    number = rng.randrange(min(10, self.count)) if rng.random() < 0.9 else rng.randrange(self.count)
                    actor = owner(number, self.count)
                    revision = None
                    body = None
                    if mode == "payload":
                        number = rng.randrange(self.count // 10) * 10 + 9
                        actor = "reader"
                    elif mode == "member":
                        actor = "reader"
                    elif mode == "denied":
                        actor = "outsider"
                    elif mode == "pinned":
                        number, actor, revision = 0, "reader", rng.randrange(1, 2001)
                    elif "mixed" in mode and n % 10 >= (9 if mode == "mixed90" else 2):
                        number = self.count - 1 - worker_id
                        actor = owner(number, self.count)
                        body = {"operation_id": str(uuid.UUID(int=(1 if mode == "mixed90" else 2) * 10 ** 15
                                                              + (repetition + 1) * 10 ** 12
                                                              + worker_id * 10 ** 8 + n + 1)),
                                "expected_revision": heads[worker_id],
                                "script_json": self.exports[int(number % 10 == 9)]}
                    started = time.perf_counter()
                    status, response, size = self.request(connection, number, actor, revision, body)
                    elapsed = (time.perf_counter() - started) * 1000
                    if mode == "denied":
                        assert status == 404 and response == {"code": "not_found", "current_revision": None, "issues": []}
                    else:
                        assert status == 200 and response["script_id"] == script_id(number)
                        assert response["script_json"] == self.exports[int(number % 10 == 9)]
                        if revision is not None:
                            assert response["revision"] == revision
                        if body is not None:
                            assert response["revision"] == heads[worker_id] + 1
                            assert response["accepted_by"] == actor
                            heads[worker_id] += 1
                    if cold or n >= 32:
                        rows.append({"ms": elapsed, "status": status, "bytes": size,
                                     "operation": "write" if body is not None else "read"})
            return rows
        pg_pid = int((self.data / "postmaster.pid").read_text().splitlines()[0])
        wal_before = self.sql("SELECT pg_current_wal_insert_lsn()")
        with Resources(self.host.pid, pg_pid) as monitor:
            started = time.perf_counter()
            with ThreadPoolExecutor(max_workers=concurrency) as pool:
                for rows in pool.map(worker, range(concurrency)):
                    samples.extend(rows)
            elapsed = time.perf_counter() - started
        record = {"label": label, "mode": mode, "concurrency": concurrency, "repetition": repetition,
                  "cache": "PG shared-buffer reset; OS cache uncontrolled; cold-start batch" if cold else "warm; 32 unmeasured operations per worker",
                  **summarize(samples, elapsed), "resources": monitor.summary(),
                  "wal_bytes_including_warmup": int(self.sql("SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn(),"
                                                             + sql_literal(wal_before) + ")"))}
        # Throughput denominator includes warmup; explicit conservative accounting, not peak QPS.
        for operation in ("read", "write"):
            selected = [row for row in samples if row["operation"] == operation]
            if selected:
                record[operation] = summarize(selected, elapsed)
        name = f"{label}-{mode}-c{concurrency}-r{repetition}"
        (self.output / f"samples-{name}.json").write_text(json.dumps(samples) + "\n")
        (self.output / f"resources-{name}.json").write_text(json.dumps(monitor.samples) + "\n")
        self.results.append(record)
        print(json.dumps({key: record[key] for key in ("label", "mode", "cache", "p50_ms", "p95_ms", "p99_ms", "requests_per_second")}), flush=True)
        return record

    def membership_write_cost(self, label):
        # Operator permission writes feed the read. Includes the candidate's INCLUDE(role) maintenance.
        script = self.sql("SELECT pg_current_wal_insert_lsn()")
        samples = []
        for n in range(12):
            started = time.perf_counter()
            self.sql(f"UPDATE script_members SET role='{'editor' if n % 2 == 0 else 'reader'}' WHERE actor_id='reader'")
            samples.append((time.perf_counter() - started) * 1000)
        wal = int(self.sql("SELECT pg_wal_lsn_diff(pg_current_wal_insert_lsn()," + sql_literal(script) + ")"))
        return {"label": label, "transactions": 12, "rows_per_transaction": self.count,
                "p50_ms": percentile(samples, 50), "p95_ms": percentile(samples, 95),
                "p99_ms": percentile(samples, 99), "raw_ms": samples, "wal_bytes": wal,
                "timing_scope": "operator SQL transaction + new psql connection/process per sample"}


def main():
    if not __debug__:
        raise RuntimeError("benchmark assertions are required; run Python without -O")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--scripts", type=int, default=5000)
    parser.add_argument("--samples", type=int, default=2400)
    parser.add_argument("--cold-samples", type=int, default=60)
    parser.add_argument("--repetitions", type=int, default=3)
    parser.add_argument("--seed", type=int, default=20261010)
    args = parser.parse_args()
    if not (100 <= args.scripts <= 10000 and 100 <= args.samples <= 20000
            and 4 <= args.cold_samples <= 1000 and 1 <= args.repetitions <= 5):
        parser.error("bounded workload: scripts100..10000, samples100..20000, cold4..1000, repetitions1..5")
    for executable in ("initdb", "pg_ctl", "psql", "cargo", "ps"):
        if not shutil.which(executable):
            raise RuntimeError(f"unavailable: {executable}")
    # Fail before leaving a cluster if resource sampling is forbidden.
    run(["ps", "-axo", "pid,ppid,time,rss"])
    if shutil.disk_usage(ROOT).free < 5 * 1024 ** 3:
        raise RuntimeError("resource budget: less than 5 GiB disk free")
    output = OUTPUT / f"run-{time.time_ns()}"
    output.mkdir(parents=True)
    run(["cargo", "build", "-p", "cantos-server", "--bins", "--locked", "--offline"])
    benchmark = Benchmark(output, args)
    report = {"source_sha": run(["git", "rev-parse", "HEAD"]),
              "dirty_paths": run(["git", "status", "--porcelain"]), "args": vars(args),
              "versions": {tool: run([tool, "--version"]) for tool in ("postgres", "cargo", "python3")},
              "platform": run(["uname", "-a"]),
              "hardware": run(["sysctl", "-n", "machdep.cpu.brand_string", "hw.memsize", "hw.logicalcpu"]),
              "query_hashes": {key: hashlib.sha256(query.encode()).hexdigest() for key, query in benchmark.queries.items()},
              "source_hashes": {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in
                                (*REVISION_READ_SOURCES, *REVISION_HTTP_SOURCES,
                                 "services/cantos-server/src/application.rs",
                                 "services/cantos-server/schema/0001_current.sql", "scripts/benchmark_revision_reads.py")},
              "candidate": CANDIDATE, "results": benchmark.results, "phases": []}
    try:
        benchmark.start()
        report["fixture"] = benchmark.seed(args.scripts)
        report["config"] = json.loads(benchmark.sql("SELECT json_object_agg(name,setting) FROM pg_settings WHERE name IN "
            "('server_version','shared_buffers','work_mem','max_connections','fsync','synchronous_commit','full_page_writes',"
            "'jit','track_io_timing','default_transaction_isolation','random_page_cost','effective_cache_size')"))
        report["catalog_indexes"] = json.loads(benchmark.sql("SELECT json_agg(indexdef ORDER BY indexname) FROM pg_indexes WHERE schemaname='public'"))
        for phase in ("baseline", "member_cover", "baseline_return"):
            if shutil.disk_usage(ROOT).free < 5 * 1024 ** 3:
                raise RuntimeError("resource budget: less than 5 GiB disk free")
            checkpoint = benchmark.clone_snapshot(phase)
            if phase in ("member_cover", "baseline_return"):
                benchmark.sql(CANDIDATE)
            if phase == "baseline_return":
                benchmark.sql("DROP INDEX bench_member_cover")
            benchmark.sql("VACUUM ANALYZE")
            benchmark.start_host()
            checks = benchmark.check_semantics()
            benchmark.plans(phase + "-before")
            before = benchmark.storage()
            for repetition in range(args.repetitions):
                # Stop host + restart PG: shared buffers reset, OS page cache is deliberately untouched.
                benchmark.stop_host()
                run(["pg_ctl", "-D", str(benchmark.data), "-l", str(output / "postgres.log"),
                     "-m", "fast", "-w", "restart"], env=benchmark.env)
                if repetition == 0:
                    benchmark.plans(phase + "-shared-buffer-cold")
                    # EXPLAIN itself warms pages; reset again before the measured cold-start batch.
                    run(["pg_ctl", "-D", str(benchmark.data), "-l", str(output / "postgres.log"),
                         "-m", "fast", "-w", "restart"], env=benchmark.env)
                benchmark.start_host()
                benchmark.workload(phase, "member", 1, repetition, cold=True)
                for mode, concurrency in [("owner", 1), ("member", 4), ("pinned", 4), ("payload", 4),
                                          ("mixed90", 4), ("mixed20", 4), ("denied", 4)]:
                    # Cloned databases can reuse operation IDs: keep parameters/writes paired across phases.
                    benchmark.workload(phase, mode, concurrency, repetition)
            cost = benchmark.membership_write_cost(phase)
            benchmark.plans(phase + "-after-permission-writes")
            checks += benchmark.check_semantics()
            report["phases"].append({"label": phase, "seed_checkpoint": checkpoint,
                                     "checks": checks, "storage_before": before,
                                     "storage_after": benchmark.storage(), "permission_write_cost": cost})
            benchmark.stop_host()
            (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    finally:
        benchmark.close()
    print(f"PASS: synthetic API/PG oracle benchmark; raw evidence: {output}", flush=True)


if __name__ == "__main__":
    main()
