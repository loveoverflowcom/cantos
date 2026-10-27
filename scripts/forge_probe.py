"""Implementation primitive for cargo xtask forge-probe. Never invokes cloud tools."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shlex
import shutil
import stat
import struct
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path("services/cantos-forge-functions/firebase")
POLICY = Path("infra/firebase/functions/probe-policy.json")
TARGET = "x86_64-unknown-linux-musl"
RUNTIME_FILES = ("package.json", "package-lock.json", "functions.mjs", "index.mjs",
                 "probe.mjs", "http.mjs", "worker.mjs", "protocol.mjs")
SCHEMA = "cantos-forge.probe-bundle/v1"


class Refused(ValueError):
    """A closed local validation failure; never contains worker output or credentials."""


def require(condition, code):
    if not condition:
        raise Refused(code)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def read_json(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path, value):
    path.write_text(json.dumps(value, sort_keys=True, indent=2) + "\n", encoding="utf-8")


def policy(root=ROOT):
    value = read_json(root / POLICY)
    require(value["project"] == "cantos-a348b" and value["target"] == TARGET
            and value["source"] == "functions" and value["runtime"] == "nodejs22",
            "invalid_probe_policy")
    return value


def destination(project, region, output, root=ROOT):
    value = policy(root)
    require(project == value["project"], "wrong_project")
    require(re.fullmatch(r"[a-z]+-[a-z]+[0-9]+", region) is not None, "invalid_region")
    path = Path(output)
    path = path if path.is_absolute() else root / path
    resolved = path.resolve()
    require((root / "artifacts/forge-probe").resolve().is_relative_to(root.resolve()), "unsafe_output")
    require(resolved.is_relative_to((root / "artifacts/forge-probe").resolve())
            and resolved != (root / "artifacts/forge-probe").resolve(), "unsafe_output")
    require(not path.exists() and not path.is_symlink(), "output_exists")
    return value, resolved


def firebase_config(value):
    return {"functions": [{"source": value["source"], "codebase": value["codebase"],
                           "runtime": value["runtime"], "ignore": value["ignore"]}]}


def static_linux_elf(data, mode):
    """Inspect ELF independently of host tools. Static musl needs no cloud glibc/loader."""
    require(mode & stat.S_IXUSR and not mode & 0o022, "worker_permissions")
    require(64 <= len(data) <= 32 * 1024 * 1024, "worker_size")
    require(data[:7] == b"\x7fELF\x02\x01\x01", "worker_not_linux_elf64")
    kind, machine, version = struct.unpack_from("<HHI", data, 16)
    require(kind in (2, 3) and machine == 62 and version == 1, "worker_wrong_architecture")
    offset = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    require(size == 56 and 1 <= count <= 128 and offset >= 64
            and offset + size * count <= len(data), "worker_invalid_headers")
    load = False
    for index in range(count):
        segment, _, start, _, _, length, _, _ = struct.unpack_from("<IIQQQQQQ", data, offset + index * size)
        require(start + length <= len(data), "worker_invalid_segment")
        require(segment != 3, "worker_dynamic_loader")  # PT_INTERP
        load |= segment == 1 and length > 0
        if segment == 2:  # PT_DYNAMIC: reject DT_NEEDED, even without PT_INTERP.
            require(length % 16 == 0, "worker_invalid_dynamic")
            require(all(struct.unpack_from("<qQ", data, pos)[0] != 1
                        for pos in range(start, start + length, 16)), "worker_dynamic_dependency")
    require(load, "worker_missing_load_segment")
    return {"format": "ELF64-LE", "machine": "x86_64", "linkage": "static", "target": TARGET}


def snapshot(root=ROOT):
    paths = [root / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml")]
    for parent in (root / "services/cantos-forge-functions/src", root / SOURCE, root / POLICY.parent):
        for path in parent.rglob("*"):
            if "node_modules" in path.parts or "bin" in path.parts:
                continue
            require(not path.is_symlink(), "source_symlink")
            if path.is_file():
                paths.append(path)
    paths.extend(root / name for name in ("services/cantos-forge-functions/Cargo.toml", "scripts/forge_probe.py"))
    values = {}
    for path in sorted(set(paths)):
        require(not path.is_symlink(), "source_symlink")
        values[str(path.relative_to(root))] = digest(path.read_bytes())
    return digest(json.dumps(values, sort_keys=True).encode())


def stage(root, output, value, region, worker, revision, inputs, compiler):
    data = worker.read_bytes()
    abi = static_linux_elf(data, worker.stat().st_mode)
    require(re.fullmatch(r"[0-9a-f]{40}", revision) is not None, "invalid_source_revision")
    require(re.fullmatch(r"[0-9a-f]{64}", inputs) is not None, "invalid_input_digest")
    output.mkdir(parents=True, exist_ok=False)
    functions = output / value["source"]
    functions.mkdir()
    for name in RUNTIME_FILES:
        source = root / SOURCE / name
        require(not source.is_symlink(), "source_symlink")
        shutil.copyfile(source, functions / name)
    (functions / "bin").mkdir()
    (functions / "bin/cantos-forge-worker").write_bytes(data)
    (functions / "bin/cantos-forge-worker").chmod(0o755)
    write_json(functions / "probe-config.json", {"region": region})
    write_json(output / "firebase.json", firebase_config(value))
    shutil.copyfile(root / SOURCE / "test/packaged.test.mjs", output / "packaged.test.mjs")
    files = {str(path.relative_to(output)): digest(path.read_bytes())
             for path in sorted(output.rglob("*")) if path.is_file()}
    write_json(output / "receipt.json", {"schema": SCHEMA, "project": value["project"],
               "codebase": value["codebase"], "function": value["function"], "region": region,
               "source_revision": revision, "source_inputs_sha256": inputs, "compiler": compiler,
               "worker": {**abi, "sha256": digest(data), "mode": "0755"}, "files": files,
               "cloud": "NOT_RUN"})
    inspect(output, root)


def inspect(bundle, root=ROOT):
    bundle = Path(bundle).absolute()
    require(not bundle.is_symlink(), "bundle_symlink")
    receipt = read_json(bundle / "receipt.json")
    value = policy(root)
    require(receipt["schema"] == SCHEMA and receipt["project"] == value["project"]
            and receipt["codebase"] == value["codebase"] and receipt["function"] == value["function"]
            and receipt["cloud"] == "NOT_RUN", "invalid_bundle_receipt")
    require(re.fullmatch(r"[0-9a-f]{40}", receipt["source_revision"]) is not None
            and re.fullmatch(r"[0-9a-f]{64}", receipt["source_inputs_sha256"]) is not None,
            "invalid_source_binding")
    require(re.fullmatch(r"[a-z]+-[a-z]+[0-9]+", receipt["region"]) is not None, "invalid_region")
    functions = bundle / value["source"]
    expected = {f"functions/{name}" for name in RUNTIME_FILES}
    expected |= {"functions/bin/cantos-forge-worker", "functions/probe-config.json", "firebase.json", "packaged.test.mjs"}
    require(set(receipt["files"]) == expected, "bundle_file_inventory")
    actual = set()
    for path in bundle.rglob("*"):
        relative = path.relative_to(bundle)
        if relative.parts[:2] == ("functions", "node_modules"):
            continue
        require(not path.is_symlink(), "bundle_symlink")
        if path.is_file() and relative != Path("receipt.json"):
            actual.add(str(relative))
    require(actual == expected, "unexpected_bundle_file")
    for name, sha in receipt["files"].items():
        require(digest((bundle / name).read_bytes()) == sha, "bundle_hash_mismatch")
    require(read_json(bundle / "firebase.json") == firebase_config(value), "invalid_effective_config")
    require(read_json(functions / "probe-config.json") == {"region": receipt["region"]}, "invalid_runtime_config")
    package = read_json(functions / "package.json")
    require(package["engines"] == {"node": "22"} and package["main"] == "functions.mjs"
            and set(package["dependencies"]) == {"firebase-functions"}
            and "scripts" not in package, "invalid_runtime_package")
    worker = functions / "bin/cantos-forge-worker"
    abi = static_linux_elf(worker.read_bytes(), worker.stat().st_mode)
    require(receipt["worker"] == {**abi, "sha256": digest(worker.read_bytes()), "mode": "0755"}
            and stat.S_IMODE(worker.stat().st_mode) == 0o755, "worker_receipt_mismatch")
    return receipt


def invoke(args, cwd=ROOT, capture=False, env=None):
    return subprocess.run(args, cwd=cwd, check=True, text=True, capture_output=capture, env=env)


def package(args):
    value, output = destination(args.project, args.region, args.output)
    plan = {"project": value["project"], "region": args.region, "codebase": value["codebase"],
            "function": value["function"], "target": TARGET, "output": str(output),
            "cloud": "NOT_RUN", "deploy_selector": f"functions:{value['codebase']}:{value['function']}"}
    print(json.dumps(plan, sort_keys=True), flush=True)
    if args.dry_run:
        return 0
    targets = invoke(["rustup", "target", "list", "--installed"], capture=True).stdout.splitlines()
    if TARGET not in targets:
        print("UNSUPPORTED: installed Rust Linux musl target/linker required; nothing installed", file=sys.stderr)
        return 3
    before = snapshot()
    revision = invoke(["git", "rev-parse", "HEAD"], capture=True).stdout.strip()
    compiler = invoke(["rustc", "--version"], capture=True).stdout.strip()
    require(compiler.startswith("rustc 1.98.1 "), "rust_1_98_1_required")
    target_dir = ROOT / "target/forge-probe-build"
    env = {key: val for key, val in os.environ.items() if key not in
           ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR")}
    invoke(["cargo", "build", "--locked", "--release", "-p", "cantos-forge", "--bin", "cantos-forge-worker",
            "--target", TARGET, "--target-dir", str(target_dir)], env=env)
    require(snapshot() == before, "source_changed_during_build")
    stage(ROOT, output, value, args.region, target_dir / TARGET / "release/cantos-forge-worker",
          revision, before, compiler)
    print("PASS: static Linux package inspected; runtime qualification and cloud are NOT_RUN")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    build = commands.add_parser("package")
    for name in ("project", "region", "output"):
        build.add_argument("--" + name, required=True)
    build.add_argument("--dry-run", action="store_true")
    for name in ("inspect", "qualify", "deploy-plan"):
        commands.add_parser(name).add_argument("--bundle", required=True, type=Path)
    args = parser.parse_args()
    try:
        if args.action == "package":
            return package(args)
        receipt = inspect(args.bundle)
        if args.action == "deploy-plan":
            print("PLAN ONLY: operator authorization, region, APIs, IAM, billing/budget and Linux qualification required")
            print("cwd: " + str(args.bundle.resolve()))
            print(shlex.join(["firebase", "deploy", "--project", receipt["project"],
                              "--config", str((args.bundle / "firebase.json").resolve()),
                              "--only", f"functions:{receipt['codebase']}:{receipt['function']}",
                              "--non-interactive"]))
            print("Cloud NOT_RUN. Never add --force; reconcile any uncertain remote outcome before retry.")
            return 0
        if args.action == "inspect":
            print(json.dumps(receipt, sort_keys=True))
            return 0
        if platform.system() != "Linux" or platform.machine() not in ("x86_64", "amd64"):
            print("UNSUPPORTED: real Linux x86_64 required; no install attempted", file=sys.stderr)
            return 3
        version = invoke(["node", "--version"], capture=True).stdout.strip()
        require(version.startswith("v22."), "node22_required")
        invoke(["npm", "ci", "--ignore-scripts", "--omit=dev", "--no-audit", "--no-fund"],
               cwd=args.bundle / "functions")
        invoke(["node", "--test", "packaged.test.mjs"], cwd=args.bundle)
        inspect(args.bundle)
        print("PASS: local Linux Node22/SDK/packaged Rust HTTP; cloud NOT_RUN")
        return 0
    except (Refused, OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        # Avoid reflecting subprocess stdout, paths or credential-bearing error messages.
        print("FAIL: " + (str(error) if isinstance(error, Refused) else type(error).__name__), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
