"""Independent production-input oracle, called only by the disposable PostgreSQL runner.

Uses Python JSON/hashlib/integer arithmetic and real loopback HTTP/app-role SQL;
never invokes a synthesis provider or accepts an inherited database configuration.
Synthetic rights declarations and planning rates are local test facts, not legal
clearance, provider prices, payment approval or provider receipts.
"""

import copy
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
from pathlib import Path
import subprocess
import time
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "target" / "production-input-evidence"
I64_MAX = (1 << 63) - 1
DIGEST_TAG = b"cantos/production-inputs/p1\n"


def input_digest(document):
    """p1's complete scoped document, independently encoded from the Rust serializer."""
    scoped = copy.deepcopy(document)
    scoped.pop("input_digest", None)
    canonical = json.dumps(scoped, ensure_ascii=False, sort_keys=True,
                           separators=(",", ":"), allow_nan=False).encode("utf-8")
    return "production-p1:sha256:" + hashlib.sha256(DIGEST_TAG + canonical).hexdigest()


def estimate(texts, rate):
    """Sum each dialogue's scalar-count rational ceiling; no floats or UTF-8 units."""
    texts = list(texts)
    for text in texts:
        # Lone surrogate code points cannot be valid Rust/UTF-8 scalar values.
        text.encode("utf-8")
    units = sum(len(text) for text in texts)
    denominator, price = rate["units_per_charge"], rate["amount_minor"]
    if (type(denominator) is not int or not 0 < denominator <= I64_MAX
            or type(price) is not int or not 0 <= price <= I64_MAX):
        raise ValueError("invalid planning rate")
    if units > I64_MAX:
        raise ValueError("usage exceeds supported scalar-count bound")
    amount = sum((len(text) * price + denominator - 1) // denominator for text in texts)
    if amount > I64_MAX:
        raise ValueError("estimate exceeds supported minor-unit bound")
    return units, amount


def http(port, token, path, payload=None, expected_status=200, method=None):
    """Actual private API, explicitly loopback with environment proxies disabled."""
    if type(port) is not int or not 1 <= port <= 65535:
        raise ValueError("invalid loopback port")
    if not path.startswith("/api/v1/studio/") or "\n" in path or "\r" in path:
        raise ValueError("invalid private API path")
    body = None if payload is None else json.dumps(payload, ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}", data=body,
        method=method or ("GET" if payload is None else "POST"),
        headers={"Cookie": f"cantos_session={token}",
                 "Origin": f"http://127.0.0.1:{port}", "Content-Type": "application/json"})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        response = opener.open(request, timeout=10)
    except urllib.error.HTTPError as error:
        response = error
    with response:
        if response.status != expected_status:
            # Read only the public error code; never include session headers or a body dump.
            try:
                code = json.load(response).get("code", "unknown")
            except (ValueError, AttributeError):
                code = "invalid_response"
            raise AssertionError(f"production HTTP {path}: expected {expected_status}, got {response.status} ({code})")
        assert response.headers["Cache-Control"] == "no-store"
        assert response.headers["X-Content-Type-Options"] == "nosniff"
        result = json.load(response)
    if token and token in json.dumps(result):
        raise RuntimeError("production HTTP exposed its session token; output suppressed")
    return result


def sql(env, app_url, query):
    """Read this runner's app-role database; explicitly refuse non-local/test URLs."""
    parsed = urllib.parse.urlparse(app_url)
    if (parsed.scheme != "postgresql" or parsed.hostname != "127.0.0.1"
            or parsed.username != "cantos_app" or parsed.password is not None
            or parsed.port is None or not parsed.path.startswith("/cantos_test_")
            or parsed.query or parsed.fragment):
        raise ValueError("production oracle requires the disposable app-role database")
    result = subprocess.run(["psql", "-X", "-q", "-A", "-t", app_url,
                             "-v", "ON_ERROR_STOP=1", "-c", query],
                            cwd=ROOT, env=env, text=True, capture_output=True, timeout=10)
    if result.returncode:
        raise RuntimeError("production app-role SQL oracle failed")
    return json.loads(result.stdout)


def row_counts(env, app_url, script):
    # script is generated here, never an externally supplied SQL identifier or literal.
    if str(uuid.UUID(script)) != script:
        raise ValueError("invalid oracle script UUID")
    return sql(env, app_url, f"""SELECT json_build_object(
        'settings',(SELECT count(*) FROM production_settings WHERE script_id='{script}'),
        'rights',(SELECT count(*) FROM production_rights_claims WHERE script_id='{script}'),
        'snapshots',(SELECT count(*) FROM production_snapshots WHERE script_id='{script}'),
        'approvals',(SELECT count(*) FROM production_approvals WHERE script_id='{script}'),
        'revisions',(SELECT count(*) FROM script_revisions WHERE script_id='{script}'),
        'reviews',(SELECT count(*) FROM script_reviews WHERE script_id='{script}'),
        'retired_execution_relations',(SELECT count(*) FROM pg_catalog.pg_class c
            JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public'
            AND (left(c.relname,8)='durable_'
                OR c.relname IN ('adaptation_attempts','adaptation_observations','speech_clips'))))::text;""")


def assert_delta(before, after, **changes):
    assert before.keys() == after.keys()
    for name in before:
        assert after[name] == before[name] + changes.get(name, 0), (name, before, after)
    assert after["retired_execution_relations"] == 0


def finding_codes(response):
    return {finding["code"] for finding in response["findings"]}


def rights_ids(script):
    """Project all applicable rights references from the complete accepted export."""
    ids = {script["work"]["rights_record_id"], script["adaptation"]["rights_record_id"]}
    ids.update(item["rights_record_id"] for item in script["provenance"])
    for act in script["episode"]["acts"]:
        for scene in act["scenes"]:
            for cue in scene.get("sound_cues", []):
                if "asset" in cue:
                    ids.add(cue["asset"]["rights_record_id"])
    return sorted(ids)


def resolved_inputs(script, settings):
    """Reference-only v1 resolution from accepted facts, independent of Rust's encoder."""
    bindings = {binding["character_id"]: binding for binding in settings["bindings"]}
    resolved = []
    for act in script["episode"]["acts"]:
        for scene in act["scenes"]:
            for line in scene["dialogues"]:
                binding = bindings[line["speaker_id"]]
                pronunciation = {entry["surface"]: entry["replacement"] for entry in binding["pronunciation"]}
                for entry in line.get("pronunciation_overrides", []):
                    if entry["surface"] in pronunciation and pronunciation[entry["surface"]] != entry["replacement"]:
                        raise ValueError("conflicting pronunciation")
                    pronunciation[entry["surface"]] = entry["replacement"]
                text, cursor = "", 0
                while cursor < len(line["text"]):
                    matches = [surface for surface in pronunciation if line["text"].startswith(surface, cursor)]
                    if matches:
                        surface = max(matches, key=len)
                        text += pronunciation[surface]
                        cursor += len(surface)
                    else:
                        text += line["text"][cursor]
                        cursor += 1
                performance = copy.deepcopy(binding["performance"])
                for key, script_key in (("emotion", "emotion"), ("intensity_permille", "intensity_permille")):
                    if performance[key] is None:
                        performance[key] = line["delivery"][script_key]
                resolved.append({
                    "dialogue_id": line["id"], "character_id": line["speaker_id"], "text": unicodedata.normalize("NFC", text),
                    "language": binding["language"], "provider_id": binding["provider_id"],
                    "model_id": binding["model_id"], "voice_id": binding["voice_id"],
                    "voice_rights_record_id": binding["voice_rights_record_id"],
                    "adapter_version": "reference-only-no-dispatch-v1", "output_contract": "reference-only/no-audio",
                    "performance": performance, "pronunciation": [
                        {"surface": surface, "replacement": pronunciation[surface]} for surface in sorted(pronunciation)]})
    return resolved


def validate_document(document):
    assert document["input_digest"] == input_digest(document)
    assert document["resolved"] == resolved_inputs(json.loads(document["revision"]["script_json"]),
                                                  document["settings"]["settings"])
    assert [binding["character_id"] for binding in document["settings"]["settings"]["bindings"]] == sorted(
        binding["character_id"] for binding in document["settings"]["settings"]["bindings"])
    assert [right["claim"]["declaration"]["record_id"] for right in document["rights"]] == sorted(
        right["claim"]["declaration"]["record_id"] for right in document["rights"])
    budget = document["settings"]["settings"]["budget"]
    if budget["rate"] is None:
        assert document["estimate"]["status"] == "unavailable"
    else:
        units, amount = estimate([line["text"] for line in document["resolved"]], budget["rate"])
        assert document["estimate"] == {
            "status": "known", "currency": budget["currency"], "amount_minor": amount,
            "units": units, "rate_reference": budget["rate"]["reference"],
            "rate_version": budget["rate"]["version"]}
    for line in document["resolved"]:
        assert line["text"] == unicodedata.normalize("NFC", line["text"])


def immutable_snapshot(snapshot):
    """Approval and eligibility are live projections; the pinned facts are immutable."""
    return {key: value for key, value in snapshot.items() if key not in ("approval", "eligibility")}


def scoped_projection(revision, settings, rights, preview, catalog_version):
    """Construct p1 facts from separate HTTP receipts, including canonical array ordering."""
    canonical_settings = copy.deepcopy(settings)
    canonical_settings["settings"]["bindings"].sort(key=lambda binding: binding["character_id"])
    for binding in canonical_settings["settings"]["bindings"]:
        binding["pronunciation"].sort(key=lambda item: item["surface"])
    applicable = set(rights_ids(json.loads(revision["script_json"])))
    applicable.update(binding["voice_rights_record_id"] for binding in canonical_settings["settings"]["bindings"])
    canonical_rights = sorted((copy.deepcopy(right) for right in rights
                              if right["claim"]["declaration"]["record_id"] in applicable),
                             key=lambda right: right["claim"]["declaration"]["record_id"])
    return {"owner_id": "alice", "catalog_version": catalog_version, "revision": revision,
            "settings": canonical_settings, "rights": canonical_rights,
            "resolved": preview["resolved"], "estimate": preview["estimate"]}


def bounded_reads(env, app_url, port, token, base, snapshot_id, script):
    """Small-fixture local observations, not a throughput or production benchmark."""
    samples = {}
    for name, path in (("state", base), ("review", base + "/review"),
                       ("snapshot", base + f"/snapshots/{snapshot_id}"),
                       ("eligibility", base + f"/snapshots/{snapshot_id}/eligibility")):
        elapsed = []
        for _ in range(9):
            start = time.perf_counter_ns()
            http(port, token, path)
            elapsed.append((time.perf_counter_ns() - start) / 1_000_000)
        samples[name] = summarize_latency(elapsed)
    queries = {
        "latest_settings": f"SELECT version,settings_bytes FROM production_settings WHERE script_id='{script}' ORDER BY version DESC LIMIT 1",
        "latest_rights": f"SELECT DISTINCT ON(record_id) record_id,version,claim_bytes FROM production_rights_claims WHERE script_id='{script}' AND owner_id='alice' ORDER BY record_id,version DESC LIMIT 129",
        "snapshot_history": f"SELECT id,input_digest FROM production_snapshots WHERE script_id='{script}' ORDER BY recorded_at DESC,id DESC LIMIT 20",
    }
    plans = {name: sql(env, app_url, "EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) " + query)
             for name, query in queries.items()}
    alternatives = {name: sql(env, app_url, "BEGIN; SET LOCAL enable_seqscan=off; EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) " + query + "; ROLLBACK;")
                    for name, query in queries.items()}
    return {"workload": "one synthetic script; nine sequential loopback reads per route; no concurrency/throughput claim",
            "local_target_p95_ms": 100, "row_counts": row_counts(env, app_url, script),
            "http": samples, "plans": plans, "index_plan_probe": alternatives,
            "comparison_limit": "enable_seqscan=off is an existing-index plan probe, not a measured speedup or index admission"}


def summarize_latency(samples):
    assert samples
    ordered = sorted(samples)
    # Nearest rank. Nine samples have no representative tail-distribution claim.
    p95 = ordered[(len(ordered) * 95 + 99) // 100 - 1]
    return {"raw_ms": samples, "samples": len(samples), "median_ms": ordered[len(ordered) // 2],
            "p95_ms": p95, "max_ms": max(samples), "local_target_met": p95 < 100}


def production_before_restart(env, port, token, app_url):
    """Cross real import/revision/production HTTP and PostgreSQL with synthetic facts."""
    catalog = http(port, token, "/api/v1/studio/production/catalog")
    capabilities = [item for item in catalog["capabilities"] if item["reference_only"]]
    assert capabilities and all(not item["billable_dispatch_available"] for item in capabilities)
    capability = next(item for item in capabilities if "vi-VN" in item["languages"])
    script = str(uuid.uuid4())
    base = f"/api/v1/studio/scripts/{script}/production"
    source_bytes = "Synthetic source: Ngày mai, mình có diễn tiếp không? 🎭\n".encode()
    import_request = {"metadata": {
        "operation_id": str(uuid.uuid4()), "file_name": "production-oracle-vi.txt", "format": "txt",
        "reference": "repository-authored disposable production-input fixture",
        "rights_holder": "Synthetic fixture author", "permission_evidence": "local test only; no legal verification",
        "usage_scope": "Disposable local production-input logic checks only"},
        "original_bytes": list(source_bytes)}
    source = http(port, token, "/api/v1/studio/imports", import_request)
    assert source["sha256"] == hashlib.sha256(source_bytes).hexdigest()
    document = json.loads((ROOT / "packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json").read_text())
    document["provenance"][0]["source_record_id"] = source["id"]
    # NFD input is normalized by the real accepted export; an astral scalar is one unit.
    document["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["text"] = "Nga\u0300y mai, mình có diễn tiếp không? 🎭"
    revision_request = {"operation_id": str(uuid.uuid4()), "expected_revision": 0,
                        "script_json": json.dumps(document, ensure_ascii=False)}
    revision = http(port, token, f"/api/v1/studio/scripts/{script}/revisions", revision_request)
    accepted_document = json.loads(revision["script_json"])
    assert accepted_document["provenance"][0]["source_record_id"] == source["id"]
    assert accepted_document["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["text"] == "Ngày mai, mình có diễn tiếp không? 🎭"
    assert http(port, token, base) == {"settings": None, "rights": [], "snapshots": []}
    voice_right = "rights-oracle-synthetic-voice"
    settings = {"bindings": [{
        "character_id": character["id"], "provider_id": capability["provider_id"],
        "model_id": capability["model_id"], "voice_id": capability["voice_ids"][0],
        "language": "vi-VN", "voice_rights_record_id": voice_right,
        "performance": {"rate_permille": 1000, "pitch_semitones": 0,
                        "emotion": None, "intensity_permille": None}, "pronunciation": []}
        for character in reversed(accepted_document["characters"])],
        "budget": {"currency": "VND", "limit_minor": 1_000_000, "territory": "private-planning",
                   "scope": "synthetic episode-local planning only; no payment permission",
                   "rate": {"reference": "synthetic caller-declared scalar planning rate; not a provider price",
                            "version": "oracle-rate-1", "units_per_charge": 7, "amount_minor": 3}}}
    setting_intents, setting_receipts, rights_intents, rights_receipts = [], [], [], []
    writes = {"settings": [], "rights": [], "approval": []}
    version = 0

    def save_settings(value, script_revision=revision["revision"]):
        nonlocal version
        request = {"operation_id": str(uuid.uuid4()), "expected_revision": script_revision,
                   "expected_settings_version": version, "settings": copy.deepcopy(value)}
        start = time.perf_counter_ns()
        receipt = http(port, token, base + "/settings", request)
        writes["settings"].append((time.perf_counter_ns() - start) / 1_000_000)
        version += 1
        assert receipt["version"] == version and receipt["script_revision"] == script_revision
        setting_intents.append(request)
        setting_receipts.append(receipt)
        return receipt

    def save_right(record_id, subject, status="granted", expected_version=0, until=None, start=0):
        request = {"operation_id": str(uuid.uuid4()), "expected_version": expected_version,
                   "claim": {"record_id": record_id, "subject": subject, "status": status,
                             "scope": "production_synthesis", "rights_holder": "Synthetic fixture author",
                             "languages": ["vi-VN"], "territory": "private-planning",
                             "attribution": "Synthetic local test data", "restrictions": "private-planning-only",
                             "permitted_scope": settings["budget"]["scope"],
                             "reference": "synthetic local assertion; not legally verified; not provider consent",
                             "valid_from_unix": start, "valid_until_unix": until}}
        start_time = time.perf_counter_ns()
        receipt = http(port, token, base + "/rights", request)
        writes["rights"].append((time.perf_counter_ns() - start_time) / 1_000_000)
        assert receipt["claim"] == {"version": expected_version + 1, "declaration": request["claim"]}
        rights_intents.append(request)
        rights_receipts.append(receipt)
        return receipt

    initial = row_counts(env, app_url, script)
    first_settings = save_settings(settings)
    assert_delta(initial, row_counts(env, app_url, script), settings=1)
    preview = http(port, token, base + "/review")
    assert {"script_unreviewed", "rights_missing"} <= finding_codes(preview)
    assert not preview["inputs_eligible"] and not preview["billable_dispatch_available"]
    freeze_intents, snapshots, approval_intents, approval_receipts = [], [], [], []

    def freeze_current():
        preview = http(port, token, base + "/review")
        request = {"operation_id": str(uuid.uuid4()), "expected_revision": preview["script_revision"],
                   "settings_version": preview["settings_version"], "input_digest": preview["input_digest"]}
        # Two actual sockets deliver the same committed intent concurrently.
        before = row_counts(env, app_url, script)
        with ThreadPoolExecutor(max_workers=2) as pool:
            deliveries = list(pool.map(lambda _: http(port, token, base + "/snapshots", request, 201), range(2)))
        assert deliveries[0] == deliveries[1]
        snapshot = deliveries[0]
        validate_document(snapshot["document"])
        assert snapshot["document"]["input_digest"] == preview["input_digest"]
        assert snapshot["document"]["resolved"] == preview["resolved"]
        assert snapshot["document"]["estimate"] == preview["estimate"]
        assert_delta(before, row_counts(env, app_url, script), snapshots=1)
        freeze_intents.append(request)
        snapshots.append(snapshot)
        return snapshot

    blocked = freeze_current()
    blocked_approval = {"operation_id": str(uuid.uuid4()), "input_digest": blocked["document"]["input_digest"]}
    before = row_counts(env, app_url, script)
    rejected = http(port, token, base + f"/snapshots/{blocked['id']}/approvals", blocked_approval, 409)
    assert rejected["code"] == "production_blocked"
    assert row_counts(env, app_url, script) == before
    reviewed_request = {"operation_id": str(uuid.uuid4()), "revision": revision["revision"]}
    review = http(port, token, f"/api/v1/studio/scripts/{script}/reviews", reviewed_request)
    for record_id in rights_ids(accepted_document):
        save_right(record_id, {"kind": "evidence", "record_id": record_id})
    voice_subject = {"kind": "voice", "provider_id": capability["provider_id"],
                     "model_id": capability["model_id"], "voice_id": capability["voice_ids"][0]}
    voice_receipt = save_right(voice_right, voice_subject)
    preview = http(port, token, base + "/review")
    assert preview["findings"] == [] and preview["inputs_eligible"]
    assert not preview["billable_dispatch_available"]
    approved_snapshot = freeze_current()

    def approve(snapshot):
        request = {"operation_id": str(uuid.uuid4()), "input_digest": snapshot["document"]["input_digest"]}
        before = row_counts(env, app_url, script)
        start = time.perf_counter_ns()
        receipt = http(port, token, base + f"/snapshots/{snapshot['id']}/approvals", request)
        writes["approval"].append((time.perf_counter_ns() - start) / 1_000_000)
        assert receipt["input_digest"] == request["input_digest"] and receipt["approved_by"] == "alice"
        assert_delta(before, row_counts(env, app_url, script), approvals=1)
        eligibility = http(port, token, base + f"/snapshots/{snapshot['id']}/eligibility")
        assert eligibility == {"inputs_eligible": True, "approval_current": True,
                               "billable_dispatch_available": False, "findings": []}
        approval_intents.append((snapshot["id"], request))
        approval_receipts.append(receipt)
        return receipt

    approve(approved_snapshot)
    approved_path = base + f"/snapshots/{approved_snapshot['id']}"
    approved_document = copy.deepcopy(approved_snapshot["document"])
    before = row_counts(env, app_url, script)
    for path in (base, base + "/review", approved_path, approved_path + "/eligibility"):
        assert http(port, "b" * 64, path, expected_status=404)["code"] == "not_found"
    assert http(port, "", base, expected_status=401)["code"] == "unauthenticated"
    denied_settings = copy.deepcopy(setting_intents[-1])
    denied_settings.update(operation_id=str(uuid.uuid4()), expected_settings_version=version)
    assert http(port, "b" * 64, base + "/settings", denied_settings, 404)["code"] == "not_found"
    denied_approval = {"operation_id": str(uuid.uuid4()), "input_digest": approved_document["input_digest"]}
    assert http(port, "b" * 64, approved_path + "/approvals", denied_approval, 404)["code"] == "not_found"
    stale_settings = copy.deepcopy(setting_intents[-1])
    stale_settings["operation_id"] = str(uuid.uuid4())
    assert http(port, token, base + "/settings", stale_settings, 409)["code"] == "stale_production_inputs"
    altered_digest = approved_document["input_digest"][:-1] + ("1" if approved_document["input_digest"][-1] == "0" else "0")
    reused_freeze = copy.deepcopy(freeze_intents[-1])
    reused_freeze["input_digest"] = altered_digest
    assert http(port, token, base + "/snapshots", reused_freeze, 409)["code"] == "operation_reused"
    reused_freeze["operation_id"] = str(uuid.uuid4())
    assert http(port, token, base + "/snapshots", reused_freeze, 409)["code"] == "stale_production_inputs"
    denied_approval["input_digest"] = altered_digest
    assert http(port, token, approved_path + "/approvals", denied_approval, 409)["code"] == "stale_production_inputs"
    unsupported = copy.deepcopy(setting_intents[0])
    unsupported.update(operation_id=str(uuid.uuid4()), expected_settings_version=version)
    unsupported["settings"]["bindings"][0]["voice_id"] = "unsupported-no-substitution"
    assert http(port, token, base + "/settings", unsupported, 400)["code"] == "invalid_request"
    assert row_counts(env, app_url, script) == before

    # Known cost does not become a charge, and unknown cost never silently becomes zero.
    units, amount = estimate([line["text"] for line in approved_document["resolved"]], settings["budget"]["rate"])
    unknown = copy.deepcopy(settings)
    unknown["budget"]["rate"] = None
    save_settings(unknown)
    unknown_preview = http(port, token, base + "/review")
    assert unknown_preview["estimate"]["status"] == "unavailable"
    assert "estimate_unavailable" in finding_codes(unknown_preview) and not unknown_preview["inputs_eligible"]
    exceeded = copy.deepcopy(settings)
    exceeded["budget"]["limit_minor"] = amount - 1
    save_settings(exceeded)
    exceeded_preview = http(port, token, base + "/review")
    assert exceeded_preview["estimate"]["amount_minor"] == amount
    assert "budget_exceeded" in finding_codes(exceeded_preview) and not exceeded_preview["inputs_eligible"]

    changes = []
    voice = copy.deepcopy(settings)
    for binding in voice["bindings"]:
        binding["voice_id"] = capability["voice_ids"][1]
    changes.append(("voice", voice))
    model = copy.deepcopy(settings)
    for binding in model["bindings"]:
        binding["model_id"] = capabilities[1]["model_id"]
    changes.append(("model", model))
    performance = copy.deepcopy(settings)
    performance["bindings"][0]["performance"]["rate_permille"] = 1001
    changes.append(("performance", performance))
    pronunciation = copy.deepcopy(settings)
    next(binding for binding in pronunciation["bindings"] if binding["character_id"] == "an")["pronunciation"] = [
        {"surface": "Vọng Đài", "replacement": "Vọng Đài"}]
    changes.append(("pronunciation", pronunciation))
    budget_scope = copy.deepcopy(settings)
    budget_scope["budget"]["scope"] += "; revised scope"
    changes.append(("budget_scope", budget_scope))
    for name, changed in changes:
        receipt = save_settings(changed)
        changed_preview = http(port, token, base + "/review")
        assert changed_preview["input_digest"] != approved_document["input_digest"], name
        state = http(port, token, base)
        projected = scoped_projection(revision, receipt, state["rights"], changed_preview, catalog["version"])
        assert input_digest(projected) == changed_preview["input_digest"], name
        reopened = http(port, token, approved_path)
        assert reopened["document"] == approved_document
        assert not reopened["eligibility"]["approval_current"]
        assert "settings_changed" in finding_codes(reopened["eligibility"])
    save_settings(settings)
    voice_version = voice_receipt["claim"]["version"]
    rights_failures = []
    for status, until, start, expected_code in (
            ("pending", None, 0, "rights_pending"),
            ("revoked", None, 0, "rights_revoked"),
            ("granted", int(time.time()) - 60, 0, "rights_expired"),
            ("granted", None, int(time.time()) + 3600, "rights_not_yet_valid")):
        receipt = save_right(voice_right, voice_subject, status, voice_version, until, start)
        voice_version = receipt["claim"]["version"]
        current = http(port, token, base + "/review")
        assert expected_code in finding_codes(current) and not current["inputs_eligible"]
        old = http(port, token, approved_path)
        assert old["document"] == approved_document
        assert "rights_changed" in finding_codes(old["eligibility"])
        assert not old["eligibility"]["approval_current"]
        rights_failures.append({"declaration": receipt["claim"], "findings": current["findings"]})
    save_right(voice_right, voice_subject, expected_version=voice_version)
    source_right = accepted_document["work"]["rights_record_id"]
    source_subject = {"kind": "evidence", "record_id": source_right}
    save_right(source_right, source_subject, "revoked", expected_version=1)
    assert "rights_revoked" in finding_codes(http(port, token, base + "/review"))
    assert not http(port, token, approved_path + "/eligibility")["approval_current"]
    save_right(source_right, source_subject, expected_version=2)

    edited_document = copy.deepcopy(accepted_document)
    edited_document["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["text"] += " Hẹn gặp lại!"
    edit_request = {"operation_id": str(uuid.uuid4()), "expected_revision": revision["revision"],
                    "script_json": json.dumps(edited_document, ensure_ascii=False)}
    second_revision = http(port, token, f"/api/v1/studio/scripts/{script}/revisions", edit_request)
    old = http(port, token, approved_path)
    assert old["document"] == approved_document
    assert "revision_changed" in finding_codes(old["eligibility"])
    assert not old["eligibility"]["approval_current"]
    save_settings(settings, second_revision["revision"])
    assert "script_unreviewed" in finding_codes(http(port, token, base + "/review"))
    second_review_request = {"operation_id": str(uuid.uuid4()), "revision": second_revision["revision"]}
    second_review = http(port, token, f"/api/v1/studio/scripts/{script}/reviews", second_review_request)
    final_snapshot = freeze_current()
    final_approval = approve(final_snapshot)
    state = http(port, token, base)
    assert state["settings"] == setting_receipts[-1]
    assert len(state["snapshots"]) == len(snapshots) and len(state["snapshots"]) <= 20
    counts = row_counts(env, app_url, script)
    assert counts["settings"] == len(setting_intents)
    assert counts["rights"] == len(rights_intents)
    assert counts["snapshots"] == len(snapshots) == 3 and counts["approvals"] == 2
    assert counts["revisions"] == 2 and counts["reviews"] == 2
    assert counts["retired_execution_relations"] == 0
    reads = bounded_reads(env, app_url, port, token, base, final_snapshot["id"], script)
    reads["writes"] = {name: summarize_latency(samples) for name, samples in writes.items()}
    return {"script": script, "base": base, "catalog": catalog, "source": source,
            "revision_request": revision_request, "revision": revision, "edit_request": edit_request,
            "second_revision": second_revision, "review_request": reviewed_request, "review": review,
            "second_review_request": second_review_request, "second_review": second_review,
            "settings_requests": setting_intents, "settings_receipts": setting_receipts,
            "rights_requests": rights_intents, "rights_receipts": rights_receipts,
            "freeze_requests": freeze_intents, "snapshots": snapshots,
            "approval_requests": approval_intents, "approval_receipts": approval_receipts,
            "state": state, "counts": counts, "reads": reads, "units": units, "amount_minor": amount,
            "rights_failures": rights_failures, "final_snapshot_id": final_snapshot["id"],
            "final_approval": final_approval}


def production_after_restart(env, port, token, app_url, fixture):
    """Exact facts survive HTTP process kill, PostgreSQL restart and migration replay."""
    base, script = fixture["base"], fixture["script"]
    assert http(port, token, base) == fixture["state"]
    for request, receipt in zip(fixture["settings_requests"], fixture["settings_receipts"]):
        assert http(port, token, base + "/settings", request) == receipt
    for request, receipt in zip(fixture["rights_requests"], fixture["rights_receipts"]):
        assert http(port, token, base + "/rights", request) == receipt
    for request, frozen in zip(fixture["freeze_requests"], fixture["snapshots"]):
        reopened = http(port, token, base + f"/snapshots/{frozen['id']}")
        assert immutable_snapshot(reopened) == immutable_snapshot(frozen)
        validate_document(reopened["document"])
        replayed = http(port, token, base + "/snapshots", request, 201)
        assert replayed == reopened
    for (snapshot_id, request), receipt in zip(fixture["approval_requests"], fixture["approval_receipts"]):
        assert http(port, token, base + f"/snapshots/{snapshot_id}/approvals", request) == receipt
    final = http(port, token, base + f"/snapshots/{fixture['final_snapshot_id']}")
    assert final["approval"] == fixture["final_approval"]
    assert final["eligibility"] == {"inputs_eligible": True, "approval_current": True,
                                   "billable_dispatch_available": False, "findings": []}
    assert row_counts(env, app_url, script) == fixture["counts"]
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    evidence = {key: value for key, value in fixture.items() if key not in ("base",)}
    evidence["provenance"] = "repository-authored synthetic facts; no LLM/TTS generation, legal verification, reservation, actual charge or payment"
    evidence["recovery"] = "actual HTTP process kill after committed snapshot/approval; PostgreSQL restart; migration replay; immutable receipt/document replay; live eligibility recheck"
    evidence["digest_oracle"] = "Python hashlib tagged SHA-256 over independently sorted compact UTF-8 scoped JSON"
    evidence["reads_after_restart"] = bounded_reads(env, app_url, port, token, base, final["id"], script)
    (EVIDENCE / "http-restart.json").write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
    (EVIDENCE / "ui-context.json").write_text(json.dumps({
        "script_id": script, "revision": fixture["second_revision"]["revision"],
        "settings_version": fixture["settings_receipts"][-1]["version"],
        "snapshot_id": final["id"], "input_digest": final["document"]["input_digest"],
        "inputs_eligible": True, "approval_current": True, "billable_dispatch_available": False,
        "fixture": "synthetic local logic evidence; no provider voice or legal consent claim"}, indent=2) + "\n")
    print("PASS: production settings/rights/review/freeze/approval/eligibility actual loopback HTTP + app-role PostgreSQL + independent p1 digest/Unicode exact estimate + missing/pending/revoked/expired rights + unknown/exceeded budget + stale settings/text + concurrent duplicate freeze + host kill + PostgreSQL restart + exact immutable replay + retired execution tables absent")
