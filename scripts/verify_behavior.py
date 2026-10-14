#!/usr/bin/env python3
"""Narrow locked Quint model gate; cargo xtask is the public command surface.

No model evaluation, transport codec, implementation reducer, solver or service lives
here. Quint owns parsing/type/effect checking and executable expected outcomes.
"""

import argparse
import base64
import hashlib
import json
import os
import re
import shutil
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path
from urllib.parse import urlsplit

# Reuse the existing tested POSIX boundary, including pinned-leader group cleanup.
from verify_kani import KaniError as ProcessError, bounded_run

ROOT = Path(__file__).resolve().parents[1]
CATALOG = "specs/behavior/catalog.json"
TOOL_DIR = "tools/behavior"
CLI = "node_modules/@informalsystems/quint/dist/src/cli.js"
INSTALLATION = "target/verification/behavior/installation.json"
SETUP_INPUTS = ("package.json", "package-lock.json", ".npmrc", ".node-version",
                "empty-user.npmrc", "empty-global.npmrc")
MAX_BYTES = 4 * 1024 * 1024
NAME = re.compile(r"^[A-Za-z][A-Za-z0-9_]*$")
STABLE_ID = re.compile(r"^[a-z][a-z0-9]*(?:[.-][a-z0-9]+)*$")
LIFECYCLES = {"draft", "accepted", "deprecated", "needs-decision"}


class BehaviorError(Exception):
    pass


def regular(path, limit=MAX_BYTES):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise BehaviorError(f"missing, linked, special or oversized input: {path.name}")
    return path.read_bytes()


def digest(path, limit=MAX_BYTES):
    return hashlib.sha256(regular(path, limit)).hexdigest()


def read_json(path):
    try:
        return json.loads(regular(path))
    except (ValueError, UnicodeError) as error:
        raise BehaviorError(f"invalid JSON: {path.name}") from error


def local_path(root, relative):
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise BehaviorError("invalid repository-relative input path")
    plain = relative.split("#", 1)[0]
    if any(part in ("", ".", "..") for part in plain.split("/")):
        raise BehaviorError("input path must be normalized and repository-relative")
    path = Path(plain)
    if path.is_absolute() or any(p in (".", "..") for p in path.parts):
        raise BehaviorError("input path escapes the repository")
    result = root / path
    if not result.resolve().is_relative_to(root.resolve()):
        raise BehaviorError("input path escapes the repository")
    for ancestor in (result, *result.parents):
        if ancestor == root:
            break
        if ancestor.is_symlink():
            raise BehaviorError("linked input paths are not supported")
    regular(result)
    return result


def require(condition, message):
    if not condition:
        raise BehaviorError(message)


def entries(capability, kind, seen):
    rows = capability.get(kind)
    require(isinstance(rows, list) and rows, f"{kind} must be nonempty")
    names = set()
    for row in rows:
        require(isinstance(row, dict), "catalog declaration row must be an object")
        identity, declaration = row.get("id"), row.get("declaration")
        require(isinstance(identity, str) and STABLE_ID.fullmatch(identity) and identity not in seen,
                "invalid or duplicate stable ID")
        require(identity.startswith(capability["id"] + "."), "ID is outside its capability")
        require(isinstance(declaration, str) and NAME.fullmatch(declaration) and declaration not in names,
                "invalid or duplicate Quint declaration")
        seen.add(identity)
        names.add(declaration)
    return rows


def load_catalog(root, selected=None):
    catalog = read_json(local_path(root, CATALOG))
    require(isinstance(catalog, dict), "catalog must be an object")
    require(catalog.get("schemaVersion") == 1, "unsupported behavior catalog schema")
    capabilities = catalog.get("capabilities")
    require(isinstance(capabilities, list) and capabilities, "empty capability catalog")
    seen, runnable = set(), []
    for cap in capabilities:
        require(isinstance(cap, dict), "capability must be an object")
        identity = cap.get("id")
        require(isinstance(identity, str) and STABLE_ID.fullmatch(identity) and identity not in seen,
                "invalid or duplicate capability ID")
        seen.add(identity)
        require(cap.get("lifecycle") in LIFECYCLES, "invalid capability lifecycle")
        local_path(root, cap.get("owner"))
        local_path(root, cap.get("inventory"))
        refs = cap.get("sourceRefs")
        require(isinstance(refs, list) and refs, "requirements must be linked")
        for ref in refs:
            local_path(root, ref)
        if cap.get("model") is None:
            require(bool(cap.get("reason")), "unmodeled capability needs a reason")
            continue
        require(cap["lifecycle"] in ("draft", "accepted"), "unresolved or deprecated model cannot gate")
        local_path(root, cap["model"])
        local_path(root, cap.get("scenarios"))
        for key in ("module", "scenarioModule"):
            require(isinstance(cap.get(key), str) and NAME.fullmatch(cap[key]), "invalid Quint module")
        props = entries(cap, "properties", seen)
        entries(cap, "runs", seen)
        entries(cap, "witnesses", seen)
        budget = cap.get("simulation", {})
        require(isinstance(budget, dict), "simulation budget must be an object")
        require(budget.get("backend") == "typescript", "only the locked TypeScript simulator is supported")
        require(isinstance(budget.get("seed"), str) and re.fullmatch(r"[0-9]{1,15}", budget["seed"]),
                "seed must be a fixed decimal integer")
        for key, upper in (("samples", 10000), ("steps", 100), ("scenarioSamples", 1),
                           ("witnessSamples", 10000), ("timeoutSeconds", 120)):
            value = budget.get(key)
            require(type(value) is int and 1 <= value <= upper, f"invalid {key} budget")
        controls = cap.get("controls")
        require(isinstance(controls, list) and len(controls) == len(props), "every property needs one control")
        covered = set()
        model = regular(root / cap["model"]).decode("utf-8")
        for control in controls:
            require(isinstance(control, dict), "control row must be an object")
            identity = control.get("id")
            require(isinstance(identity, str) and STABLE_ID.fullmatch(identity) and identity not in seen,
                    "invalid or duplicate control ID")
            require(identity.startswith(cap["id"] + "."), "control ID is outside its capability")
            seen.add(identity)
            require(control.get("property") in {p["id"] for p in props}, "control targets an unknown property")
            require(control["property"] not in covered, "duplicate property control")
            covered.add(control["property"])
            before, after = control.get("from"), control.get("to")
            require(isinstance(before, str) and before and model.count(before) == 1,
                    "negative substitution must match exactly once")
            require(isinstance(after, str) and after and after != before, "empty or inert negative substitution")
        runnable.append(cap)
    if selected is not None:
        require(selected in {c["id"] for c in capabilities}, "unknown capability ID")
        runnable = [cap for cap in runnable if cap["id"] == selected]
    require(bool(runnable), "selected capability has no executable reviewed-scope model")
    return runnable


def child_environment():
    require(not os.environ.get("NODE_OPTIONS"), "unreviewed Node preload/options environment")
    env = dict(os.environ)
    # Never resolve a missing locked dependency through ambient global modules.
    env.pop("NODE_PATH", None)
    env.update({"NO_COLOR": "1", "LC_ALL": "C", "TZ": "UTC",
                "NPM_CONFIG_UPDATE_NOTIFIER": "false", "NPM_CONFIG_USERCONFIG": str(ROOT / TOOL_DIR / "empty-user.npmrc"),
                "NPM_CONFIG_GLOBALCONFIG": str(ROOT / TOOL_DIR / "empty-global.npmrc")})
    return env


def process(command, cwd, timeout=10, log=None):
    try:
        return bounded_run([str(x) for x in command], cwd, child_environment(), timeout, log)
    except ProcessError as error:
        raise BehaviorError(f"behavior tool: {error}") from error


def runtime(root):
    manifest = read_json(local_path(root, TOOL_DIR + "/package.json"))
    versions, executables = {}, {}
    for tool in ("node", "npm"):
        executable = shutil.which(tool)
        require(executable is not None, f"missing {tool}; install the pinned development toolchain explicitly")
        result = process([executable, "--version"], root)
        match = re.search(r"(?:^|\n)v?(\d+\.\d+\.\d+)\s*$", result.stdout)
        require(result.returncode == 0 and match is not None, f"{tool} version probe failed")
        version = match.group(1)
        require(version == manifest["engines"][tool], f"expected {tool} {manifest['engines'][tool]}, found {version}")
        versions[tool], executables[tool] = version, str(Path(executable).resolve())
    require(manifest.get("packageManager") == "npm@" + versions["npm"], "npm pin disagrees")
    require(regular(local_path(root, TOOL_DIR + "/.node-version")).decode().strip() == versions["node"], "Node pin disagrees")
    return manifest, versions, executables


def validate_lock(manifest, lock):
    require(isinstance(lock, dict) and lock.get("lockfileVersion") == 3, "unsupported npm lock")
    packages = lock.get("packages")
    require(isinstance(packages, dict) and "" in packages, "missing locked root package")
    require(lock.get("name") == manifest.get("name") and lock.get("version") == manifest.get("version"),
            "lock root identity disagrees")
    require(packages[""]["devDependencies"] == manifest["devDependencies"], "manifest/lock disagree")
    expected = {k: v for k, v in packages.items() if k}
    require(bool(expected), "empty dependency lock")
    for relative, metadata in expected.items():
        require(isinstance(relative, str) and relative.startswith("node_modules/") and
                all(part not in ("", ".", "..") for part in relative.split("/")) and "\\" not in relative,
                "invalid locked package path")
        require(isinstance(metadata, dict) and isinstance(metadata.get("version"), str), "missing locked package version")
        resolved = metadata.get("resolved")
        require(isinstance(resolved, str), "missing locked package origin")
        origin = urlsplit(resolved)
        require(origin.scheme == "https" and origin.netloc == "registry.npmjs.org" and
                not origin.query and not origin.fragment and origin.path.endswith(".tgz"),
                "dependency is not from the official npm registry")
        integrity = metadata.get("integrity")
        require(isinstance(integrity, str) and integrity.startswith("sha512-"), "dependency lacks SHA-512 integrity")
        try:
            checksum = base64.b64decode(integrity[7:], validate=True)
        except ValueError as error:
            raise BehaviorError("invalid locked dependency integrity") from error
        require(len(checksum) == 64, "invalid locked dependency integrity")
    quint = expected.get("node_modules/@informalsystems/quint", {})
    require(quint.get("version") == manifest["devDependencies"].get("@informalsystems/quint"), "Quint lock pin disagrees")
    return expected


def setup_inputs(root):
    return {name: digest(local_path(root, TOOL_DIR + "/" + name)) for name in SETUP_INPUTS}


def validate_installation(receipt, inputs, versions):
    require(isinstance(receipt, dict) and receipt.get("schemaVersion") == 1 and
            receipt.get("inputs") == inputs and receipt.get("tools") == versions,
            "installed tool/config differs from explicit setup receipt; rerun --setup")


def installed_tool(root, require_receipt=True):
    manifest, versions, executables = runtime(root)
    tool = root / TOOL_DIR
    lock = read_json(local_path(root, TOOL_DIR + "/package-lock.json"))
    expected = validate_lock(manifest, lock)
    installed = read_json(local_path(root, TOOL_DIR + "/node_modules/.package-lock.json"))
    require(installed.get("lockfileVersion") == 3, "unsupported installed npm lock")
    require(set(installed["packages"]) == set(expected), "installed dependency set differs from lock; rerun --setup")
    for relative, metadata in expected.items():
        actual = installed["packages"][relative]
        require(all(actual.get(k) == metadata.get(k) for k in ("version", "resolved", "integrity")), "installed lock is stale")
        require(read_json(local_path(tool, relative + "/package.json"))["version"] == metadata["version"], "installed package version differs")
    version = manifest["devDependencies"].get("@informalsystems/quint")
    require(isinstance(version, str) and re.fullmatch(r"\d+\.\d+\.\d+", version), "Quint must be exactly pinned")
    cli = local_path(tool, CLI)
    versions["quint"] = version
    files = {}
    total_bytes = 0
    for path in sorted((tool / "node_modules").rglob("*")):
        if path.is_symlink():
            require(path.parent.name == ".bin", "linked dependency input")
        elif path.is_file():
            total_bytes += path.stat().st_size
            require(total_bytes <= 128 * 1024 * 1024 and len(files) < 10000, "installed tool exceeds its input bound")
            files[str(path.relative_to(tool))] = digest(path, 32 * 1024 * 1024)
    versions["installedTreeSha256"] = hashlib.sha256(json.dumps(files, sort_keys=True).encode()).hexdigest()
    versions["executableSha256"] = {key: digest(Path(value), 128 * 1024 * 1024) for key, value in executables.items()}
    versions["cliSha256"] = digest(cli)
    # Check installed content before executing Quint, not merely after recording its hash.
    if require_receipt:
        receipt = local_path(root, INSTALLATION)
        validate_installation(read_json(receipt), setup_inputs(root), versions)
        versions["installationReceiptSha256"] = digest(receipt)
    result = process([executables["node"], cli, "--version"], root)
    require(result.returncode == 0 and result.stdout.strip() == version, "installed Quint version differs")
    return versions, executables, cli


def validate_types(data, module, declarations, kind="predicate"):
    require(data.get("stage") == "typechecking" and data.get("errors") == [], "Quint type/effect checks did not complete")
    modules = {m["name"]: m for m in data.get("modules", [])}
    require(module in modules, "Quint checked the wrong module")
    defs = {d.get("name"): d for d in modules[module]["declarations"] if d.get("kind") == "def"}
    for name in declarations:
        require(name in defs, f"missing selected declaration: {name}")
        declaration = defs[name]
        inferred = data.get("types", {}).get(str(declaration["id"]), {}).get("type", {})
        require(inferred.get("kind") == "bool", "selected declaration is not a Boolean")
        qualifiers = ("run",) if kind == "run" else ("val", "pureval")
        require(declaration.get("qualifier") in qualifiers, "selected declaration has the wrong execution mode")


def validate_scenarios(data, expected):
    require(data.get("stage") == "testing" and data.get("errors") == [] and
            data.get("failed") == [] and data.get("ignored") == [], "named scenarios failed, errored or were skipped")
    require(sorted(data.get("passed", [])) == sorted(expected), "named scenarios missing, duplicated or unregistered")


def validate_simulation(data, returncode, violation, steps):
    expected_status = "violation" if violation else "ok"
    require(returncode == (1 if violation else 0), "unexpected Quint process exit")
    require(data.get("stage") == "running" and data.get("status") == expected_status and
            data.get("errors") == [], "missing actual invariant outcome; errors do not kill a control")
    trace = data.get("trace")
    require(isinstance(trace, list) and len(trace) >= 2 and type(data.get("seed")) is int,
            "empty trace or missing concrete trace seed")
    require(violation or len(trace) == steps + 1, "sample campaign ended before its requested step bound")


def validate_itf(path, source, status, count=None):
    data = read_json(path)
    meta = data.get("#meta", {})
    require(meta.get("format") == "ITF" and meta.get("source") == str(source) and meta.get("status") == status,
            "ITF source/status is not bound to this command")
    variables = data.get("vars")
    require(isinstance(variables, list) and len(variables) == 4 and
            set(variables) == {"state", "before", "last", "outcome"}, "wrong ITF observation variables")
    states = data.get("states")
    require(isinstance(states, list) and len(states) >= 2, "empty or missing ITF counterexample")
    require(count is None or len(states) == count, "ITF/result checkpoint count differs")
    for index, state in enumerate(states):
        require(isinstance(state, dict) and all(v in state for v in variables), "incomplete ITF observations")
        observed = state.get("#meta", {}).get("index")
        require(type(observed) is int and observed == index, "ITF checkpoint indices are missing or reordered")
    return digest(path)


def snapshot(root, capabilities):
    paths = {CATALOG, "scripts/verify_behavior.py", "scripts/verify_kani.py",
             "xtask/src/behavior.rs", "xtask/src/gate.rs", "xtask/src/cli.rs", "Cargo.lock"}
    paths.update(str(p.relative_to(root)) for p in (root / TOOL_DIR).iterdir() if p.is_file())
    paths.update(str(p.relative_to(root)) for p in (root / "specs/behavior").rglob("*.qnt"))
    for cap in capabilities:
        paths.update([cap["model"], cap["scenarios"]])
        paths.update([cap["owner"], cap["inventory"], *[ref.split("#", 1)[0] for ref in cap["sourceRefs"]]])
    return {p: digest(local_path(root, p)) for p in sorted(paths)}


def write_json(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def retain_inputs(root, bundle, inputs):
    """Preserve exact dirty bytes, not only hashes that cannot reconstruct them."""
    for relative, expected in inputs.items():
        contents = regular(local_path(root, relative))
        require(hashlib.sha256(contents).hexdigest() == expected,
                "behavior source changed before input retention")
        target = bundle / "inputs" / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(contents)
        require(digest(target) == expected, "retained behavior input differs")


def artifact_root(root):
    parent = root / "target/verification/behavior"
    require(parent.resolve().is_relative_to(root.resolve()) and
            not any(path.is_symlink() for path in (parent, parent.parent, parent.parent.parent)),
            "linked artifact root")
    parent.mkdir(parents=True, exist_ok=True)
    return parent


def execute_capability(root, cap, versions, executables, cli, inputs, revision):
    parent = artifact_root(root)
    bundle = parent / (cap["id"] + "-" + uuid.uuid4().hex)
    bundle.mkdir(mode=0o700)
    stages = {key: "NOT_RUN" for key in ("typecheck", "scenarios", "simulation", "witnesses", "negativeControls", "freshness")}
    report = {"schemaVersion": 1, "capability": cap["id"], "lifecycle": cap["lifecycle"], "scope": "model-only",
              "startedAt": datetime.now(timezone.utc).isoformat(), "sourceRevision": revision,
              "inputs": inputs, "inputSnapshot": "inputs/", "tools": versions, "budget": cap["simulation"], "stages": stages,
              "implementationConformance": "NOT_RUN", "solverProof": "NOT_RUN",
              "selection": {key: cap[key] for key in ("properties", "runs", "witnesses")},
              "controlSelection": [{"id": row["id"], "property": row["property"]} for row in cap["controls"]],
              "commands": [], "traces": {}}
    stage = "typecheck"
    budget = cap["simulation"]
    base = [executables["node"], "--max-old-space-size=512", str(cli)]

    def invoke(label, args):
        output, log = bundle / (label + ".json"), bundle / (label + ".log")
        command = base + args + ["--out", str(output)]
        record = {"label": label, "argv": command, "cwd": str(root), "timeoutSeconds": budget["timeoutSeconds"]}
        report["commands"].append(record)
        write_json(bundle / "report.json", report)
        start = time.monotonic()
        try:
            result = process(command, root, budget["timeoutSeconds"], log)
            record.update({"exit": result.returncode, "durationSeconds": round(time.monotonic() - start, 3)})
            return result, read_json(output)
        except BehaviorError as error:
            record.update({"error": str(error), "durationSeconds": round(time.monotonic() - start, 3)})
            raise
        finally:
            if log.exists(): record["logSha256"] = digest(log)
            if output.exists(): record["outputSha256"] = digest(output)
            write_json(bundle / "report.json", report)

    def simulate(label, source, invariant, violation, samples):
        trace = bundle / (label + ".itf.json")
        result, data = invoke(label, ["run", str(source), "--main", cap["module"], "--backend=typescript",
                                      "--seed", budget["seed"], "--max-samples", str(samples),
                                      "--max-steps", str(budget["steps"]), "--invariant", invariant,
                                      "--out-itf", str(trace)])
        validate_simulation(data, result.returncode, violation, budget["steps"])
        report["traces"][label] = {"file": trace.name, "sha256": validate_itf(trace, source, data["status"], len(data["trace"])), "seed": data["seed"]}

    try:
        retain_inputs(root, bundle, inputs)
        receipt = local_path(root, INSTALLATION)
        require(digest(receipt) == versions["installationReceiptSha256"], "setup receipt changed before execution")
        shutil.copyfile(receipt, bundle / "installation.json")
        report["installationReceipt"] = {"file": "installation.json", "sha256": digest(bundle / "installation.json")}
        for label, source, module, defs in (
            ("model-types", cap["model"], cap["module"], [r["declaration"] for r in cap["properties"] + cap["witnesses"]]),
            ("scenario-types", cap["scenarios"], cap["scenarioModule"], [r["declaration"] for r in cap["runs"]])):
            result, data = invoke(label, ["typecheck", str(root / source)])
            require(result.returncode == 0, "type/effect process failed")
            validate_types(data, module, defs, "run" if label == "scenario-types" else "predicate")
        stages[stage] = "PASS"
        stage = "scenarios"
        names = [r["declaration"] for r in cap["runs"]]
        result, data = invoke("named-runs", ["test", str(root / cap["scenarios"]), "--main", cap["scenarioModule"],
                                           "--backend=typescript", "--seed", budget["seed"], "--max-samples", str(budget["scenarioSamples"]),
                                           "--match", "^(" + "|".join(names) + ")$", "--out-itf", str(bundle / "run_{test}_{seq}.itf.json")])
        require(result.returncode == 0, "named run process failed")
        validate_scenarios(data, names)
        for name in names:
            paths = sorted(bundle.glob("run_" + name + "_*.itf.json"))
            require(len(paths) == budget["scenarioSamples"], "missing or duplicate named-run traces")
            report["traces"][name] = [{"file": path.name, "sha256": validate_itf(path, root / cap["scenarios"], "passed")} for path in paths]
        stages[stage] = "PASS"
        stage = "simulation"
        invariant = " and ".join(p["declaration"] for p in cap["properties"])
        simulate("properties", root / cap["model"], invariant, False, budget["samples"])
        stages[stage] = "PASS"
        stage = "witnesses"
        for row in cap["witnesses"]:
            simulate("witness-" + row["declaration"], root / cap["model"], "not(" + row["declaration"] + ")", True, budget["witnessSamples"])
        stages[stage] = "PASS"
        stage = "negativeControls"
        model = regular(root / cap["model"]).decode("utf-8")
        props = {p["id"]: p["declaration"] for p in cap["properties"]}
        for index, row in enumerate(cap["controls"]):
            label = "control-" + str(index + 1)
            mutant = bundle / (label + ".qnt")
            mutant.write_text(model.replace(row["from"], row["to"], 1))
            result, data = invoke(label + "-types", ["typecheck", str(mutant)])
            require(result.returncode == 0, "negative control type/effect failure is not a killed mutation")
            validate_types(data, cap["module"], [props[row["property"]]])
            simulate(label, mutant, props[row["property"]], True, budget["witnessSamples"])
        stages[stage] = "PASS"
        stage = "freshness"
        require(snapshot(root, [cap]) == inputs, "behavior sources changed during execution")
        head = process(["git", "rev-parse", "HEAD"], root)
        require(head.returncode == 0 and head.stdout.strip() == revision, "source revision changed during execution")
        current, _, _ = installed_tool(root)
        require(current == versions, "installed tool changed during execution")
        stages[stage] = "PASS"
        report["status"] = "PASS"
    except (BehaviorError, OSError, KeyError, UnicodeError) as error:
        stages[stage] = "FAIL"
        report.update({"status": "FAIL", "error": str(error)})
        raise BehaviorError(f"{cap['id']}: {error}; evidence: {bundle.relative_to(root)}") from error
    finally:
        report["finishedAt"] = datetime.now(timezone.utc).isoformat()
        write_json(bundle / "report.json", report)
        print(f"behavior {cap['id']}: {report.get('status', 'FAIL')} (model-only); evidence: {bundle.relative_to(root)}")
        print("implementation-conformance: NOT_RUN; DB/API/storage/client/device acceptance: NOT_RUN; exhaustive proof: NOT_RUN")
    return bundle


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capability")
    parser.add_argument("--setup", action="store_true")
    parser.add_argument("--probe", action="store_true")
    parser.add_argument("--runtime-only", action="store_true")
    args = parser.parse_args()
    try:
        if args.probe:
            require(not args.setup and args.capability is None, "probe must be read-only")
            versions = runtime(ROOT)[1] if args.runtime_only else installed_tool(ROOT)[0]
            print("; ".join(f"{k} {versions[k]}" for k in ("node", "npm", "quint") if k in versions))
            return 0
        require(not args.runtime_only, "runtime-only belongs to probe mode")
        capabilities = load_catalog(ROOT, args.capability)
        if args.setup:
            manifest, _, executables = runtime(ROOT)
            validate_lock(manifest, read_json(local_path(ROOT, TOOL_DIR + "/package-lock.json")))
            initial_inputs = setup_inputs(ROOT)
            output = artifact_root(ROOT)
            log = output / ("setup-" + uuid.uuid4().hex + ".log")
            command = [executables["npm"], "ci", "--ignore-scripts", "--no-audit", "--no-fund",
                       "--registry=https://registry.npmjs.org/", "--cache", str(output / "npm-cache")]
            result = process(command, ROOT / TOOL_DIR, 120, log)
            require(result.returncode == 0, f"locked npm setup failed; log: {log.relative_to(ROOT)}")
            require(setup_inputs(ROOT) == initial_inputs, "toolchain inputs changed during explicit setup")
            installed, _, _ = installed_tool(ROOT, require_receipt=False)
            receipt = output / ("installation-" + uuid.uuid4().hex + ".json")
            write_json(receipt, {"schemaVersion": 1, "inputs": initial_inputs, "tools": installed,
                                 "installedAt": datetime.now(timezone.utc).isoformat(), "argv": command,
                                 "log": log.name, "logSha256": digest(log)})
            receipt.chmod(0o600)
            receipt.replace(ROOT / INSTALLATION)
        versions, executables, cli = installed_tool(ROOT)
        revision_result = process(["git", "rev-parse", "HEAD"], ROOT)
        require(revision_result.returncode == 0, "source revision unavailable")
        revision = revision_result.stdout.strip()
        for cap in capabilities:
            execute_capability(ROOT, cap, versions, executables, cli, snapshot(ROOT, [cap]), revision)
        return 0
    except (BehaviorError, OSError, KeyError, UnicodeError) as error:
        print(f"behavior: FAIL: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
