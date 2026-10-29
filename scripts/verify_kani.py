#!/usr/bin/env python3
"""Single registered Kani invocation primitive, owned by cargo xtask.

Rust supplies the fixed scope/model. Probe mode is read-only and never calls the
installer proxy. No arbitrary verifier options or model-changing environment is accepted.
Receipt mode (`--check-receipt`) is read-only too: it re-validates one retained run against the
current registration and inputs and never executes a verifier.
"""

import argparse
import contextlib
import hashlib
import io
import json
import os
import re
import selectors
import signal
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TIMEOUT_SECONDS = 900
MAX_LOG_BYTES = 4 * 1024 * 1024
MAX_CONFIGURATION_BYTES = 1024 * 1024
MAX_GROUP_STATUS_BYTES = 64 * 1024
DEFAULT_PACKAGE = "cantos-server"
# Closed allowlist of workspace packages that may own a proof kernel (mirrors xtask's
# `Package`): manifest, hashed source directories and hashed files, all workspace-relative.
# The API row is the long-standing model and must not change; a new package is a new row.
PACKAGES = {
    "cantos-server": (
        "services/cantos-server/Cargo.toml",
        # The Server's own contract representation is under its src; its canonical schemas and
        # fixtures (formerly the separate packages/contracts crate) are a hashed directory.
        ("services/cantos-server/src", "services/cantos-server/contracts", ".cargo"),
        ("Cargo.toml", "services/cantos-server/Cargo.toml", "services/cantos-server/build.rs",
         "scripts/verify_kani.py", "xtask/src/kani.rs"),
    ),
    "cantos-forge": (
        "services/cantos-forge-functions/Cargo.toml",
        ("services/cantos-forge-functions/src", ".cargo"),
        ("Cargo.toml", "services/cantos-forge-functions/Cargo.toml", "scripts/verify_kani.py", "xtask/src/kani.rs"),
    ),
    "cantos-identity": (
        "services/cantos-identity/Cargo.toml",
        ("services/cantos-identity/src", ".cargo"),
        ("Cargo.toml", "services/cantos-identity/Cargo.toml", "scripts/verify_kani.py", "xtask/src/kani.rs"),
    ),
}
FORBIDDEN_ENVIRONMENT = frozenset((
    "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTFLAGS",
    "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
))


class KaniError(Exception):
    pass


def package_registration(package):
    """Fail closed on any name outside the allowlist; never echo the rejected value."""
    if not isinstance(package, str) or package not in PACKAGES:
        raise KaniError("unregistered Kani package")
    return PACKAGES[package]


def package_of(args):
    """A missing package is the API, so every earlier invocation keeps its exact model."""
    package = getattr(args, "package", DEFAULT_PACKAGE)
    package_registration(package)
    return package


def child_environment(bundle, env):
    forbidden = sorted(key for key, value in env.items() if value and (
        key in FORBIDDEN_ENVIRONMENT or (key.startswith("CARGO_TARGET_") and key.endswith("_RUSTFLAGS"))))
    if forbidden:
        raise KaniError("unreviewed proof environment: " + ", ".join(forbidden))
    result = dict(env)
    result["PATH"] = os.pathsep.join((str(bundle / "bin"), str(bundle / "toolchain/bin"), env.get("PATH", "")))
    result["RUSTUP_TOOLCHAIN"] = (bundle / "rust-toolchain-version").read_text().strip()
    # Match the official proxy's child-only rustup dynamic-linker cleanup.
    for key in ("LD_LIBRARY_PATH", "DYLD_FALLBACK_LIBRARY_PATH"):
        if key in result:
            result[key] = os.pathsep.join(path for path in result[key].split(os.pathsep)
                                          if not (len(Path(path).parts) >= 3 and Path(path).parts[-3] == "toolchains" and Path(path).parts[-1] == "lib"))
    return result


@contextlib.contextmanager
def interrupt_signals():
    previous = {}
    def interrupted(signum, _frame):
        raise KaniError("interrupted by " + signal.Signals(signum).name)
    try:
        for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            previous[signum] = signal.signal(signum, interrupted)
        yield
    finally:
        for signum, handler in previous.items():
            signal.signal(signum, handler)


def darwin_group_is_only_zombies(group):
    """Inspect only the pinned group; unavailable or ambiguous evidence fails closed."""
    try:
        # This fixed OS command cannot execute verifier code or create descendants.
        # Bound its duration and the bytes admitted to memory; never log process details.
        with tempfile.TemporaryFile() as output:
            result = subprocess.run(
                ["/bin/ps", "-g", str(group), "-o", "pid=,pgid=,stat="],
                stdin=subprocess.DEVNULL, stdout=output, stderr=subprocess.DEVNULL,
                env={"LC_ALL": "C"}, timeout=0.5, check=False,
            )
            output.seek(0)
            snapshot = output.read(MAX_GROUP_STATUS_BYTES + 1)
        if result.returncode != 0 or len(snapshot) > MAX_GROUP_STATUS_BYTES:
            return False
        members = set()
        for row in snapshot.decode("ascii").splitlines():
            fields = row.split()
            if (len(fields) != 3 or not fields[0].isdigit() or not fields[1].isdigit()
                    or len(fields[0]) > 10 or len(fields[1]) > 10):
                return False
            pid, observed_group, state = int(fields[0]), int(fields[1]), fields[2]
            if (not 1 <= pid <= 2**31 - 1 or pid in members or observed_group != group
                    or re.fullmatch(r"Z[+<>AELNSsVWX]*", state) is None):
                return False
            members.add(pid)
        # The unreaped leader must appear; an empty/truncated group is not cleanup proof.
        return group in members
    except (OSError, UnicodeError, subprocess.TimeoutExpired):
        return False


def stop_owned_group(process):
    """Keep the leader pinned until signalling or proof that no member can execute."""
    try:
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except PermissionError:
        # Darwin denies signals to a zombie-only group. Do not reap to work around
        # this: reaping would permit PGID reuse before a later group signal.
        # uname observes the actual host even when a fixture models Linux Kani.
        if os.uname().sysname != "Darwin":
            raise
        exited = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
        if exited is None or exited.si_pid != process.pid or not darwin_group_is_only_zombies(process.pid):
            raise


def bounded_run(command, cwd, env, timeout, output_path, max_bytes=MAX_LOG_BYTES):
    """Keep the owned leader unreaped until its process group has been stopped."""
    if os.name != "posix" or not hasattr(os, "WNOWAIT"):
        raise KaniError("the registered verifier requires a POSIX waitid process boundary")
    sink = io.BytesIO()
    log = None
    process = None
    selector = selectors.DefaultSelector()
    try:
        if output_path is not None:
            fd = os.open(output_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            log = os.fdopen(fd, "wb")
        with interrupt_signals():
            process = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                       stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
            selector.register(process.stdout, selectors.EVENT_READ)
            deadline = time.monotonic() + timeout
            eof = False
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise KaniError("verifier command timed out")
                for key, _ in selector.select(min(remaining, 0.05)):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        eof = True
                    else:
                        if sink.tell() + len(chunk) > max_bytes:
                            retained = chunk[:max(0, max_bytes - sink.tell())]
                            sink.write(retained)
                            if log: log.write(retained); log.flush()
                            raise KaniError("verifier output exceeded its byte limit")
                        sink.write(chunk)
                        if log: log.write(chunk); log.flush()
                done = os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                if done is not None:
                    # An exited leader may leave descendants holding its output pipe.
                    if not eof:
                        stop_owned_group(process)
                    else:
                        break
    except OSError as error:
        raise KaniError("could not execute verifier command") from error
    finally:
        if process is not None:
            # Never reap before signalling the exact group this invocation created.
            stop_owned_group(process)
            process.wait(timeout=5)
            if process.stdout: process.stdout.close()
        selector.close()
        if log: log.close()
    return subprocess.CompletedProcess(command, process.returncode, sink.getvalue().decode("utf-8", errors="replace"))


def probe(bundle, args):
    # Resolve caller-relative KANI_HOME once before subprocess cwd switches to bundle.
    bundle = bundle.resolve()
    if args.target != "x86_64-unknown-linux-gnu" or sys.platform != "linux":
        raise KaniError("registered model requires x86_64-unknown-linux-gnu")
    for relative in ("bin/kani-driver", "bin/kani-compiler", "bin/cbmc", "toolchain/bin/cargo", "toolchain/bin/rustc"):
        path = bundle / relative
        if not path.is_file() or not os.access(path, os.X_OK):
            raise KaniError("missing existing Kani bundle executable: " + relative)
    try:
        if (bundle / "rustc-version").read_text().strip() != args.rustc_version:
            raise KaniError("bundled compiler sentinel does not match the fixed model")
        if (bundle / "rust-toolchain-version").read_text().strip() != args.toolchain:
            raise KaniError("bundled toolchain sentinel does not match the fixed model")
        env = child_environment(bundle, os.environ)
    except OSError as error:
        raise KaniError("missing compiler/toolchain sentinel") from error
    def run(command):
        output = bounded_run(command, bundle, env, 10, None)
        if output.returncode != 0: raise KaniError("installed verifier version probe failed")
        return output.stdout.strip()
    compiler = run([str(bundle / "toolchain/bin/rustc"), "--version"])
    host = run([str(bundle / "toolchain/bin/rustc"), "-vV"])
    expected = "Kani Rust Verifier " + args.kani_version + " (cargo plugin)\nCBMC " + args.cbmc_version
    version = run([str(bundle / "bin/kani-driver"), "kani", "--version"])
    if compiler != args.rustc_version or ("host: " + args.target) not in host.splitlines():
        raise KaniError("actual bundled compiler/host does not match the fixed model")
    if version != expected:
        raise KaniError("actual Kani/CBMC versions do not match the fixed model")
    return {"version": version, "compiler": compiler, "host": args.target, "bundle": str(bundle)}


def validate_output(text, harness, cover_count, allowed_unreachable):
    if cover_count < 1: raise KaniError("a positive registered cover count is required")
    selected = re.findall(r"^Checking harness (.+)\.\.\.$", text, re.MULTILINE)
    if selected != [harness]: raise KaniError("expected exactly one matching executed harness")
    headers = re.findall(r"^Check [^\n]*$", text, re.MULTILINE)
    # Kani quotes each description; a quoted one may wrap where rustfmt wrapped a long asserted
    # expression. A continuation line can never start another field or check header, so a
    # wrapped description cannot absorb a missing Location or a following block.
    blocks = list(re.finditer(
        r"^Check ([1-9][0-9]*): ([^\n]+)\n[ \t]+- Status: ([A-Z]+)\n"
        r"[ \t]+- Description: (\"[^\n]*(?:\n(?![ \t]+- |Check [0-9]+: )[^\n]*)*\"|[^\n\"][^\n]*)\n"
        r"[ \t]+- Location: ([^\n]+)(?=\n|$)", text, re.MULTILINE))
    if not blocks or len(blocks) != len(headers): raise KaniError("missing or malformed property blocks")
    for field in ("Status", "Description", "Location"):
        if len(re.findall(r"^[ \t]+- " + field + r":", text, re.MULTILINE)) != len(blocks):
            raise KaniError("missing or duplicated property fields")
    ids, numbers, covers, ordinary, unreachable = set(), [], [], [], []
    for block in blocks:
        number, identifier, status, description, location = block.groups()
        if identifier in ids: raise KaniError("duplicate property identity")
        ids.add(identifier); numbers.append(int(number))
        if not description.strip() or not location.strip(): raise KaniError("incomplete property evidence")
        if identifier.startswith(harness + ".cover."):
            if status != "SATISFIED": raise KaniError("intended cover was not satisfied")
            covers.append(identifier)
        else:
            if status not in ("SUCCESS", "UNREACHABLE"): raise KaniError("property failed or was undetermined")
            ordinary.append(identifier)
            if status == "UNREACHABLE": unreachable.append(identifier)
    if numbers != list(range(1, len(numbers) + 1)): raise KaniError("missing or duplicate property number")
    if not ordinary: raise KaniError("zero ordinary verification checks")
    if set(covers) != {harness + ".cover." + str(number) for number in range(1, cover_count + 1)}:
        raise KaniError("cover identities/count do not match registration")
    if set(unreachable) != set(allowed_unreachable): raise KaniError("unreviewed or changed unreachable assertion evidence")
    if len(re.findall(r"^SUMMARY:$", text, re.MULTILINE)) != 1: raise KaniError("missing or ambiguous check summary")
    checks = re.findall(r"^ \*\* ([0-9]+) of ([0-9]+) failed(?: \(([0-9]+) unreachable\))?$", text, re.MULTILINE)
    cover_summary = re.findall(r"^ \*\* ([0-9]+) of ([0-9]+) cover properties satisfied$", text, re.MULTILINE)
    if checks != [("0", str(len(ordinary)), str(len(unreachable)) if unreachable else "")]:
        # Some versions explicitly print (0 unreachable); it carries the same information.
        if checks != [("0", str(len(ordinary)), "0")] or unreachable:
            raise KaniError("ordinary check summary does not match complete property evidence")
    if cover_summary != [(str(cover_count), str(cover_count))]: raise KaniError("cover summary does not match registration")
    if re.findall(r"^VERIFICATION:- .*?$", text, re.MULTILINE) != ["VERIFICATION:- SUCCESSFUL"]:
        raise KaniError("verification did not conclusively succeed")
    if re.findall(r"^Complete - .*?$", text, re.MULTILINE) != ["Complete - 1 successfully verified harnesses, 0 failures, 1 total."]:
        raise KaniError("completion did not establish exactly one successful proof")
    return {"checks": len(ordinary), "covers": len(covers), "unreachable": unreachable}


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_identity(root):
    identity = {}
    for label, arguments in (("revision", ["rev-parse", "HEAD"]),
                             ("status", ["status", "--porcelain=v1", "--untracked-files=normal"]),
                             ("diff", ["diff", "--no-ext-diff", "--no-textconv", "--binary", "HEAD", "--"])):
        result = bounded_run(["git", "--no-optional-locks", *arguments], root, os.environ.copy(), 10, None)
        if result.returncode != 0: raise KaniError("could not bind Git source identity")
        if label == "revision":
            identity[label] = result.stdout.strip()
            if not re.fullmatch(r"[0-9a-f]{40,64}", identity[label]):
                raise KaniError("invalid Git source revision identity")
        else:
            identity[label + "_sha256"] = hashlib.sha256(result.stdout.encode()).hexdigest()
            if label == "status": identity["dirty"] = bool(result.stdout.strip())
    return identity


def source_tree_identity(root, package=DEFAULT_PACKAGE):
    """Fixed per-package build inputs, including newly added and removed files."""
    _, directories, names = package_registration(package)
    paths = set()
    for directory in directories:
        parent = root / directory
        if parent.exists(): paths.update(path for path in parent.rglob("*") if path.is_file())
    for name in names:
        path = root / name
        if path.is_file(): paths.add(path)
    return {str(path.relative_to(root)): sha256(path) for path in sorted(paths)}


def identity_digest(identity):
    return hashlib.sha256(json.dumps(identity, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def configuration_sha256(path):
    with path.open("rb") as source:
        approved = source.read(MAX_CONFIGURATION_BYTES + 1)
    if len(approved) > MAX_CONFIGURATION_BYTES:
        raise KaniError("proof configuration is too large")
    return hashlib.sha256(approved).hexdigest()


def configuration_identity(root, env, package=DEFAULT_PACKAGE):
    """Same fixed candidates as Rust, including absence at every Cargo config path."""
    manifest = package_registration(package)[0]
    root = root.resolve()
    candidates = {root / "Cargo.toml", root / manifest}
    for parent in (root, *root.parents):
        candidates.update(parent / ".cargo" / name for name in ("config", "config.toml"))
    # Cargo ignores an empty CARGO_HOME; both approval layers must bind its fallback.
    home = env.get("CARGO_HOME")
    if not home:
        user_home = env.get("HOME")
        if not user_home:
            raise KaniError("fixed proof configuration requires nonempty CARGO_HOME or HOME")
        home = str(Path(user_home) / ".cargo")
    cargo_home = (root / home).absolute()
    candidates.update(cargo_home / name for name in ("config", "config.toml"))
    identity = {}
    for path in sorted(candidates):
        try:
            identity[str(path)] = configuration_sha256(path)
        except FileNotFoundError:
            identity[str(path)] = None
    return identity


def approved_configuration(args, root):
    manifest = package_registration(package_of(args))[0]
    try:
        approved = json.loads(args.approved_config)
        if (not isinstance(approved, dict) or not approved
                or any(not isinstance(path, str) or not Path(path).is_absolute()
                       or (digest is not None and (not isinstance(digest, str)
                           or not re.fullmatch(r"[0-9a-f]{64}", digest)))
                       for path, digest in approved.items())
                or any(approved.get(str(root / name)) is None
                       for name in ("Cargo.toml", manifest))):
            raise ValueError()
    except (AttributeError, TypeError, ValueError):
        raise KaniError("Rust-approved configuration identity is required") from None
    return approved


def check_configuration(root, approved, package=DEFAULT_PACKAGE):
    if configuration_identity(root, os.environ, package) != approved:
        raise KaniError("Rust-approved proof configuration changed before or during verification")


def verify(args, root=ROOT):
    root = root.resolve()
    package = package_of(args)
    approved = approved_configuration(args, root)
    check_configuration(root, approved, package)
    home = Path(os.environ.get("KANI_HOME", str(Path.home() / ".kani")))
    bundle = (home / ("kani-" + args.kani_version)).resolve()
    tools = probe(bundle, args)
    check_configuration(root, approved, package)
    source = (root / args.source).resolve()
    if not source.is_file() or not source.is_relative_to(root.resolve()):
        raise KaniError("registered source must be inside the repository")
    lock = root / "Cargo.lock"
    lock_before = sha256(lock)
    inputs = {str(source): sha256(source)}
    model_inputs = source_tree_identity(root, package)
    git_before = git_identity(root)
    artifact = root / "artifacts/agent-runs" / ("kani-" + uuid.uuid4().hex)
    artifact.mkdir(parents=True, mode=0o700)
    command = [str(bundle / "bin/kani-driver"), "kani", "--manifest-path", str(root / "Cargo.toml"),
               "-p", package, "--lib", "--no-default-features", "--exact", "--harness", args.harness,
               "--unwind", str(args.unwind), "--solver", "cadical", "--output-format", "regular",
               "--target-dir", str(root / "target/assurance-kani")]
    metadata = {"tools": tools, "command": command, "package": package, "source_inputs": inputs,
                "lock_before": lock_before, "harness": args.harness, "unwind": args.unwind,
                "expected_covers": args.cover_count, "status": "FAIL", "raw_log": "raw.log"}
    metadata["git"] = git_before
    metadata["approved_configuration"] = approved
    metadata["approved_configuration_sha256"] = identity_digest(approved)
    metadata["model_inputs_sha256"] = identity_digest(model_inputs)
    metadata["model_inputs"] = model_inputs
    started = time.monotonic()
    error = None
    try:
        check_configuration(root, approved, package)
        output = bounded_run(command, root, child_environment(bundle, os.environ), TIMEOUT_SECONDS, artifact / "raw.log", MAX_LOG_BYTES)
        if output.returncode != 0: raise KaniError("verifier process failed with exit " + str(output.returncode))
        metadata.update(validate_output(output.stdout, args.harness, args.cover_count, args.allow_unreachable))
        metadata["status"] = "PASS"
    except (KaniError, OSError, KeyboardInterrupt) as failure:
        error = failure
        metadata["error"] = str(failure)
    finally:
        metadata["elapsed_seconds"] = time.monotonic() - started
        try:
            metadata["lock_after"] = sha256(lock)
            metadata["configuration_after_sha256"] = identity_digest(configuration_identity(root, os.environ, package))
            metadata["model_inputs_after_sha256"] = identity_digest(source_tree_identity(root, package))
            if (metadata["lock_after"] != lock_before
                    or metadata["model_inputs_after_sha256"] != metadata["model_inputs_sha256"]
                    or metadata["configuration_after_sha256"] != metadata["approved_configuration_sha256"]
                    or any(sha256(Path(path)) != digest for path, digest in inputs.items())
                    or git_identity(root) != git_before):
                error = KaniError("Cargo.lock or registered source/configuration changed during verification")
                metadata["error"] = str(error)
        except (OSError, KaniError):
            error = KaniError("could not establish unchanged lock/source/configuration after verification")
            metadata["error"] = str(error)
        if error is not None: metadata["status"] = "FAIL"
        (artifact / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print("Kani evidence: " + str(artifact.relative_to(root)))
    if error is not None: raise KaniError(str(error))
    print("Executed 1 exact harness; " + str(metadata["checks"]) + " checks, " + str(metadata["covers"]) + " satisfied covers; lock unchanged")
    return metadata


MAX_RECEIPT_DIFFERENCES = 8


def receipt_command(root, package, harness, unwind):
    """The exact verifier argv after the bundle executable, for a checkout at `root`."""
    return ["kani", "--manifest-path", str(root / "Cargo.toml"), "-p", package, "--lib",
            "--no-default-features", "--exact", "--harness", harness, "--unwind", str(unwind),
            "--solver", "cadical", "--output-format", "regular",
            "--target-dir", str(root / "target/assurance-kani")]


def portable_configuration(identity, root):
    """Workspace candidates by relative path. Outside the checkout only present files count: an
    absent ancestor/CARGO_HOME config is the same on every machine, a present one stays absolute
    and is therefore comparable only on the machine (and path) that recorded it."""
    result = {}
    for path, digest in identity.items():
        candidate = Path(path)
        if candidate.is_relative_to(root):
            result[str(candidate.relative_to(root))] = digest
        elif digest is not None:
            result[path] = digest
    return result


def differences(before, after):
    changed = sorted(path for path in set(before) | set(after) if before.get(path) != after.get(path))
    shown = ", ".join(changed[:MAX_RECEIPT_DIFFERENCES])
    return shown + (f" and {len(changed) - MAX_RECEIPT_DIFFERENCES} more" if len(changed) > MAX_RECEIPT_DIFFERENCES else "")


def check_receipt(args, root=ROOT):
    """Classify one retained `metadata.json` as CURRENT, or raise with every stale/invalid reason.

    Checks the actual runner output shape (raw log re-parsed with the current registration), the
    recorded outcome, tool/solver/bound/feature model, Cargo.lock, configuration and the whole
    per-package input tree the run hashed. Consistency is not authenticity: a checksum is no
    tamper evidence, and a CURRENT receipt is reused evidence of one past run, not a new proof.
    """
    root = root.resolve()
    package = package_of(args)
    relative = Path(args.check_receipt)
    if relative.is_absolute() or ".." in relative.parts or relative.name != "metadata.json":
        raise KaniError("receipt must be a repository-relative metadata.json")
    receipt = root / relative
    log = receipt.parent / "raw.log"
    for path in (receipt, log):
        if path.is_symlink() or not path.is_file() or not path.resolve().is_relative_to(root):
            raise KaniError("receipt or its raw log is missing, linked or outside the checkout")
        if path.stat().st_size > MAX_LOG_BYTES:
            raise KaniError("receipt or its raw log is oversized")
    try:
        metadata = json.loads(receipt.read_text(encoding="utf-8"))
        text = log.read_text(encoding="utf-8")
    except (OSError, UnicodeError, ValueError):
        raise KaniError("receipt is not readable JSON with a UTF-8 raw log") from None
    if not isinstance(metadata, dict):
        raise KaniError("receipt is not an object")
    problems = []
    if metadata.get("status") != "PASS" or "error" in metadata:
        problems.append("recorded outcome is " + str(metadata.get("status")) + ", not PASS")
    try:
        parsed = validate_output(text, args.harness, args.cover_count, args.allow_unreachable)
    except KaniError as error:
        raise KaniError("raw log is not complete runner evidence for the registration: " + str(error)) from None
    if any(metadata.get(key) != value for key, value in parsed.items()):
        problems.append("recorded check/cover/unreachable counts differ from the raw log")
    expected = {"package": package, "harness": args.harness, "unwind": args.unwind,
                "expected_covers": args.cover_count, "raw_log": "raw.log"}
    problems += [f"{key} differs from the current registration" for key, value in expected.items()
                 if metadata.get(key) != value]
    tools = metadata.get("tools") if isinstance(metadata.get("tools"), dict) else {}
    pinned = {"version": "Kani Rust Verifier " + args.kani_version + " (cargo plugin)\nCBMC " + args.cbmc_version,
              "compiler": args.rustc_version, "host": args.target}
    if any(tools.get(key) != value for key, value in pinned.items()):
        problems.append("recorded Kani/CBMC/compiler/host differ from the pinned model")
    command = metadata.get("command")
    manifest = command[3] if isinstance(command, list) and len(command) > 3 and isinstance(command[3], str) else ""
    origin = Path(manifest).parent if manifest.endswith("/Cargo.toml") else None
    if origin is None or command[1:] != receipt_command(origin, package, args.harness, args.unwind):
        problems.append("recorded verifier argv is not the registered solver/bound/feature model")
    registered = str((root / args.source).resolve())
    inputs = metadata.get("source_inputs")
    if not isinstance(inputs, dict) or len(inputs) != 1 or origin is None \
            or Path(next(iter(inputs))) != origin / args.source:
        problems.append("recorded source is not the registered harness source")
    elif not Path(registered).is_file() or sha256(Path(registered)) != next(iter(inputs.values())):
        problems.append("registered source changed since the run")
    lock = sha256(root / "Cargo.lock")
    if not (metadata.get("lock_before") == metadata.get("lock_after") == lock):
        problems.append("Cargo.lock differs from the run, or changed during it")
    recorded = metadata.get("model_inputs")
    if not isinstance(recorded, dict) or identity_digest(recorded) != metadata.get("model_inputs_sha256") \
            or metadata.get("model_inputs_after_sha256") != metadata.get("model_inputs_sha256"):
        problems.append("recorded package inputs are inconsistent or changed during the run")
    else:
        current = source_tree_identity(root, package)
        if current != recorded:
            problems.append("package inputs changed since the run: " + differences(recorded, current))
    approved = metadata.get("approved_configuration")
    if not isinstance(approved, dict) or origin is None \
            or identity_digest(approved) != metadata.get("approved_configuration_sha256") \
            or metadata.get("configuration_after_sha256") != metadata.get("approved_configuration_sha256"):
        problems.append("recorded configuration is inconsistent or changed during the run")
    else:
        current = portable_configuration(configuration_identity(root, os.environ, package), root)
        before = portable_configuration(approved, origin)
        if current != before:
            problems.append("proof configuration differs from the run: " + differences(before, current))
    if problems:
        raise KaniError("receipt is not CURRENT: " + "; ".join(problems))
    revision = metadata.get("git", {}).get("revision", "unknown") if isinstance(metadata.get("git"), dict) else "unknown"
    print(f"Kani receipt CURRENT: {args.harness}; {parsed['checks']} checks, {parsed['covers']} covers; "
          f"run at revision {revision}. Consistency of one retained run only: not re-executed, not tamper evidence.")
    return parsed


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    for name in ("kani-version", "cbmc-version", "rustc-version", "toolchain", "target"):
        result.add_argument("--" + name, required=True)
    result.add_argument("--probe", action="store_true")
    result.add_argument("--package", default=DEFAULT_PACKAGE)
    result.add_argument("--harness")
    result.add_argument("--source")
    result.add_argument("--unwind", type=int)
    result.add_argument("--cover-count", type=int)
    result.add_argument("--allow-unreachable", action="append", default=[])
    result.add_argument("--approved-config")
    result.add_argument("--check-receipt", metavar="METADATA")
    return result


def main():
    args = parser().parse_args()
    try:
        package_of(args)
        if args.check_receipt:
            if args.probe or args.approved_config or not args.harness or not args.source \
                    or args.unwind != 2 or not args.cover_count or args.cover_count < 1:
                raise KaniError("receipt mode needs the exact registered harness/source/unwind/cover model only")
            check_receipt(args)
        elif args.probe:
            bundle = Path(os.environ.get("KANI_HOME", str(Path.home() / ".kani"))) / ("kani-" + args.kani_version)
            print(probe(bundle, args)["version"])
        else:
            if not args.harness or not args.source or args.unwind != 2 or not args.cover_count or args.cover_count < 1:
                raise KaniError("an exact registered harness/source/unwind/cover model is required")
            verify(args)
        return 0
    except (KaniError, OSError, KeyboardInterrupt) as error:
        print("Kani primitive: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
