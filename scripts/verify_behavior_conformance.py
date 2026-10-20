"""Private-admission ITF codec and differential observer gate, owned by cargo xtask.

This module decodes Quint expectations and compares independent Rust observations. It
never evaluates the model or decides an admission. Events alone enter the Rust observer.
"""

import os
import re
import shutil
import time
import tomllib
from pathlib import Path
from datetime import datetime, timezone

from verify_kani import FORBIDDEN_ENVIRONMENT, KaniError, bounded_run, configuration_identity

CAPABILITY = "cantos.media.private-audio-admission"
SCHEMA = "cantos-private-audio-admission-observation-v1"
SCRIPT = "scripts/verify_behavior_conformance.py"
OBSERVER = "services/cantos-server/src/contexts/media/quint_conformance_tests.rs"
TEST = "contexts::media::domain::lifecycle::tests::quint_conformance_tests::replay_quint_private_admission"
ERRORS = {"PermissionRevoked", "PermissionExpired", "InvalidExpiry", "StaleBrief", "DigestMismatch", "OperationReused"}
OWNERS = {"AwaitingBytes", "OwnerAdmitted", "Expired", "Revoked", "Stale"}
UNITS = {"#tup": []}
CONFIGURED_TOOL_SELECTION = {"PATH", "RUSTUP_TOOLCHAIN", "RUSTUP_HOME", "CARGO_HOME", "LD_LIBRARY_PATH", "DYLD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH"}


class ConformanceError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ConformanceError(message)


def record(value, keys, context):
    require(isinstance(value, dict) and set(value) == set(keys), "invalid " + context + " fields")
    return value


def integer(value, low=0, high=2**63 - 1):
    record(value, ["#bigint"], "ITF bigint")
    raw = value["#bigint"]
    require(isinstance(raw, str) and re.fullmatch(r"0|-?[1-9][0-9]{0,18}", raw), "malformed ITF bigint")
    parsed = int(raw)
    require(low <= parsed <= high, "ITF bigint outside registered range")
    return parsed


def boolean(value):
    require(type(value) is bool, "expected ITF Boolean")
    return value


def variant(value, tags):
    record(value, ["tag", "value"], "ITF variant")
    require(isinstance(value["tag"], str) and value["tag"] in tags, "unknown ITF variant")
    return value["tag"], value["value"]


def unit(value):
    require(value == UNITS and isinstance(value, dict), "invalid nullary ITF variant")
    return None


def request(value):
    record(value, ["digest", "length"], "ITF request")
    return {key: integer(value[key], 0, 1) for key in ("digest", "length")}


def event(value):
    tag, payload = variant(value, {"NoEvent", "Assertion", "Begin", "Complete", "Abandon", "Revoke", "Invalidate", "RestoreCurrent", "Clock"})
    if tag == "Begin":
        payload = request(payload)
    elif tag == "Complete":
        payload = boolean(payload)
    elif tag == "Clock":
        payload = integer(payload, 0, 4)
    else:
        payload = unit(payload)
    return {"tag": tag, "value": payload}


def outcome(value):
    tag, payload = variant(value, {"NoOutcome", "Inspect", "Replay", "Admitted", "Rejected", "MediaRejected", "Cancelled", "FactsChanged"})
    if tag == "Rejected":
        payload, empty = variant(payload, ERRORS)
        unit(empty)
    else:
        payload = unit(payload)
    return {"tag": tag, "value": payload}


def state(value):
    record(value, ["facts", "receipt", "pending", "inspections", "admissions", "ownerView"], "ITF state")
    facts = record(value["facts"], ["now", "revoked", "current"], "ITF facts")
    facts = {"now": integer(facts["now"], 0, 4), "revoked": boolean(facts["revoked"]), "current": boolean(facts["current"])}
    tag, receipt = variant(value["receipt"], {"Unsettled", "Settled"})
    if tag == "Unsettled":
        receipt = unit(receipt)
    else:
        record(receipt, ["id", "admittedAt", "digest", "length"], "ITF receipt")
        receipt = {"id": integer(receipt["id"]), "admittedAt": integer(receipt["admittedAt"], 0, 4),
                   "digest": integer(receipt["digest"], 0, 1), "length": integer(receipt["length"], 0, 1)}
    tag, pending = variant(value["pending"], {"Idle", "Inspecting"})
    pending = unit(pending) if tag == "Idle" else request(pending)
    owner, empty = variant(value["ownerView"], OWNERS)
    unit(empty)
    # Mutant traces may legitimately violate the original invariant. Decode, then
    # detect semantic drift by observation rather than counting a parser error as a kill.
    return {"facts": facts, "receipt": receipt, "pending": pending,
            "inspections": integer(value["inspections"]), "admissions": integer(value["admissions"]), "ownerView": owner}


def decode_trace(raw):
    require(isinstance(raw, dict) and raw.get("vars") == ["state", "before", "last", "outcome"], "unsupported ITF variable representation")
    checkpoints = raw.get("states")
    require(isinstance(checkpoints, list) and 2 <= len(checkpoints) <= 101, "empty or oversized ITF trace")
    result = []
    previous = None
    for index, checkpoint in enumerate(checkpoints):
        record(checkpoint, ["#meta", "state", "before", "last", "outcome"], "ITF checkpoint")
        meta = record(checkpoint["#meta"], ["index"], "ITF checkpoint metadata")
        require(type(meta["index"]) is int and meta["index"] == index, "missing or reordered ITF checkpoint")
        expected, before = state(checkpoint["state"]), state(checkpoint["before"])
        action, result_outcome = event(checkpoint["last"]), outcome(checkpoint["outcome"])
        require((action["tag"] == "NoEvent") == (index == 0), "NoEvent only valid at initial checkpoint")
        require(before == (expected if index == 0 else previous), "ITF before checkpoint is not prior state")
        if action["tag"] in ("NoEvent", "Assertion"):
            require(result_outcome == {"tag": "NoOutcome", "value": None}, "effectful assertion/initial outcome")
        if action["tag"] == "Assertion":
            require(expected == before, "effectful assertion checkpoint")
        result.append({"index": index, "event": action, "state": expected, "outcome": result_outcome})
        previous = expected
    return result


def snapshot(root, behavior):
    """Pin implementation, fixture, compiler configuration and comparison inputs."""
    paths = {SCRIPT, "scripts/test_verify_behavior_conformance.py", "Cargo.toml", "Cargo.lock", ".cargo/config.toml", "rust-toolchain.toml"}
    for directory in ("services/cantos-server", "packages/contracts/api"):
        paths.add(directory + "/Cargo.toml")
        paths.update(str(path.relative_to(root)) for path in (root / directory / "src").rglob("*.rs"))
    paths.update(str(path.relative_to(root)) for path in (root / "packages/contracts/fixtures/briefs").rglob("*.json"))
    paths.update(str(path.relative_to(root)) for path in (root / "packages/contracts/schema").rglob("*.json"))
    for relative in ("services/cantos-server/build.rs", "packages/contracts/api/build.rs", "rust-toolchain", "rust-toolchain.toml"):
        if (root / relative).exists():
            paths.add(relative)
    return {path: behavior.digest(behavior.local_path(root, path)) for path in sorted(paths)}


def collect_cases(root, cap, bundle, report, inputs, versions, revision, behavior):
    require(cap["id"] == CAPABILITY and cap.get("observationSchema") == SCHEMA and
            cap.get("implementationConformance") == "pure-core-observer", "unregistered implementation observation adapter/schema")
    require(report.get("schemaVersion") == 1 and report.get("capability") == cap["id"] and
            report.get("scope") == "model-only" and report.get("status") == "PASS", "model evidence did not pass independently")
    require(report.get("sourceRevision") == revision and report.get("inputs") == inputs and
            report.get("tools") == versions and report.get("budget") == cap["simulation"], "stale model/catalog/tool/config/input identity")
    require(report.get("selection") == {key: cap[key] for key in ("properties", "runs", "witnesses")} and
            report.get("controlSelection") == [{"id": row["id"], "property": row["property"]} for row in cap["controls"]], "stale scenario/control selection")
    require(set(report.get("stages", {})) == {"typecheck", "scenarios", "simulation", "witnesses", "negativeControls", "freshness"} and
            set(report["stages"].values()) == {"PASS"}, "partial model stage evidence")
    require(report.get("observationVariables") == sorted(["state", "before", "last", "outcome"]),
            "model variables do not match the registered observation schema")
    require(all(behavior.digest(behavior.local_path(bundle / "inputs", path)) == sha for path, sha in inputs.items()), "stale retained model inputs")
    receipt = report.get("installationReceipt", {})
    require(receipt.get("file") == "installation.json" and receipt.get("sha256") == versions["installationReceiptSha256"] and
            behavior.digest(bundle / "installation.json") == receipt["sha256"], "stale retained setup receipt")
    traces = report.get("traces", {})
    expected_keys = {row["declaration"] for row in cap["runs"]} | {"witness-" + row["declaration"] for row in cap["witnesses"]} | {"properties"} | {"control-" + str(n + 1) for n in range(len(cap["controls"]))}
    require(set(traces) == expected_keys, "missing, duplicated or unregistered trace selection")
    cases = []

    def add(identity, evidence, source, status, sensitivity):
        require(isinstance(evidence, dict), "invalid trace evidence")
        path = behavior.local_path(bundle, evidence.get("file"))
        require(behavior.digest(path) == evidence.get("sha256"), "stale raw ITF hash")
        behavior.validate_itf(path, source, status, expected_variables=report.get("observationVariables"))
        checkpoints = decode_trace(behavior.read_json(path))
        cases.append({"id": identity, "checkpoints": checkpoints, "sensitivity": sensitivity,
                      "source": str(source), "file": path.name, "sha256": evidence["sha256"], "seed": evidence.get("seed", cap["simulation"]["seed"])})

    for row in cap["runs"]:
        evidence = traces[row["declaration"]]
        require(isinstance(evidence, list) and len(evidence) == cap["simulation"]["scenarioSamples"], "empty/duplicate named trace selection")
        for item in evidence:
            add(row["id"], item, root / cap["scenarios"], "passed", False)
    add(cap["id"] + ".sample.properties", traces["properties"], root / cap["model"], "ok", False)
    for row in cap["witnesses"]:
        add(row["id"], traces["witness-" + row["declaration"]], root / cap["model"], "violation", False)
    model = behavior.regular(bundle / "inputs" / cap["model"]).decode()
    for index, row in enumerate(cap["controls"]):
        label = "control-" + str(index + 1)
        mutant = bundle / (label + ".qnt")
        require(behavior.regular(mutant).decode() == model.replace(row["from"], row["to"], 1), "stale mutant source identity")
        add(row["id"], traces[label], mutant, "violation", True)
    require(bool(cases) and len({case["id"] for case in cases}) == len(cases), "empty or duplicate conformance cases")
    return cases


def replay_input(cases):
    return {"schemaVersion": 1, "observationSchema": SCHEMA, "traces": [
        {"id": case["id"], "checkpoints": [{"index": step["index"], "event": step["event"]} for step in case["checkpoints"]]}
        for case in cases]}


def observed_state(value):
    record(value, ["facts", "receipt", "pending", "inspections", "admissions", "ownerView"], "observer state")
    facts = record(value["facts"], ["now", "revoked", "current"], "observer facts")
    require(type(facts["now"]) is int and 0 <= facts["now"] <= 4 and
            type(facts["revoked"]) is bool and type(facts["current"]) is bool, "invalid observer facts")
    for key in ("inspections", "admissions"):
        require(type(value[key]) is int and 0 <= value[key] <= 2**63 - 1, "invalid observer count")
    require(value["ownerView"] in OWNERS, "unknown observer owner projection")
    for key in ("pending", "receipt"):
        item = value[key]
        if item is None:
            continue
        keys = ["digest", "length"] if key == "pending" else ["id", "admittedAt", "digest", "length"]
        record(item, keys, "observer " + key)
        require(all(type(item[field]) is int and 0 <= item[field] <= 1 for field in ("digest", "length")), "invalid observer payload aliases")
        if key == "receipt":
            require(type(item["id"]) is int and item["id"] >= 0 and type(item["admittedAt"]) is int and
                    0 <= item["admittedAt"] <= 4, "invalid observer receipt aliases")


def observed_outcome(value):
    record(value, ["tag", "value"], "observer outcome")
    require(value["tag"] in {"NoOutcome", "Inspect", "Replay", "Admitted", "Rejected", "MediaRejected", "Cancelled", "FactsChanged"}, "unknown observer outcome")
    require(value["value"] in ERRORS if value["tag"] == "Rejected" else value["value"] is None, "invalid observer outcome payload")


def compare(cases, observed):
    record(observed, ["schemaVersion", "observationSchema", "traces"], "observer envelope")
    require(observed["schemaVersion"] == 1 and observed["observationSchema"] == SCHEMA, "unsupported observer schema")
    rows = observed["traces"]
    require(isinstance(rows, list) and [row.get("id") for row in rows] == [case["id"] for case in cases], "missing/reordered observer traces")
    mismatches = []
    for case, row in zip(cases, rows):
        record(row, ["id", "checkpoints"], "observer trace")
        checkpoints = row["checkpoints"]
        require(isinstance(checkpoints, list) and len(checkpoints) == len(case["checkpoints"]), "missing observer checkpoints")
        initial_submission, settled_document = None, None
        calls = 0
        first = None
        for expected, actual in zip(case["checkpoints"], checkpoints):
            record(actual, ["index", "state", "outcome", "submission", "rendition", "decisionCalls"], "observer checkpoint")
            index = expected["index"]
            require(type(actual["index"]) is int and actual["index"] == index, "reordered observer checkpoints")
            calls += expected["event"]["tag"] in ("Begin", "Complete")
            require(type(actual["decisionCalls"]) is int and actual["decisionCalls"] == calls, "missing/duplicate production decision calls, including assertion effects")
            require(isinstance(actual["submission"], dict) and bool(actual["submission"]), "missing real submission receipt")
            if initial_submission is None:
                initial_submission = actual["submission"]
            require(actual["submission"] == initial_submission, "implementation rewrote immutable submission receipt")
            document = actual["rendition"]
            if document is not None:
                require(isinstance(document, dict) and bool(document), "missing real rendition receipt")
                if settled_document is None:
                    settled_document = document
                require(document == settled_document, "implementation rewrote immutable rendition receipt")
            elif settled_document is not None:
                require(False, "implementation erased settled rendition receipt")
            observed_state(actual["state"])
            observed_outcome(actual["outcome"])
            require((actual["state"]["receipt"] is None) == (document is None), "insufficient receipt observations")
            if first is None and (actual["state"] != expected["state"] or actual["outcome"] != expected["outcome"]):
                first = {"id": case["id"], "index": index, "expected": {key: expected[key] for key in ("state", "outcome")},
                         "observed": {key: actual[key] for key in ("state", "outcome")}}
        if first is not None:
            mismatches.append(first)
    return mismatches


def compiler_override_environment(key):
    return key in FORBIDDEN_ENVIRONMENT or (key.startswith("CARGO_TARGET_") and
           key.endswith(("_RUSTFLAGS", "_RUNNER", "_LINKER")))


def observer_environment(behavior, input_path, output_path, fault=None):
    env = behavior.child_environment()
    forbidden = sorted(key for key, value in env.items() if value and (compiler_override_environment(key)))
    require(not forbidden, "unreviewed implementation compiler environment: " + ", ".join(forbidden))
    for key in ("CANTOS_QUINT_REPLAY_INPUT", "CANTOS_QUINT_REPLAY_OUTPUT", "CANTOS_QUINT_OBSERVER_FAULT"):
        require(not env.get(key), "ambient observer configuration is not permitted")
    env.update({"CANTOS_QUINT_REPLAY_INPUT": str(input_path), "CANTOS_QUINT_REPLAY_OUTPUT": str(output_path)})
    if fault:
        env["CANTOS_QUINT_OBSERVER_FAULT"] = fault
    return env


def compiler_configuration(root, behavior):
    try:
        identity = configuration_identity(root, os.environ)
    except KaniError as error:
        raise ConformanceError("compiler configuration identity: " + str(error)) from error
    for path, sha in identity.items():
        if sha is None or Path(path).name == "Cargo.toml":
            continue
        data = tomllib.loads(behavior.regular(Path(path)).decode())
        build = data.get("build", {})
        require(not any(key in build for key in ("rustflags", "rustc", "rustc-wrapper", "rustc-workspace-wrapper", "target")), "Cargo compiler override configuration is not permitted")
        targets = data.get("target", {})
        require(not any(isinstance(target, dict) and any(key in target for key in ("rustflags", "runner", "linker")) for target in targets.values()), "Cargo target compiler override is not permitted")
        environment = data.get("env", {})
        require(not any(compiler_override_environment(key) or key in CONFIGURED_TOOL_SELECTION or key.startswith("CANTOS_QUINT_") for key in environment), "Cargo configured environment overrides the observation boundary")
    return identity


def implementation_tools(root, behavior):
    try:
        pin = tomllib.loads(behavior.regular(root / "rust-toolchain.toml").decode())["toolchain"]["channel"]
    except (KeyError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise ConformanceError("invalid production toolchain pin") from error
    require(isinstance(pin, str) and re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", pin), "implementation observer requires an exact stable toolchain pin")
    identity = {}
    for name in ("cargo", "rustc"):
        executable = shutil.which(name)
        require(executable is not None, "missing " + name + " for independent implementation observer")
        args = [executable, "--version"] + (["--verbose"] if name == "rustc" else [])
        probe = behavior.process(args, root)
        require(probe.returncode == 0 and re.match(re.escape(name + " " + pin) + r"(?: |$)", probe.stdout), "implementation observer requires " + name + " " + pin)
        identity[name] = {"invocation": executable, "version": probe.stdout.strip(),
                          "launcherSha256": behavior.digest(Path(executable).resolve(), 256 * 1024 * 1024)}
    sysroot = behavior.process([identity["rustc"]["invocation"], "--print", "sysroot"], root)
    require(sysroot.returncode == 0 and Path(sysroot.stdout.strip()).is_absolute(), "missing Rust sysroot identity")
    sysroot = Path(sysroot.stdout.strip())
    for name in ("cargo", "rustc"):
        binary = (sysroot / "bin" / name).resolve()
        identity[name].update({"toolchainBinary": str(binary), "binarySha256": behavior.digest(binary, 256 * 1024 * 1024)})
    drivers = sorted((sysroot / "lib").glob("*rustc_driver*"))
    require(bool(drivers), "missing compiler driver identity")
    identity["compilerDrivers"] = {str(path): behavior.digest(path.resolve(), 256 * 1024 * 1024) for path in drivers}
    return identity


def execute(root, cap, bundle, report, versions, inputs, revision, behavior):
    result = {"schemaVersion": 1, "observationSchema": SCHEMA, "scope": "pure-core-synthetic-shell",
              "modelReportSha256": behavior.digest(bundle / "report.json"), "sourceRevision": revision,
              "implementationInputs": {}, "status": "NOT_RUN", "commands": [],
              "startedAt": datetime.now(timezone.utc).isoformat(),
              "stages": {key: "NOT_RUN" for key in ("identity", "originalTraces", "mutantSensitivity", "observerSensitivity", "freshness")},
              "DB/API/authorization/media/storage/client/device": "NOT_RUN", "solverProof": "NOT_RUN"}
    stage = "identity"
    target = bundle / "conformance.json"

    def invoke(label, cases, fault=None):
        input_path, output_path, log = (bundle / (label + suffix) for suffix in ("-input.json", "-observations.json", ".log"))
        behavior.write_json(input_path, replay_input(cases))
        env = observer_environment(behavior, input_path, output_path, fault)
        cargo = result["implementationTools"]["cargo"]["toolchainBinary"]
        env["RUSTC"] = result["implementationTools"]["rustc"]["toolchainBinary"]
        command = [cargo, "test", "-p", "cantos-server", "--lib", "--locked", "--offline", TEST, "--", "--ignored", "--exact", "--nocapture"]
        entry = {"label": label, "argv": command, "cwd": str(root), "timeoutSeconds": 180,
                 "inputSha256": behavior.digest(input_path), "fault": fault, "rustc": env["RUSTC"]}
        result["commands"].append(entry)
        behavior.write_json(target, result)
        start = time.monotonic()
        try:
            completed = bounded_run(command, root, env, 180, log)
            entry.update({"exit": completed.returncode, "durationSeconds": round(time.monotonic() - start, 3)})
            require(completed.returncode == 0 and re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", completed.stdout), "observer failed or selected zero/incorrect tests")
            data = behavior.read_json(output_path)
            entry["outputSha256"] = behavior.digest(output_path)
            return compare(cases, data)
        except KaniError as error:
            raise ConformanceError("implementation observer process: " + str(error)) from error
        finally:
            if log.exists():
                entry["logSha256"] = behavior.digest(log)
            behavior.write_json(target, result)

    try:
        result["implementationInputs"] = snapshot(root, behavior)
        result["compilerConfiguration"] = compiler_configuration(root, behavior)
        result["implementationTools"] = implementation_tools(root, behavior)
        cases = collect_cases(root, cap, bundle, report, inputs, versions, revision, behavior)
        behavior.retain_inputs(root, bundle, result["implementationInputs"])
        result["traces"] = [{key: case[key] for key in ("id", "source", "file", "sha256", "seed", "sensitivity")} for case in cases]
        result["stages"][stage] = "PASS"
        positive = [case for case in cases if not case["sensitivity"]]
        negative = [case for case in cases if case["sensitivity"]]
        stage = "originalTraces"
        mismatch = invoke("original", positive)
        result["originalMismatches"] = mismatch
        require(not mismatch, "actual implementation diverges from original Quint expectations")
        result["positiveTraces"] = len(positive)
        result["positiveCheckpoints"] = sum(len(case["checkpoints"]) for case in positive)
        result["stages"][stage] = "PASS"
        stage = "mutantSensitivity"
        # A faulty model can enable an event that is illegal after the correct core
        # rejected its predecessor. Replay growing prefixes and stop only at a
        # genuine state/outcome mismatch, never at a parser/runtime/guard failure.
        pending, mismatch, length = list(negative), [], 2
        while pending:
            prefixes = [{**case, "checkpoints": case["checkpoints"][:length]} for case in pending]
            found = invoke("mutants-prefix-" + str(length), prefixes)
            mismatch.extend(found)
            killed = {row["id"] for row in found}
            pending = [case for case in pending if case["id"] not in killed]
            require(all(length < len(case["checkpoints"]) for case in pending), "correct observer did not distinguish a deliberately wrong model control")
            length += 1
        result["mutantMismatches"] = mismatch
        require({row["id"] for row in mismatch} == {case["id"] for case in negative}, "incomplete semantic model sensitivity")
        result["stages"][stage] = "PASS"
        stage = "observerSensitivity"
        fault_cases = [case for case in positive if case["id"] in {CAPABILITY + ".run.revoke-during-inspection", CAPABILITY + ".run.revoked-replay"}]
        require(len(fault_cases) == 2, "missing observer-fault witness scenarios")
        mismatch = invoke("fault-ignore-revocation", fault_cases, "ignore-revocation")
        result["observerFaultMismatches"] = mismatch
        require(CAPABILITY + ".run.revoke-during-inspection" in {row["id"] for row in mismatch} and
                CAPABILITY + ".run.revoked-replay" in {row["id"] for row in mismatch}, "revocation input adapter defect escaped original trace assertions")
        result["stages"][stage] = "PASS"
        stage = "freshness"
        require(compiler_configuration(root, behavior) == result["compilerConfiguration"] and
                implementation_tools(root, behavior) == result["implementationTools"], "compiler tool/configuration changed during conformance")
        require(snapshot(root, behavior) == result["implementationInputs"] and behavior.snapshot(root, [cap]) == inputs, "implementation/model inputs changed during conformance")
        require(behavior.installed_tool(root)[0] == versions, "installed model tool changed during conformance")
        head = behavior.process(["git", "rev-parse", "HEAD"], root)
        require(head.returncode == 0 and head.stdout.strip() == revision, "implementation revision changed during conformance")
        result["stages"][stage] = "PASS"
        result["status"] = "PASS"
    except (ValueError, OSError, KeyError, TypeError, behavior.BehaviorError) as error:
        result["stages"][stage] = "FAIL"
        result.update({"status": "FAIL", "error": str(error)})
        raise ConformanceError(str(error)) from error
    finally:
        result["finishedAt"] = datetime.now(timezone.utc).isoformat()
        behavior.write_json(target, result)
        print(f"implementation-conformance: {result['status']} (pure-core-synthetic-shell); evidence: {target.relative_to(root)}")
    return result
