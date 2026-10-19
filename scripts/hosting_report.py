#!/usr/bin/env python3
"""Offline issue-280 evidence reporting primitive, invoked through cargo xtask.

Records are operator declarations, not provider receipts. This module neither follows evidence
references nor verifies their truth, submits requests, retries writes, or grants authority.
Only schema-controlled labels/digests reach stdout; raw fields and parse errors never do.
"""

import argparse
import json
import os
import re
import stat
from pathlib import Path
from urllib.parse import urlsplit


MAX_BYTES = 262144
STATUSES = frozenset({"PASS", "FAIL", "BLOCKED", "NOT_RUN", "UNKNOWN"})
KINDS = frozenset({
    "documented", "statically-checked", "example-tested", "mock-integration-tested",
    "compiled", "integration-tested", "interaction-tested", "production-observed",
})
OBSERVED = frozenset({"integration-tested", "interaction-tested", "production-observed"})
GLOBAL_CHECKS = (
    "provider_decision", "operator_scope", "inventory", "qualification", "circleci_build",
    "api_compatibility", "dns_recovery", "operator_acceptance", "retention_window",
    "retirement_dependencies", "hosting_cleanup",
)
COMPONENT_CHECKS = (
    "artifact", "cloudflare_publish", "preview_http", "preview_browser", "final_domain",
    "final_http", "final_browser", "release_rollback",
)
COMPONENTS = {"theatre-web": "https://cantos.cc", "studio-web": "https://studio.cantos.cc"}
SOURCE_BOUND = frozenset({
    "qualification", "circleci_build", "api_compatibility", "operator_acceptance",
    *COMPONENT_CHECKS,
})
LIVE_CHECKS = frozenset({
    "inventory", "circleci_build", "api_compatibility", "cloudflare_publish", "preview_http",
    "final_domain", "final_http", "release_rollback", "hosting_cleanup",
})
COMPONENT_FIELDS = {
    "artifact_sha256", "api_origin", "deployment_id", "deployment_environment",
    "deployment_origin", "final_origin", "rollback_deployment_id", "rollback_environment",
    "checks",
}


class RecordError(ValueError):
    """A closed, safe reason code; never wrap an exception containing raw input."""


def _require(condition, code):
    if not condition:
        raise RecordError(code)


def _keys(value, expected):
    _require(type(value) is dict and set(value) == set(expected), "FIELDS")


def _hex(value, size, nullable=True):
    if value is None and nullable:
        return
    _require(type(value) is str and re.fullmatch(r"[0-9a-f]{%d}" % size, value), "DIGEST")


def _identity(value):
    _require(value is None or (type(value) is str and
             re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]{0,127}", value)), "IDENTITY")


def _origin(value):
    if value is None:
        return
    _require(type(value) is str and len(value) <= 253, "ORIGIN")
    try:
        parsed = urlsplit(value)
        host = parsed.hostname
        valid = (parsed.scheme == "https" and host and parsed.netloc == host and
                 not parsed.path and not parsed.query and not parsed.fragment and
                 "." in host and all(re.fullmatch(r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?", part)
                                     for part in host.split(".")))
        _require(valid, "ORIGIN")
    except ValueError:
        raise RecordError("ORIGIN") from None


def _row(row, name, source):
    _keys(row, {"status", "kind", "source_sha", "evidence_sha256"})
    _require(type(row["status"]) is str and row["status"] in STATUSES, "STATUS")
    _require(row["kind"] is None or (type(row["kind"]) is str and row["kind"] in KINDS), "KIND")
    _hex(row["source_sha"], 40)
    evidence = row["evidence_sha256"]
    _require(type(evidence) is list and len(evidence) <= 16, "EVIDENCE")
    for digest in evidence:
        _hex(digest, 64, nullable=False)
    if row["status"] == "PASS":
        _require(row["kind"] is not None and evidence, "PASS_WITHOUT_EVIDENCE")
        if name in SOURCE_BOUND:
            _require(source is not None and row["source_sha"] == source, "SOURCE_MISMATCH")
        if name in LIVE_CHECKS:
            _require(row["kind"] in OBSERVED, "LIVE_EVIDENCE_REQUIRED")
        if name in {"preview_browser", "final_browser"}:
            _require(row["kind"] == "interaction-tested", "BROWSER_EVIDENCE_REQUIRED")


def validate(record):
    """Validate metadata atomically; no file, clock, network or environment access."""
    _keys(record, {"schema_version", "release_sha", "config_sha", "approved_api_origin",
                   "circleci", "checks", "components"})
    _require(type(record["schema_version"]) is int and record["schema_version"] == 1, "VERSION")
    for key in ("release_sha", "config_sha"):
        _hex(record[key], 40)
    _origin(record["approved_api_origin"])
    _keys(record["circleci"], {"pipeline_id", "workflow_id", "job_number"})
    for value in record["circleci"].values():
        _identity(value)
    _keys(record["checks"], GLOBAL_CHECKS)
    for name, row in record["checks"].items():
        _row(row, name, record["release_sha"])
    if record["checks"]["circleci_build"]["status"] == "PASS":
        _require(record["config_sha"] and all(record["circleci"].values()), "CI_IDENTITY_REQUIRED")
    _keys(record["components"], COMPONENTS)
    for name, component in record["components"].items():
        _keys(component, COMPONENT_FIELDS)
        _hex(component["artifact_sha256"], 64)
        for key in ("api_origin", "deployment_origin", "final_origin"):
            _origin(component[key])
        _require(component["final_origin"] in (None, COMPONENTS[name]), "SURFACE_MISMATCH")
        for key in ("deployment_id", "rollback_deployment_id"):
            _identity(component[key])
        for key in ("deployment_environment", "rollback_environment"):
            _require(component[key] in (None, "preview", "production"), "ENVIRONMENT")
        _keys(component["checks"], COMPONENT_CHECKS)
        for check, row in component["checks"].items():
            _row(row, check, record["release_sha"])
            if row["status"] != "PASS":
                continue
            _require(record["config_sha"] and component["artifact_sha256"] and
                     component["api_origin"] and
                     component["api_origin"] == record["approved_api_origin"], "ARTIFACT_IDENTITY_REQUIRED")
            if check != "artifact":
                _require(component["deployment_id"] and component["deployment_origin"] and
                         component["deployment_environment"], "DEPLOYMENT_IDENTITY_REQUIRED")
            if check in {"final_domain", "final_http", "final_browser"}:
                _require(component["final_origin"] == COMPONENTS[name] and
                         component["deployment_environment"] == "production", "FINAL_DOMAIN_REQUIRED")
            if check == "release_rollback":
                _require(component["rollback_deployment_id"] and
                         component["rollback_deployment_id"] != component["deployment_id"] and
                         component["rollback_environment"] == "production", "ROLLBACK_TARGET_REQUIRED")
    return record


def render(record):
    """Only fixed labels, validated enums and SHA values enter the report."""
    validate(record)
    lines = ["Operator-declared evidence only; references and live state were NOT verified.",
             "This report authorizes no operation or retirement.",
             "Release SHA: " + (record["release_sha"] or "UNKNOWN"),
             "Config SHA: " + (record["config_sha"] or "UNKNOWN")]
    rows = [("global", name, record["checks"][name]) for name in GLOBAL_CHECKS]
    for component in COMPONENTS:
        rows.extend((component, name, record["components"][component]["checks"][name])
                    for name in COMPONENT_CHECKS)
    for scope, name, row in rows:
        lines.append(f"DECLARED {row['status']:7} {scope}/{name} [{row['kind'] or 'no-evidence'}]")
    pending = sum(row["status"] != "PASS" for _, _, row in rows)
    lines.append(f"{pending} of {len(rows)} rows lack a declared PASS; operator review remains required.")
    if any(row["status"] == "UNKNOWN" for _, _, row in rows):
        lines.append("Unknown outcome: reconcile the exact remote operation read-only before any retry.")
    return "\n".join(lines)


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        _require(key not in result, "DUPLICATE_FIELD")
        result[key] = value
    return result


def _invalid_constant(_value):
    raise RecordError("JSON")


def load(path):
    """One bounded regular local file; do not follow a final symlink or evidence references."""
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
        try:
            _require(stat.S_ISREG(os.fstat(descriptor).st_mode), "REGULAR_FILE_REQUIRED")
            with os.fdopen(descriptor, "rb", closefd=False) as source:
                data = source.read(MAX_BYTES + 1)
        finally:
            os.close(descriptor)
    except OSError:
        raise RecordError("READ_FAILED") from None
    _require(len(data) <= MAX_BYTES, "TOO_LARGE")
    try:
        record = json.loads(data, object_pairs_hook=_pairs, parse_constant=_invalid_constant)
    except RecordError:
        raise
    except (UnicodeError, ValueError, RecursionError):
        raise RecordError("JSON") from None
    return validate(record)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--record", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        print(render(load(args.record)))
    except RecordError as error:
        print(f"hosting-report: INVALID_RECORD ({error})")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
