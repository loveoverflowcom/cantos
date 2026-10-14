#!/usr/bin/env python3
"""Run revision, import, adaptation and production tests in a new disposable cluster.

Never reads DATABASE_URL or connects to an existing database. --keep retains this
test cluster for local Studio inspection; its explicit stop command is printed.
"""
import argparse
import hashlib
import io
import json
import os
import re
from urllib.parse import urlsplit
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request
import uuid
import zipfile

from production_oracle import production_after_restart, production_before_restart
import reference_msgpack
import reference_protobuf

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "target" / "revision-evidence"
ADAPTATION_EVIDENCE = ROOT / "target" / "adaptation-evidence"
EDITOR_EVIDENCE = ROOT / "target" / "studio-editor-evidence"
CATALOG_EVIDENCE = ROOT / "target" / "catalog-evidence"
ADAPTATION_TOOL = ROOT / "scripts" / "cantos_adaptation_tool.py"


def adaptation_tool(env, port, token, command, payload, expected_status):
    """Exercise the real CLI without putting an authentication token in argv or evidence."""
    command_line = [sys.executable, str(ADAPTATION_TOOL), "--base-url",
                    f"http://127.0.0.1:{port}", command]
    result = subprocess.run(command_line, cwd=ROOT,
                            env=dict(env, CANTOS_SESSION_TOKEN=token),
                            input=json.dumps(payload, ensure_ascii=False), text=True,
                            capture_output=True, timeout=15)
    if token in result.stdout or token in result.stderr:
        raise RuntimeError("adaptation tool exposed its session token; output suppressed")
    try:
        envelope = json.loads(result.stdout)
    except (ValueError, TypeError):
        raise RuntimeError("adaptation tool did not return one JSON envelope") from None
    expected_ok = 200 <= expected_status < 300
    if (not isinstance(envelope, dict) or set(envelope) != {"ok", "http_status", "result"}
            or envelope["ok"] is not expected_ok
            or envelope["http_status"] != expected_status
            or not isinstance(envelope["result"], dict)
            or result.returncode != (0 if expected_ok else 1)):
        raise RuntimeError("adaptation tool returned an unexpected status or envelope")
    return envelope


def adaptation_http(port, token, path, payload, expected_status=200):
    """Authenticated review acceptance and raw malformed-body probes on this test host."""
    body = payload if isinstance(payload, bytes) else json.dumps(payload).encode()
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/api/v1/studio/adaptations{path}", data=body, method="POST",
        headers={"Cookie": f"cantos_session={token}",
                 "Origin": f"http://127.0.0.1:{port}", "Content-Type": "application/json"})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        response = opener.open(request, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        assert response.status == expected_status
        assert response.headers["Cache-Control"] == "no-store"
        assert response.headers["X-Content-Type-Options"] == "nosniff"
        return json.load(response)


def run(command, env=None, output=None):
    result = subprocess.run(command, cwd=ROOT, env=env, text=True, capture_output=True, timeout=180)
    if output:
        output.write_text(result.stdout + result.stderr)
    if result.returncode:
        print(result.stdout + result.stderr)
        raise RuntimeError(f"FAIL: {command[0]} exited {result.returncode}")
    print(result.stdout.strip())
    return result.stdout


def free_port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def reap_completed_databases(env, cluster_url, retained):
    """After a suite exits, discard only its completed DBs in this runner's fresh cluster.

    Main constructs cluster_url after initdb; no inherited URL is used. Keep the exact
    restart fixtures until socket/restart assertions finish. Never force-close connections.
    """
    parsed = urlsplit(cluster_url)
    if (parsed.scheme != "postgresql" or parsed.hostname != "127.0.0.1"
            or parsed.username != "cantos_test_admin" or parsed.password is not None
            or parsed.path != "/postgres" or parsed.port is None or parsed.query or parsed.fragment):
        raise ValueError("completed cleanup requires this disposable cluster's admin URL")
    command = ["psql", "-X", cluster_url, "-v", "ON_ERROR_STOP=1", "-At", "-c"]
    result = subprocess.run(command + ["SELECT datname FROM pg_database"], env=env,
                            text=True, capture_output=True, check=True, timeout=10)
    candidates = [name for name in result.stdout.splitlines()
                  if name.startswith("cantos_test_") and name not in retained]
    if any(not re.fullmatch(r"cantos_test_[a-z0-9_]+", name) for name in candidates):
        raise ValueError("unsafe disposable database name")
    for name in candidates:
        subprocess.run(command + [f'DROP DATABASE "{name}"'], env=env, text=True,
                       capture_output=True, check=True, timeout=10)
    print(f"Released {len(candidates)} completed disposable test databases", flush=True)


def retained_restart_databases():
    """Names only: credentials and source bodies never enter cleanup output."""
    paths = (EVIDENCE / "restart.json", EDITOR_EVIDENCE / "editor-api-journey.json",
             CATALOG_EVIDENCE / "restart.json", CATALOG_EVIDENCE / "http-restart.json",
             CATALOG_EVIDENCE / "audio-restart.json")
    return {json.loads(path.read_text())["database"] for path in paths if path.exists()}


def isolated_environment():
    # libpq hostaddr/service/options can override an explicit local URL. Host configuration
    # must also belong to this run rather than exposing an inherited static directory.
    return {key: value for key, value in os.environ.items()
            if not key.startswith(("PG", "CANTOS_")) and key != "DATABASE_URL"}


def http(port, fixture, method="GET"):
    path = "head" if method == "GET" else "revisions"
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/api/v1/studio/scripts/{fixture['script']}/{path}",
        data=None if method == "GET" else json.dumps(fixture["request"]).encode(),
        method=method,
        headers={"Cookie": f"cantos_session={fixture['token']}",
                 "Origin": f"http://127.0.0.1:{port}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=3) as response:
        assert response.headers["Cache-Control"] == "no-store"
        return json.load(response)


def msgpack_http(port, fixture, method="GET", accept="application/msgpack"):
    """The negotiated revision routes over the real socket, decoded by the independent oracle."""
    path = "head" if method == "GET" else "revisions"
    headers = {"Cookie": f"cantos_session={fixture['token']}",
               "Origin": f"http://127.0.0.1:{port}", "Accept": accept}
    body = None
    if method != "GET":
        body = reference_msgpack.encode(fixture["request"])
        headers["Content-Type"] = "application/msgpack"
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/api/v1/studio/scripts/{fixture['script']}/{path}",
        data=body, method=method, headers=headers)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        response = opener.open(request, timeout=3)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        assert response.headers["Cache-Control"] == "no-store"
        assert response.headers["X-Content-Type-Options"] == "nosniff"
        if response.status != 200:
            assert response.headers["Content-Type"] == "application/json"
            return response.status, json.load(response)
        assert response.headers["Content-Type"] == "application/msgpack"
        assert response.headers["Vary"] == "accept"
        return response.status, reference_msgpack.decode(response.read())


def protobuf_http(port, fixture, method="GET", accept="application/protobuf"):
    """Real socket probe: independent encode-only oracle checks exact request/success bytes."""
    path = "head" if method == "GET" else "revisions"
    headers = {"Cookie": f"cantos_session={fixture['token']}",
               "Origin": f"http://127.0.0.1:{port}", "Accept": accept}
    body = None
    if method != "GET":
        body = reference_protobuf.encode("SaveRevisionRequest", fixture["request"])
        headers["Content-Type"] = "application/protobuf"
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/api/v1/studio/scripts/{fixture['script']}/{path}",
        data=body, method=method, headers=headers)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        response = opener.open(request, timeout=3)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        assert response.headers["Cache-Control"] == "no-store"
        assert response.headers["X-Content-Type-Options"] == "nosniff"
        if response.status != 200:
            assert response.headers["Content-Type"] == "application/json"
            return response.status, json.load(response)
        assert response.headers["Content-Type"] == "application/protobuf"
        assert response.headers["Vary"] == "accept"
        expected = reference_protobuf.encode("RevisionResponse", fixture["response"])
        actual = response.read()
        assert actual == expected, "Protobuf revision differs from independent wire oracle"
        return response.status, actual


def import_http(port, token, source=None, request=None, original=False):
    path = "/imports" if source is None else f"/imports/{source}"
    if original:
        path += "/original"
    headers = {"Cookie": f"cantos_session={token}", "Origin": f"http://127.0.0.1:{port}",
               "Content-Type": "application/json"}
    call = urllib.request.Request(f"http://127.0.0.1:{port}/api/v1/studio{path}",
                                  data=None if request is None else json.dumps(request).encode(),
                                  headers=headers, method="GET" if request is None else "POST")
    with urllib.request.urlopen(call, timeout=5) as response:
        assert response.headers["Cache-Control"] == "no-store"
        assert response.headers["X-Content-Type-Options"] == "nosniff"
        return response.read() if original else json.load(response)


def import_restart_fixtures():
    """Original synthetic bytes; DOCX expected text is independent of the Rust extractor."""
    docx = io.BytesIO()
    with zipfile.ZipFile(docx, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("[Content_Types].xml", '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
        archive.writestr("word/document.xml", '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Mai: Ngày mai, mình có diễn tiếp không?</w:t></w:r></w:p><w:p><w:r><w:t>Nam: Có, ở Vọng Đài.</w:t></w:r></w:p></w:body></w:document>')
    fixtures = []
    for name, fmt, raw in [("vietnamese.txt", "txt", b"\xef\xbb\xbf" + "Mai: Nga\u0300y mai, mình có diễn tiếp không?\r\n\r\n— Chưa rõ người nói.\r\n".encode()),
                           ("vietnamese.docx", "docx", docx.getvalue()),
                           ("invalid.txt", "txt", b"\xff\x00")]:
        fixtures.append({"metadata": {"operation_id": str(uuid.uuid4()), "file_name": name,
                                      "format": fmt, "reference": "original synthetic recovery fixture",
                                      "rights_holder": None, "permission_evidence": None, "usage_scope": None},
                         "original_bytes": list(raw)})
    return fixtures


def adaptation_snapshot(env, app_url):
    """Independent application-role row counts, outside the HTTP response implementation."""
    query = """SELECT json_build_object(
        'caller_runs',(SELECT count(*) FROM adaptation_runs WHERE input_version='c1'),
        'attempts',(SELECT count(*) FROM adaptation_attempts),
        'submissions',(SELECT count(*) FROM adaptation_submissions),
        'proposals',(SELECT count(*) FROM adaptation_proposals),
        'acceptances',(SELECT count(*) FROM adaptation_acceptances),
        'revisions',(SELECT count(*) FROM script_revisions),
        'evidence',(SELECT count(*) FROM script_evidence))::text;"""
    result = subprocess.run(["psql", "-X", "-A", "-t", app_url, "-v", "ON_ERROR_STOP=1",
                             "-c", query], cwd=ROOT, env=env, text=True,
                            capture_output=True, timeout=10)
    if result.returncode:
        raise RuntimeError("adaptation row-count oracle failed")
    snapshot = json.loads(result.stdout)
    assert snapshot["attempts"] == 0
    return snapshot


def assert_adaptation_delta(before, after, **changes):
    assert before.keys() == after.keys()
    for name in before:
        assert after[name] == before[name] + changes.get(name, 0), (name, before, after)


def adaptation_before_restart(env, port, token, app_url):
    """Synthetic caller-created output crosses the actual CLI, socket, host and PostgreSQL."""
    original = (ROOT / "packages/contracts/fixtures/adaptation/source-vi.txt").read_bytes()
    import_request = {
        "metadata": {"operation_id": str(uuid.uuid4()), "file_name": "adaptation-source-vi.txt",
                     "format": "txt", "reference": "original synthetic CLI recovery fixture",
                     "rights_holder": "Synthetic fixture author",
                     "permission_evidence": "Repository-authored fixture; no publication clearance",
                     "usage_scope": "Disposable local integration checks only"},
        "original_bytes": list(original)}
    source = import_http(port, token, request=import_request)
    assert source["sha256"] == hashlib.sha256(original).hexdigest()
    assert source["outcome"]["status"] == "parsed"
    assert import_http(port, token, source=source["id"], original=True) == original
    initial = adaptation_snapshot(env, app_url)
    invalid_body = {"code": "invalid_request", "current_revision": None, "issues": []}
    assert adaptation_http(port, token, "/contexts", b"{", 400) == invalid_body
    assert adaptation_http(port, token, "/contexts", b" " * (64 * 1024 + 1), 413) == invalid_body
    assert adaptation_snapshot(env, app_url) == initial

    context_request = {
        "operation_id": str(uuid.uuid4()), "source_id": source["id"],
        "source_sha256": source["sha256"],
        "extractor_version": source["outcome"]["extraction"]["extractor_version"],
        "script_id": str(uuid.uuid4()), "expected_revision": 0, "rights_authorization": True}
    context_envelope = adaptation_tool(env, port, token, "context", context_request, 201)
    context = context_envelope["result"]
    assert context["request"] == context_request
    assert context["source"] == source
    assert context["context_version"] == "c1"
    assert context["contract_version"] == "cantos-adaptation-1"
    assert context["status"] == "awaiting_proposal"
    assert context["input_revision"] is None
    assert context["proposal"] is None
    assert context["latest_submission"] is None
    assert context["accepted_revision"] is None
    assert json.loads(context["proposal_schema_json"]) == json.loads(
        (ROOT / "packages/contracts/schema/adaptation/cantos-adaptation-1.schema.json").read_text())
    prompt_source = json.loads(context["user_prompt"])["source"]
    assert prompt_source["id"] == source["id"]
    assert prompt_source["sha256"] == hashlib.sha256(original).hexdigest()
    assert prompt_source["blocks"] == source["outcome"]["extraction"]["blocks"]
    created = adaptation_snapshot(env, app_url)
    assert_adaptation_delta(initial, created, caller_runs=1, evidence=2)

    generation = {"host_tool": "synthetic-cli-fixture-no-inference", "provider": None,
                  "model": None, "configuration_json": None,
                  "prompt_version": context["prompt_version"], "usage": None, "cost": None}
    invalid_submission = {"run_id": context["id"], "submission": {
        "operation_id": str(uuid.uuid4()), "context_digest": context["context_digest"],
        "proposal_json": "{", "generation": generation}}
    invalid_envelope = adaptation_tool(env, port, token, "submit", invalid_submission, 200)
    invalid = invalid_envelope["result"]
    assert invalid["status"] == "invalid"
    assert invalid["proposal"] is None
    assert invalid["problem"] == {"code": "invalid_output", "issues": [
        {"path": "/", "rule": "json_contract"}]}
    assert invalid["output_sha256"] == hashlib.sha256(b"{").hexdigest()
    assert invalid["generation"] == generation
    invalid_context = adaptation_tool(env, port, token, "context-read", {"run_id": context["id"]}, 200)
    assert invalid_context["result"]["status"] == "awaiting_proposal"
    assert invalid_context["result"]["latest_submission"] == invalid
    assert_adaptation_delta(created, adaptation_snapshot(env, app_url), submissions=1)

    # This is repository-authored proposal data, never a synthetic service labelled as AI.
    document = json.loads((ROOT / "packages/contracts/fixtures/adaptation/proposal-vi.json").read_text())
    document["scenes"][0]["cues"].extend([
        {"kind": "music", "description": "Nhạc nền là đề xuất cần tác giả duyệt.",
         "line_index": 1, "edge": "start", "source_blocks": [1]},
        {"kind": "sfx", "description": "Âm thanh cần xác minh trước sản xuất.",
         "line_index": 2, "edge": "end", "source_blocks": [2]}])
    assert document["scenes"][0]["lines"][2]["speaker"] is None
    output = json.dumps(document, ensure_ascii=False)
    valid_submission = {"run_id": context["id"], "submission": {
        "operation_id": str(uuid.uuid4()), "context_digest": context["context_digest"],
        "proposal_json": output, "generation": generation}}
    valid_envelope = adaptation_tool(env, port, token, "submit", valid_submission, 200)
    valid = valid_envelope["result"]
    assert valid["status"] == "valid"
    assert valid["problem"] is None
    assert valid["generation"] == generation
    assert valid["output_sha256"] == hashlib.sha256(output.encode()).hexdigest()
    proposal = valid["proposal"]
    assert any(finding["code"] == "unresolved_speaker" for finding in proposal["findings"])
    proposed_script = json.loads(proposal["script_json"])
    scene = proposed_script["episode"]["acts"][0]["scenes"][0]
    unknown_speaker = next(character for character in proposed_script["characters"]
                           if character["id"] == scene["dialogues"][2]["speaker_id"])
    assert unknown_speaker["name"] == "Người nói chưa xác định"
    assert unknown_speaker["role"] == "character"
    assert unknown_speaker["personality"] == "Attribution unresolved; creator confirmation required."
    assert [line["text"] for line in scene["dialogues"]] == [
        "Đêm xuống bên bến sông.", "Tôi sẽ chờ ở đây.", "Có ai nghe thấy không?"]
    assert {cue["kind"] for cue in scene["sound_cues"]} == {"ambience", "music", "sfx"}
    review_envelope = adaptation_tool(env, port, token, "review", {"run_id": context["id"]}, 200)
    assert review_envelope["result"]["workflow"] == "caller"
    reviewed = review_envelope["result"]["context"]
    assert reviewed["source"] == source
    assert reviewed["proposal"] == proposal
    assert reviewed["latest_submission"] == valid
    assert reviewed["status"] == "succeeded"
    assert reviewed["accepted_revision"] is None
    assert_adaptation_delta(created, adaptation_snapshot(env, app_url), submissions=2, proposals=1)

    acceptance = {"operation_id": str(uuid.uuid4()), "expected_revision": 0,
                  "script_json": proposal["script_json"], "reviewed_findings": True}
    saved = adaptation_http(port, token, f"/{context['id']}/accept", acceptance)
    assert saved["script_id"] == context_request["script_id"]
    assert saved["revision"] == 1
    assert json.loads(saved["script_json"]) == proposed_script
    accepted_envelope = adaptation_tool(env, port, token, "context-read", {"run_id": context["id"]}, 200)
    accepted = accepted_envelope["result"]
    assert accepted["status"] == "accepted"
    assert accepted["accepted_revision"] == saved
    assert import_http(port, token, source=source["id"]) == source
    assert import_http(port, token, source=source["id"], original=True) == original
    settled = adaptation_snapshot(env, app_url)
    assert_adaptation_delta(created, settled, submissions=2, proposals=1, acceptances=1, revisions=1)
    return {"source": source, "original": original, "context_request": context_request,
            "context_envelope": context_envelope, "invalid_submission": invalid_submission,
            "invalid_envelope": invalid_envelope, "valid_submission": valid_submission,
            "valid_envelope": valid_envelope, "review_envelope": review_envelope,
            "acceptance": acceptance, "saved": saved, "accepted_envelope": accepted_envelope,
            "snapshot": settled}


def adaptation_after_restart(env, port, token, app_url, fixture):
    """Exact immutable receipts/revision survive process death, database restart and replay."""
    accepted = fixture["accepted_envelope"]
    context = accepted["result"]
    reopened = adaptation_tool(env, port, token, "context-read", {"run_id": context["id"]}, 200)
    assert reopened == accepted
    replayed_context = adaptation_tool(env, port, token, "context", fixture["context_request"], 201)
    assert replayed_context["result"] == context
    for kind in ("invalid", "valid"):
        assert adaptation_tool(env, port, token, "submit", fixture[f"{kind}_submission"], 200) == fixture[f"{kind}_envelope"]
    assert adaptation_http(port, token, f"/{context['id']}/accept", fixture["acceptance"]) == fixture["saved"]
    replayed_review = adaptation_tool(env, port, token, "review", {"run_id": context["id"]}, 200)
    assert replayed_review["result"] == {"workflow": "caller", "context": context}
    assert http(port, {"script": fixture["saved"]["script_id"], "token": token}) == fixture["saved"]
    assert import_http(port, token, source=fixture["source"]["id"]) == fixture["source"]
    assert import_http(port, token, source=fixture["source"]["id"], original=True) == fixture["original"]
    assert adaptation_snapshot(env, app_url) == fixture["snapshot"]
    ADAPTATION_EVIDENCE.mkdir(parents=True, exist_ok=True)
    evidence = {"generation": "repository-authored synthetic fixture; no inference", "attempts": 0,
                "source_sha256_oracle": hashlib.sha256(fixture["original"]).hexdigest(),
                "initial_context_cli": fixture["context_envelope"],
                "invalid_submission_cli": fixture["invalid_envelope"],
                "valid_submission_cli": fixture["valid_envelope"],
                "review_cli": fixture["review_envelope"], "accepted_revision": fixture["saved"],
                "reopened_context_cli": reopened, "replayed_context_cli": replayed_context,
                "replayed_review_cli": replayed_review, "row_count_oracle": fixture["snapshot"]}
    (ADAPTATION_EVIDENCE / "external-tool-restart.json").write_text(
        json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
    print("PASS: actual subprocess CLI + loopback HTTP + app-role PostgreSQL context/invalid+valid proposal/review/accept + host kill + PostgreSQL restart + exact replay + immutable source/revision + zero generation attempts")


def studio_http(port, token, path, method="GET", payload=None):
    """Exact private Studio responses through the actual loopback socket, without proxies."""
    call = urllib.request.Request(
        f"http://127.0.0.1:{port}/api/v1/studio{path}",
        data=None if payload is None else json.dumps(payload).encode(), method=method,
        headers={"Cookie": f"cantos_session={token}", "Origin": f"http://127.0.0.1:{port}",
                 "Content-Type": "application/json"})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(call, timeout=10) as response:
        assert response.status == 200
        assert response.headers["Cache-Control"] == "no-store"
        assert response.headers["X-Content-Type-Options"] == "nosniff"
        return json.load(response)


def editor_snapshot(env, app_url):
    """Independent SQL row counts detect any duplicate or partial editor acceptance."""
    query = """SELECT json_build_object(
        'runs',(SELECT count(*) FROM adaptation_runs),
        'attempts',(SELECT count(*) FROM adaptation_attempts),
        'submissions',(SELECT count(*) FROM adaptation_submissions),
        'proposals',(SELECT count(*) FROM adaptation_proposals),
        'acceptances',(SELECT count(*) FROM adaptation_acceptances),
        'revisions',(SELECT count(*) FROM script_revisions),
        'reviews',(SELECT count(*) FROM script_reviews),
        'review_operations',(SELECT count(*) FROM script_review_operations),
        'sources',(SELECT count(*) FROM source_records),
        'evidence',(SELECT count(*) FROM script_evidence),
        'scripts',(SELECT count(*) FROM scripts))::text;"""
    result = subprocess.run(["psql", "-X", "-A", "-t", app_url, "-v", "ON_ERROR_STOP=1",
                             "-c", query], cwd=ROOT, env=env, text=True,
                            capture_output=True, timeout=10)
    if result.returncode:
        raise RuntimeError("editor row-count oracle failed")
    return json.loads(result.stdout)


def editor_probe(fixture):
    """Readiness checks the saved head in the editor database, never the other test DB."""
    return {"script": fixture["script"], "token": fixture["token"],
            "response": fixture["second"], "request": fixture["save_request"]}


def editor_recovery(env, port, app_url, fixture):
    """Compare exact source/proposal/export/review/history facts and replay committed intents."""
    token, script = fixture["token"], fixture["script"]
    assert studio_http(port, token, f"/scripts/{script}/head") == fixture["second"]
    for name in ("first", "second"):
        saved = fixture[name]
        assert studio_http(port, token, f"/scripts/{script}/revisions/{saved['revision']}") == saved
    assert import_http(port, token, request=fixture["import_request"]) == fixture["source"]
    source = fixture["source"]
    assert import_http(port, token, source=source["id"]) == source
    original = import_http(port, token, source=source["id"], original=True)
    assert original == fixture["original"].encode()
    assert hashlib.sha256(original).hexdigest() == source["sha256"]
    assert studio_http(port, token, f"/scripts/{script}/sources/{source['id']}") == fixture["linked_source"]
    context = fixture["context"]
    assert studio_http(port, token, f"/adaptations/{context['id']}/context") == context
    assert studio_http(port, token, f"/adaptations/{context['id']}/review") == {"workflow": "caller", "context": context}
    assert adaptation_http(port, token, "/contexts", fixture["context_request"], 201) == context
    assert adaptation_http(port, token, f"/{context['id']}/proposals", fixture["proposal_request"]) == context["latest_submission"]
    assert adaptation_http(port, token, f"/{context['id']}/accept", fixture["acceptance"]) == fixture["first"]
    assert studio_http(port, token, f"/scripts/{script}/revisions", "POST", fixture["save_request"]) == fixture["second"]
    assert studio_http(port, token, f"/scripts/{script}/reviews", "POST", fixture["review_request"]) == fixture["review"]
    assert studio_http(port, token, f"/scripts/{script}/history?after_revision=0&limit=1") == fixture["history_first"]
    assert studio_http(port, token, f"/scripts/{script}/history?after_revision=1&limit=1") == fixture["history_last"]
    assert studio_http(port, token, "/validation", "POST", {"script_json": fixture["second"]["script_json"]}) == {"issues": []}
    snapshot = editor_snapshot(env, app_url)
    assert snapshot == fixture["counts"], (snapshot, fixture["counts"])
    return snapshot


def catalog_http(port, path, method="GET", payload=None, expected_status=200, producer_token=None):
    """Actual socket requests against the catalog's own synthetic app-role database."""
    headers = {"Origin": f"http://127.0.0.1:{port}", "Content-Type": "application/json", "Accept": "application/json"}
    if producer_token is None:
        headers["Cookie"] = "cantos_session=" + "a" * 64
    else:
        headers["Authorization"] = "Bearer " + producer_token
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}/api/v1/studio{path}", method=method,
        data=None if payload is None else json.dumps(payload, ensure_ascii=False).encode(),
        headers=headers)
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(request, timeout=5) as response:
        assert response.status == expected_status
        assert response.headers["Cache-Control"] == "no-store"
        assert response.headers["X-Content-Type-Options"] == "nosniff"
        assert response.headers["Content-Type"] == "application/json"
        return json.load(response)


def catalog_probe(port, fixture):
    assert catalog_http(port, f"/works/{fixture['work_id']}") == fixture["current"]["work"]


def catalog_recovery(port, fixture):
    """A committed intent's original result survives HTTP process death and real PG restart."""
    result = {
        "work": catalog_http(port, f"/works/{fixture['work_id']}"),
        "unit": catalog_http(port, f"/content-units/{fixture['unit_id']}"),
        "create_replay": catalog_http(port, "/works", "POST", fixture["create_body"], 201),
        "unit_replay": catalog_http(port, f"/works/{fixture['work_id']}/content-units",
                                    "POST", fixture["unit_body"], 201),
    }
    assert result["work"] == fixture["current"]["work"]
    assert result["unit"] == fixture["current"]["units"][0]
    assert result["create_replay"] == fixture["original"]
    assert result["unit_replay"] == fixture["current"]
    content = fixture["content_target"]
    unit = content["unit_id"]
    path = f"/content-units/{unit}/content/revisions"
    result["content_head"] = catalog_http(port, f"/content-units/{unit}/content/head")
    result["content_pinned"] = catalog_http(port, f"{path}/{content['first']['content']['id']}")
    result["content_replay"] = catalog_http(port, path, "POST", content["request"], 201)
    assert result["content_head"] == content["second"]["content"]
    assert result["content_pinned"] == content["first"]["content"]
    assert result["content_replay"] == content["first"]
    brief = fixture["brief_target"]
    path = f"/content-units/{brief['unit_id']}/production-briefs"
    result["brief_read"] = catalog_http(port, f"{path}/1")
    result["brief_replay"] = catalog_http(port, path, "POST", brief["request"], 201)
    result["grant_replay"] = catalog_http(port, f"{path}/1/producer-grants", "POST", brief["grant_request"], 201)
    result["producer_instructions"] = catalog_http(port, f"/producer/content-units/{brief['unit_id']}/production-briefs/1", producer_token=brief["grant_request"]["token"])
    assert result["brief_read"] == brief["response"] == result["brief_replay"]
    assert result["grant_replay"] == brief["grant"]
    assert result["producer_instructions"] == brief["response"]["brief"]
    return result


def audio_recovery(port, fixture):
    """Replay receipts and read exact private bytes over TCP before/after API and PG restart."""
    unit, submission = fixture["unit"], fixture["request"]["id"]
    base = f"/content-units/{unit}/audio-submissions"
    result = {
        "status": catalog_http(port, f"{base}/{submission}"),
        "prepare_replay": catalog_http(port, base, "POST", fixture["request"], 201),
        "producer_status": catalog_http(port, f"/producer/audio-submissions/{submission}", producer_token=fixture["upload_token"]),
        "candidate": catalog_http(port, f"{base}/{submission}/release-candidates", "POST", fixture["candidate_request"], 201),
    }
    assert result["status"] == result["prepare_replay"] == result["producer_status"] == fixture["status"]
    assert result["candidate"] == fixture["candidate"]
    rendition = fixture["rendition"]
    path = Path(fixture["root"]) / "objects" / rendition["verified_media"]["sha256"]
    actual = path.read_bytes()
    assert len(actual) == rendition["verified_media"]["byte_length"]
    assert hashlib.sha256(actual).hexdigest() == rendition["verified_media"]["sha256"]

    class NoReadRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, request, response, code, message, headers, new_url):
            return None

    delivery_opener = urllib.request.build_opener(
        urllib.request.ProxyHandler({}), NoReadRedirect())

    def preview(method, path, extra=None, expected_status=200):
        headers = {"Cookie": "cantos_session=" + "a" * 64,
                   "Origin": f"http://127.0.0.1:{port}"}
        headers.update(extra or {})
        request = urllib.request.Request(
            f"http://127.0.0.1:{port}/api/v1/studio{path}",
            method=method, headers=headers)
        try:
            response = delivery_opener.open(request, timeout=15)
        except urllib.error.HTTPError as error:
            response = error
        with response:
            assert response.status == expected_status, (path, response.status)
            assert response.headers["Cache-Control"] == "no-store"
            assert response.headers["X-Content-Type-Options"] == "nosniff"
            return response.read(), response.headers

    media = rendition["verified_media"]
    byte_path = f"{base}/{submission}/renditions/{rendition['id']}/bytes"
    expected_etag = '"' + media["sha256"] + '"'
    received, headers = preview("GET", byte_path)
    assert received == actual
    assert hashlib.sha256(received).hexdigest() == media["sha256"]
    assert headers["Content-Type"] == "audio/ogg"
    assert headers["Content-Length"] == str(len(actual))
    assert headers["Accept-Ranges"] == "bytes"
    assert headers["ETag"] == expected_etag
    assert headers.get("Content-Range") is None
    assert headers.get("Content-Encoding") is None
    delivery = {"sha256": hashlib.sha256(received).hexdigest(),
                "byte_length": len(received), "content_type": headers["Content-Type"],
                "etag": headers["ETag"], "ranges": []}
    received, headers = preview("HEAD", byte_path, {"Range": f"bytes={len(actual)}-"})
    assert received == b""
    assert headers["Content-Length"] == str(len(actual))
    assert headers["Content-Type"] == "audio/ogg"
    assert headers["ETag"] == expected_etag
    assert headers.get("Content-Range") is None
    delivery["head_byte_length"] = int(headers["Content-Length"])
    middle, final_first = len(actual) // 2, len(actual) - 32
    for value, first, last in [
            ("bytes=0-31", 0, 31),
            (f"bytes={middle}-{middle + 31}", middle, middle + 31),
            ("bytes=-32", final_first, len(actual) - 1),
            (f"bytes={final_first}-", final_first, len(actual) - 1)]:
        received, headers = preview("GET", byte_path, {"Range": value}, 206)
        assert received == actual[first:last + 1]
        assert headers["Content-Length"] == str(last - first + 1)
        assert headers["Content-Range"] == f"bytes {first}-{last}/{len(actual)}"
        assert headers["Content-Type"] == "audio/ogg"
        assert headers["ETag"] == expected_etag
        delivery["ranges"].append({"request": value, "content_range": headers["Content-Range"],
                                   "byte_length": len(received),
                                   "sha256": hashlib.sha256(received).hexdigest()})
    received, headers = preview("GET", byte_path, {"Range": f"bytes={len(actual)}-"}, 416)
    assert headers["Content-Range"] == f"bytes */{len(actual)}"
    assert json.loads(received)["code"] == "invalid_request"
    delivery["unsatisfiable_content_range"] = headers["Content-Range"]
    access_path = byte_path.removesuffix("/bytes") + "/read-access"
    received, headers = preview("GET", access_path, expected_status=303)
    assert received == b""
    assert headers["Location"] == "/api/v1/studio" + byte_path
    assert headers.get("X-Cantos-Read-Expires-At") is None
    delivery["access_location"] = headers["Location"]
    result["delivery"] = delivery
    request = urllib.request.Request(f"http://127.0.0.1:{port}/api/v1/studio/producer/audio-submissions/{submission}/bytes", data=actual, method="PUT", headers={"Origin": f"http://127.0.0.1:{port}", "Content-Type": "application/octet-stream", "Accept": "application/json", "Authorization": "Bearer " + fixture["upload_token"]})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(request, timeout=15) as response:
        assert response.status == 200
        result["upload_replay"] = json.load(response)
    assert result["upload_replay"] == rendition
    return result


def start_host(env, port, log, fixture, readiness=None):
    host_env = dict(env, CANTOS_ENV="development", CANTOS_BIND_ADDRESS=f"127.0.0.1:{port}",
                    CANTOS_WEB_ORIGIN=f"http://127.0.0.1:{port}",
                    CANTOS_WEB_DIST=str(EVIDENCE / "no-static-assets"))
    host = subprocess.Popen([str(ROOT / "target/debug/cantos-server")], cwd=ROOT, env=host_env, stdout=log, stderr=log)
    try:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if host.poll() is not None:
                raise RuntimeError("host exited before readiness; inspect host.log")
            try:
                if readiness is None:
                    assert http(port, fixture) == fixture["response"]
                else:
                    readiness(port, fixture)
                return host
            except (OSError, urllib.error.URLError):
                time.sleep(0.05)
        raise RuntimeError("host readiness timed out")
    except BaseException:
        # Until return the caller cannot clean this process up, including assertion/interrupt.
        host.kill()
        host.wait(timeout=5)
        raise


def main():
    if not __debug__:
        raise RuntimeError("PostgreSQL assertions are required; run Python without -O")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--keep", action="store_true")
    args = parser.parse_args()
    for tool in ("initdb", "pg_ctl", "psql", "cargo"):
        if not shutil.which(tool):
            raise RuntimeError(f"unavailable: {tool}")
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    cluster = EVIDENCE / f"cluster-{time.time_ns()}"
    cluster.mkdir()
    data = cluster / "pgdata"
    port = free_port()
    env = isolated_environment()
    url = f"postgresql://cantos_test_admin@127.0.0.1:{port}/postgres"
    env["CANTOS_TEST_CLUSTER_URL"] = url
    started = False
    host = None
    editor_host = None
    catalog_host = None
    audio_host = None
    try:
        run(["initdb", "-D", str(data), "--username=cantos_test_admin", "--auth=trust", "--no-locale", "--encoding=UTF8"], env=env, output=cluster / "initdb.log")
        # Bound retained WAL on low-disk development hosts without relaxing durability.
        # Every gate still owns a new cluster; PostgreSQL checkpoints/recycles its own WAL.
        server_options = (f"-h 127.0.0.1 -p {port} -k /tmp "
                          "-c max_wal_size=64MB -c min_wal_size=32MB "
                          "-c checkpoint_timeout=30s -c fsync=on "
                          "-c synchronous_commit=on -c full_page_writes=on")
        run(["pg_ctl", "-D", str(data), "-l", str(cluster / "postgres.log"), "-o", server_options, "-w", "start"], env=env)
        started = True
        run(["psql", "-X", url, "-v", "ON_ERROR_STOP=1", "-c", "CREATE ROLE cantos_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE; SELECT version();"], env=env)
        run(["cargo", "test", "-p", "cantos-api", "--test", "studio_oidc_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1", "--nocapture"], env=env, output=EVIDENCE / "studio-oidc-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "identity_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1", "--nocapture"], env=env, output=EVIDENCE / "identity-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "revisions_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1"], env=env, output=EVIDENCE / "postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "imports_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1"], env=env, output=EVIDENCE / "import-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "adaptations_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1"], env=env, output=EVIDENCE / "adaptation-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "production_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1"], env=env, output=EVIDENCE / "production-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "catalog_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1", "--skip", "catalog_recovery_after_server_restart"], env=env, output=EVIDENCE / "catalog-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "catalog_http_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1", "--skip", "catalog_http_recovery_after_server_restart"], env=env, output=EVIDENCE / "catalog-http-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--lib", "--locked", "--offline", "contexts::listening::postgres::tests", "--", "--ignored", "--test-threads=1"], env=env, output=EVIDENCE / "listener-library-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "test", "-p", "cantos-api", "--test", "audio_intake_postgres", "--locked", "--offline", "--", "--ignored", "--test-threads=1"], env=env, output=EVIDENCE / "audio-intake-postgres-tests.log")
        reap_completed_databases(env, url, retained_restart_databases())
        run(["cargo", "build", "-p", "cantos-api", "--bins", "--locked", "--offline"], env=env, output=EVIDENCE / "host-build.log")
        fixture = json.loads((EVIDENCE / "restart.json").read_text())
        editor = json.loads((EDITOR_EVIDENCE / "editor-api-journey.json").read_text())
        catalog = json.loads((CATALOG_EVIDENCE / "http-restart.json").read_text())
        audio = json.loads((CATALOG_EVIDENCE / "audio-restart.json").read_text())
        migration_url = f"postgresql://cantos_test_admin@127.0.0.1:{port}/{fixture['database']}"
        run(["psql", "-X", migration_url, "-v", "ON_ERROR_STOP=1", "-c",
             "GRANT INSERT ON source_records,script_evidence TO cantos_app; "
             "GRANT SELECT,INSERT ON adaptation_runs,adaptation_attempts,adaptation_observations,"
             "adaptation_proposals,adaptation_cancellations,adaptation_acceptances,adaptation_submissions TO cantos_app; "
             "GRANT UPDATE(status,problem,dispatch_deadline,updated_at) ON adaptation_runs TO cantos_app; "
             "GRANT SELECT,INSERT ON production_settings,production_rights_claims,production_snapshots,production_approvals TO cantos_app; "
             "GRANT SELECT,INSERT ON audio_upload_intents,audio_intake_receipts TO cantos_app; "
             "GRANT EXECUTE ON FUNCTION production_lock_actor(text),production_lock_member(text,text) TO cantos_app;"], env=env)
        app_url = f"postgresql://cantos_app@127.0.0.1:{port}/{fixture['database']}"
        env["DATABASE_URL"] = app_url
        editor_app_url = f"postgresql://cantos_app@127.0.0.1:{port}/{editor['database']}"
        editor_env = dict(env, DATABASE_URL=editor_app_url)
        catalog_app_url = f"postgresql://cantos_app@127.0.0.1:{port}/{catalog['database']}"
        catalog_env = dict(env, DATABASE_URL=catalog_app_url)
        audio_env = dict(env, DATABASE_URL=f"postgresql://cantos_app@127.0.0.1:{port}/{audio['database']}", CANTOS_AUDIO_ROOT=audio["root"])
        with (EVIDENCE / "host.log").open("w") as log, (EDITOR_EVIDENCE / "host.log").open("w") as editor_log, (CATALOG_EVIDENCE / "http-host.log").open("w") as catalog_log:
            http_port = free_port()
            host = start_host(env, http_port, log, fixture)
            editor_http_port = free_port()
            editor_host = start_host(editor_env, editor_http_port, editor_log, editor_probe(editor))
            catalog_http_port = free_port()
            catalog_host = start_host(catalog_env, catalog_http_port, catalog_log, catalog, readiness=catalog_probe)
            audio_http_port = free_port()
            audio_host = start_host(audio_env, audio_http_port, catalog_log, audio, readiness=lambda p, f: audio_recovery(p, f))
            audio_before = audio_recovery(audio_http_port, audio)
            catalog_before = catalog_recovery(catalog_http_port, catalog)
            editor_before = editor_recovery(editor_env, editor_http_port, editor_app_url, editor)
            assert http(http_port, fixture, "POST") == fixture["response"]
            imports = import_restart_fixtures()
            receipts = [import_http(http_port, fixture["token"], request=item) for item in imports]
            for index, (item, receipt) in enumerate(zip(imports, receipts)):
                raw = bytes(item["original_bytes"])
                assert receipt["sha256"] == hashlib.sha256(raw).hexdigest()
                assert receipt["outcome"]["status"] == ("failed" if index == 2 else "parsed")
                assert import_http(http_port, fixture["token"], source=receipt["id"], original=True) == raw
            assert receipts[0]["original_text"].encode() == bytes(imports[0]["original_bytes"])
            assert [block["text"] for block in receipts[1]["outcome"]["extraction"]["blocks"]] == [
                "Mai: Ngày mai, mình có diễn tiếp không?", "Nam: Có, ở Vọng Đài."]
            production = production_before_restart(env, http_port, fixture["token"], app_url)
            adaptation = adaptation_before_restart(env, http_port, fixture["token"], app_url)
            # Abrupt HTTP process death after a committed save (lost response replay).
            host.kill()
            host.wait(timeout=5)
            host = None
            editor_host.kill()
            editor_host.wait(timeout=5)
            editor_host = None
            catalog_host.kill()
            catalog_host.wait(timeout=5)
            catalog_host = None
            audio_host.kill()
            audio_host.wait(timeout=5)
            audio_host = None
            run(["pg_ctl", "-D", str(data), "-l", str(cluster / "postgres.log"), "-m", "fast", "-w", "restart"], env=env)
            run(["cargo", "test", "-p", "cantos-api", "--test", "catalog_postgres", "--locked", "--offline", "catalog_recovery_after_server_restart", "--", "--ignored", "--exact", "--test-threads=1"], env=env, output=EVIDENCE / "catalog-restart-tests.log")
            print("PASS: Catalog PostgreSQL restart + exact work/unit snapshots + migration replay + original-operation replay + independent row counts")
            run(["cargo", "test", "-p", "cantos-api", "--test", "catalog_http_postgres", "--locked", "--offline", "catalog_http_recovery_after_server_restart", "--", "--ignored", "--exact", "--test-threads=1"], env=env, output=EVIDENCE / "catalog-http-restart-tests.log")
            print("PASS: Studio Catalog real HTTP adapter + PostgreSQL restart + exact work/unit snapshots + original-operation replay + independent row counts")
            migration_env = dict(env, DATABASE_URL=f"postgresql://cantos_test_admin@127.0.0.1:{port}/{fixture['database']}")
            run([str(ROOT / "target/debug/cantos-migrate")], env=migration_env)
            editor_migration_env = dict(env, DATABASE_URL=f"postgresql://cantos_test_admin@127.0.0.1:{port}/{editor['database']}")
            run([str(ROOT / "target/debug/cantos-migrate")], env=editor_migration_env)
            host = start_host(env, http_port, log, fixture)
            editor_host = start_host(editor_env, editor_http_port, editor_log, editor_probe(editor))
            catalog_host = start_host(catalog_env, catalog_http_port, catalog_log, catalog, readiness=catalog_probe)
            audio_host = start_host(audio_env, audio_http_port, catalog_log, audio, readiness=lambda p, f: audio_recovery(p, f))
            audio_after = audio_recovery(audio_http_port, audio)
            assert audio_before == audio_after
            (CATALOG_EVIDENCE / "audio-server-restart.json").write_text(json.dumps({"provenance": audio["provenance"], "before": audio_before, "after": audio_after, "checks": "real socket HTTP; killed API host; PostgreSQL restart; exact original-operation prepare/PUT/candidate replay; owner GET/HEAD SHA-256/length/type/ETag; first/middle/suffix/final ranges and 416; local read-access 303"}, ensure_ascii=False, indent=2) + "\n")
            print("PASS: Audio Lifecycle actual socket HTTP + API kill + PostgreSQL restart + exact immutable receipts/reviews/candidates + original PUT replay + owner byte GET/HEAD/Range integrity + refreshable local read access")
            catalog_after = catalog_recovery(catalog_http_port, catalog)
            assert catalog_after == catalog_before
            (CATALOG_EVIDENCE / "http-server-restart.json").write_text(json.dumps({
                "provenance": catalog["provenance"],
                "checks": "actual loopback HTTP; process kill; PostgreSQL restart; exact original-operation replay",
                "before": catalog_before, "after": catalog_after,
                "work_id": catalog["work_id"], "unit_id": catalog["unit_id"]},
                ensure_ascii=False, indent=2) + "\n")
            print("PASS: Studio Catalog actual socket HTTP + server kill + PostgreSQL restart + exact work/unit snapshots + original-operation replay")
            assert http(http_port, fixture) == fixture["response"]
            assert http(http_port, fixture, "POST") == fixture["response"]
            # C2 seeds this operation via Protobuf before host death. Replays through JSON,
            # legacy MessagePack and Protobuf must preserve the original row and exact digests.
            expected_protobuf = reference_protobuf.encode("RevisionResponse", fixture["response"])
            assert protobuf_http(http_port, fixture) == (200, expected_protobuf)
            assert protobuf_http(http_port, fixture, "POST") == (200, expected_protobuf)
            assert protobuf_http(http_port, fixture, accept="text/html") == (406, {
                "code": "invalid_request", "current_revision": None,
                "issues": [{"path": "accept", "rule": "not_acceptable"}]})
            # The lost-response replay again, in MessagePack, after process death and restart.
            assert msgpack_http(http_port, fixture) == (200, fixture["response"])
            assert msgpack_http(http_port, fixture, "POST") == (200, fixture["response"])
            assert msgpack_http(http_port, fixture, accept="text/html") == (406, {
                "code": "invalid_request", "current_revision": None,
                "issues": [{"path": "accept", "rule": "not_acceptable"}]})
            for item, receipt in zip(imports, receipts):
                assert import_http(http_port, fixture["token"], request=item) == receipt
                assert import_http(http_port, fixture["token"], source=receipt["id"]) == receipt
                assert import_http(http_port, fixture["token"], source=receipt["id"], original=True) == bytes(item["original_bytes"])
            adaptation_after_restart(env, http_port, fixture["token"], app_url, adaptation)
            production_after_restart(env, http_port, fixture["token"], app_url, production)
            editor_after = editor_recovery(editor_env, editor_http_port, editor_app_url, editor)
            assert editor_after == editor_before
            (EDITOR_EVIDENCE / "server-restart.json").write_text(json.dumps({
                "provenance": editor["provenance"], "checks": "exact source/proposal/accepted revisions/reviews/history; committed-intent replay; HTTP process kill; PostgreSQL restart; migration replay",
                "before": editor_before, "after": editor_after,
                "script": editor["script"], "run_id": editor["context"]["id"],
                "source_sha256": editor["source"]["sha256"]}, ensure_ascii=False, indent=2) + "\n")
            print("PASS: Studio editor HTTP journey + host kill + PostgreSQL restart + exact source/proposal/accepted revisions/review/history + idempotent replay + independent row counts")
            (EVIDENCE / "import-restart.json").write_text(json.dumps({"sources": receipts}, ensure_ascii=False, indent=2) + "\n")
            print("PASS: original TXT/DOCX/failed-input bytes and SHA-256 + HTTP kill + PostgreSQL restart + immutable receipt replay")
            print("PASS: HTTP process kill + PostgreSQL restart + migration replay + exact export/actor/time + same-operation replay")
            print("PASS: MessagePack head/replay decoded by the independent oracle equal the JSON revision; 406 stays a JSON error")
        (EVIDENCE / "run.json").write_text(json.dumps({"cluster": str(cluster), "port": port, "app_url": app_url, "catalog_app_url": catalog_app_url,
            "audio_app_url": audio_env["DATABASE_URL"], "audio_root": audio["root"], "script": fixture["script"]}, indent=2) + "\n")
        (EDITOR_EVIDENCE / "run.json").write_text(json.dumps({
            "cluster": str(cluster), "data": str(data), "port": port,
            "app_url": editor_app_url, "script": editor["script"],
            "run_id": editor["context"]["id"], "source_id": editor["source"]["id"],
            "token": editor["token"], "retained": args.keep}, indent=2) + "\n")
    finally:
        if audio_host is not None:
            audio_host.kill()
            audio_host.wait(timeout=5)
        if host is not None:
            host.kill()
            host.wait(timeout=5)
        if editor_host is not None:
            editor_host.kill()
            editor_host.wait(timeout=5)
        if catalog_host is not None:
            catalog_host.kill()
            catalog_host.wait(timeout=5)
        if started and not args.keep:
            run(["pg_ctl", "-D", str(data), "-m", "fast", "-w", "stop"], env=env)
        if args.keep and started:
            print(f"Retained test cluster. Stop with: pg_ctl -D {data} -m fast -w stop")


if __name__ == "__main__":
    main()
