"""Ownership of explicit disposable PostgreSQL resources; never verification acceptance.

The runner writes this record only for a freshly started --keep cluster. Consumers still
confirm its exact live data_directory before use. No application/fixture URL is represented.
"""
import json
import os
import re
import subprocess
from pathlib import Path

OWNERSHIP_RECORD = "target/revision-evidence/retained-cluster.json"
SCOPES = ("full", "schema", "theatre-firebase")


def process_identity(pid):
    try:
        # stat's comm may contain spaces or parentheses. Fields after the last ')' start at3.
        fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        return fields[19]  # starttime, field22
    except (OSError, IndexError):
        return None


def runner_alive(record):
    pid = record.get("runner_pid")
    if not isinstance(pid, int) or isinstance(pid, bool) or pid <= 0:
        raise ValueError("retained ownership has no valid runner PID")
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return False
    except PermissionError:
        return True
    identity = record.get("runner_identity")
    return identity is None or process_identity(pid) == identity


def validate_ownership(record, evidence):
    if (not isinstance(record, dict) or type(record.get("format")) is not int or record.get("format") != 1
            or record.get("purpose") != "retained-disposable-cluster"
            or record.get("retained") is not True or record.get("scope") not in SCOPES
            or record.get("gate_status") not in ("PASS", "FAIL", "NOT_RUN")
            or record.get("full_gate_status") not in ("PASS", "FAIL", "NOT_RUN")):
        raise ValueError("invalid retained ownership record")
    if ((record["scope"] == "full" and record["full_gate_status"] != record["gate_status"])
            or (record["scope"] != "full" and record["full_gate_status"] != "NOT_RUN")):
        raise ValueError("retained ownership verification outcomes are inconsistent")
    if record.get("runner_active") is not False or runner_alive(record):
        raise ValueError("retained cluster runner is still active")
    port = record.get("port")
    if not isinstance(port, int) or isinstance(port, bool) or not 1024 <= port <= 65535:
        raise ValueError("invalid retained ownership port")
    raw = record.get("cluster")
    if not isinstance(raw, str) or not Path(raw).is_absolute():
        raise ValueError("retained ownership requires an absolute cluster directory")
    cluster = Path(raw)
    scope = record["scope"]
    parent = evidence.resolve() if scope == "full" else evidence.resolve() / scope
    if (cluster != cluster.resolve() or cluster.parent != parent
            or not re.fullmatch(r"cluster-[0-9]+", cluster.name)):
        raise ValueError("retained ownership scope/directory mismatch")
    data = cluster / "pgdata"
    if not (data / "PG_VERSION").is_file() or data.is_symlink():
        raise ValueError("retained cluster data directory is missing or replaced")
    if hasattr(os, "getuid") and (cluster.stat().st_uid != os.getuid() or data.stat().st_uid != os.getuid()):
        raise ValueError("retained cluster belongs to another operating-system owner")
    return port, data


def confirm_running(record, evidence, env):
    port, data = validate_ownership(record, evidence)
    # libpq environment settings must never redirect this fixed loopback connection.
    isolated = {key: value for key, value in env.items()
                if not key.startswith(("PG", "CANTOS_")) and key != "DATABASE_URL"}
    command = ["psql", "-X", f"postgresql://cantos_test_admin@127.0.0.1:{port}/postgres",
               "-v", "ON_ERROR_STOP=1", "-At", "-c", "SHOW data_directory"]
    try:
        result = subprocess.run(command, env=isolated, text=True, capture_output=True, timeout=10)
    except (OSError, subprocess.TimeoutExpired):
        raise ValueError("retained cluster is stopped or unreachable") from None
    if result.returncode:
        raise ValueError("retained cluster is stopped or unreachable")
    if Path(result.stdout.strip()).resolve() != data.resolve():
        raise ValueError("retained cluster port is served by another data directory")


def write_ownership(path, cluster, port, scope, active, status):
    """Called only by the process that initialized/started/bootstraped this exact cluster."""
    if scope not in SCOPES or status not in ("PASS", "FAIL", "NOT_RUN"):
        raise ValueError("invalid retained ownership outcome")
    value = {"format": 1, "purpose": "retained-disposable-cluster", "scope": scope,
             "cluster": str(cluster.resolve()), "port": port, "retained": True,
             "runner_pid": os.getpid(), "runner_identity": process_identity(os.getpid()),
             "runner_active": active, "gate_status": status,
             "full_gate_status": status if scope == "full" else "NOT_RUN"}
    temporary = path.with_suffix(".pending")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "w") as stream:
        json.dump(value, stream, indent=2)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    return value
