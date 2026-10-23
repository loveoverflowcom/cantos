"""Native OCI producer (#348): build the Server/Identity pair, optionally publish it to GHCR.

Local and GitHub Actions producers share this one owner. It builds from a ``git archive`` of one
clean commit, never on the VPS, and publishes only with an explicit private authfile. A published
pair is read back by registry digest with the release owner's verifier. The result is a
*candidate*: every qualification gate is recorded ``NOT_RUN``, so ``release oci-verify`` refuses it
until real gate records exist and an operator admits the producer. Nothing here activates a host.
"""

import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import platform as host_platform
import re
import subprocess
import tarfile
import tempfile
import tomllib
import uuid

import release_artifact as contract
import release_oci as oci
import release_oci_registry as registry

REGISTRY = "ghcr.io/loveoverflowcom/"
CONTAINERFILE = "infra/oci/{}/Containerfile"
COMPATIBILITY = "infra/oci/compatibility.json"
POLICY = ("rust-toolchain.toml", "Cargo.lock", COMPATIBILITY, "scripts/release_oci_build.py",
          "scripts/release_oci.py", "scripts/release_oci_registry.py", ".github/workflows/native-oci-publish.yml",
          *(CONTAINERFILE.format(s) for s in oci.SERVICES))
MACHINES = {"x86_64": "linux/amd64", "aarch64": "linux/arm64"}
GATE_REASON = "producer builds and publishes only; this gate has no execution here"


# Only process/runtime locations, never provider tokens or ambient registry authentication.
CHILD_ENV = ("PATH", "HOME", "TMPDIR", "XDG_RUNTIME_DIR", "XDG_CONFIG_HOME", "XDG_DATA_HOME",
             "XDG_CACHE_HOME", "SSL_CERT_FILE", "SSL_CERT_DIR")


def sh(root, argv, *, env=None):
    env = {key: value for key, value in (os.environ if env is None else env).items() if key in CHILD_ENV}
    result = subprocess.run(argv, cwd=root, check=False, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            env=env, timeout=7200)
    # Never echo output: build logs and push errors can carry registry or credential detail.
    contract.require(result.returncode == 0, "command failed: " + argv[0] + " " + argv[1])
    return result.stdout.decode()


def git_bytes(root, sha, name):
    return oci.git(root, "show", sha + ":" + name)


def compatibility(root, sha):
    """Declared identifiers: tracked policy plus the locked Turso version, never a runtime claim."""
    declared = json.loads(git_bytes(root, sha, COMPATIBILITY))
    contract.require(isinstance(declared, dict) and set(declared) == {"rpc", "server_schema", "identity_schema",
                     "security_state"}, "compatibility policy fields")
    lock = tomllib.loads(git_bytes(root, sha, "Cargo.lock").decode())
    versions = sorted({p["version"] for p in lock["package"] if p["name"] == "turso"})
    contract.require(len(versions) == 1, "exactly one locked Turso version required")
    result = {**declared, "turso": versions[0]}
    for value in result.values():
        oci.identifier(value)
    return result


def configuration_identity(root, sha, platform):
    """Tracked build policy and base-image pins determine the identity; the commit alone does not."""
    policy = {name: hashlib.sha256(git_bytes(root, sha, name)).hexdigest() for name in POLICY}
    return contract.fingerprint({"component": "native-oci", "platform": platform, "profile": "release",
                                 "policy": policy})


def producer_identity(kind, env, source_sha, build_id=None):
    if kind == "local":
        return {"kind": "local", "build_id": oci.identifier(build_id)}
    contract.require(kind == "github-actions" and build_id is None, "unsupported producer kind")
    contract.require(env.get("GITHUB_ACTIONS") == "true" and env.get("GITHUB_REPOSITORY") == "loveoverflowcom/cantos"
                     and env.get("GITHUB_SHA") == source_sha and env.get("GITHUB_WORKFLOW_SHA") == source_sha
                     and env.get("GITHUB_RUN_ATTEMPT") == "1" and env.get("GITHUB_EVENT_NAME") == "workflow_dispatch",
                     "Actions requires the exact source/workflow and a first manual attempt; reconcile reruns")
    ref = env.get("GITHUB_WORKFLOW_REF", "")
    match = re.fullmatch(r"loveoverflowcom/cantos/(\.github/workflows/[a-z0-9-]+\.ya?ml)@refs/.+", ref)
    contract.require(match is not None, "workflow reference must name a repository workflow")
    result = {"kind": "github-actions", "workflow_sha": env.get("GITHUB_WORKFLOW_SHA", ""),
              "workflow_path": match[1], "run_id": env.get("GITHUB_RUN_ID", ""),
              "run_attempt": env.get("GITHUB_RUN_ATTEMPT", ""), "job_id": env.get("GITHUB_JOB", "")}
    oci.producer(result)
    return result


def host_members(root, sha):
    """Committed host assets only, with Git's own mode; anything else is refused, not skipped."""
    listing = subprocess.run(["git", "ls-tree", "-r", "-z", sha], cwd=root, check=True,
                             stdout=subprocess.PIPE).stdout.decode()
    members = [("REVISION", (sha + "\n").encode(), 0o644)]
    for record in filter(None, listing.split("\0")):
        meta, name = record.split("\t", 1)
        mode, kind, _ = meta.split()
        if not oci.host_asset(name) or name == "REVISION":
            continue
        contract.require(kind == "blob" and mode in ("100644", "100755"), "host asset must be a regular blob")
        members.append((name, git_bytes(root, sha, name), 0o755 if mode == "100755" else 0o644))
    contract.require(any(n.startswith("infra/vps/") for n, _, _ in members), "no committed host assets")
    return sorted(members)


def runtime_tar(members):
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode="w", format=tarfile.PAX_FORMAT) as archive:
        for name, data, mode in members:
            entry = tarfile.TarInfo(name)
            entry.size, entry.mode, entry.mtime = len(data), mode, 0
            archive.addfile(entry, io.BytesIO(data))
    return buffer.getvalue()


def bundle_record(data, members):
    return {"path": "runtime.tar", "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest(),
            "entries": [{"path": n, "bytes": len(d), "sha256": hashlib.sha256(d).hexdigest(), "mode": m}
                        for n, d, m in members]}


def check_local_image(service, inspection, sha, configuration, platform):
    """What the registry readback will demand, decided before anything is pushed."""
    os_name, arch = platform.split("/")
    labels = inspection.get("Config", {}).get("Labels") or {}
    config = inspection.get("Config", {})
    contract.require(inspection.get("Os") == os_name and inspection.get("Architecture") == arch
                     and not inspection.get("Variant"), "built image platform mismatch")
    contract.require(labels.get("org.opencontainers.image.source") == contract.REPOSITORY
                     and labels.get("org.opencontainers.image.revision") == sha
                     and labels.get("io.cantos.release.configuration") == configuration
                     and labels.get("io.cantos.binary") == service, "built image identity labels mismatch")
    contract.require(re.fullmatch(r"[1-9][0-9]{0,9}:[1-9][0-9]{0,9}", config.get("User", "")) is not None,
                     "image must run as a numeric non-root user")
    contract.require(config.get("Entrypoint") == ["/usr/local/bin/" + service] and not config.get("Cmd"),
                     "image entrypoint must be exactly the service binary")
    contract.require(config.get("Env") is None or not any(e.split("=", 1)[0].endswith(("PASSWORD", "SECRET", "TOKEN"))
                     for e in config["Env"]), "image environment must carry no credential names")
    return "sha256:" + contract.digest(inspection["Id"].removeprefix("sha256:"))


def candidate(sha, configuration, platform, producer, compat, images, members_data):
    """Pure assembly. Gates stay NOT_RUN: this record can never satisfy `release oci-verify`."""
    members = members_data
    bundle = runtime_tar(members)
    records = {gate: {"gate": gate, "status": "NOT_RUN", "reason": GATE_REASON,
                      "source_sha": sha} for gate in oci.GATES}
    record_bytes = {gate: contract.canonical(value) + b"\n" for gate, value in records.items()}
    evidence = {"schema_version": 1, "source_sha": sha, "configuration_sha256": configuration,
                "platform": platform, "images": images, "producer": producer,
                "gates": {gate: {"status": "NOT_RUN", "record_sha256": hashlib.sha256(data).hexdigest()}
                          for gate, data in record_bytes.items()}}
    evidence_bytes = contract.canonical(evidence) + b"\n"
    manifest = {"schema_version": 1, "kind": "cantos-native-oci", "repository": contract.REPOSITORY,
                "source_sha": sha, "configuration_sha256": configuration, "platform": platform,
                "producer": producer, "compatibility": compat, "images": images,
                "runtime_bundle": bundle_record(bundle, members),
                "evidence": {"path": "evidence.json", "bytes": len(evidence_bytes),
                             "sha256": hashlib.sha256(evidence_bytes).hexdigest()}}
    return manifest, bundle, evidence_bytes, record_bytes


def write_candidate(output, manifest, bundle, evidence, records):
    output.mkdir(parents=True, exist_ok=False)
    (output / "gate-records").mkdir()
    for name, data in {"runtime.tar": bundle, "evidence.json": evidence}.items():
        with (output / name).open("xb") as stream:
            stream.write(data)
    for gate, data in records.items():
        with (output / "gate-records" / (gate + ".json")).open("xb") as stream:
            stream.write(data)
    contract.write_new(output / "manifest.json", manifest)
    return hashlib.sha256((output / "manifest.json").read_bytes()).hexdigest()


def preflight(root, platform, which=host_platform):
    contract.require(platform in oci.PLATFORMS, "platform must be linux/amd64 or linux/arm64")
    contract.require(which.system() == "Linux" and MACHINES.get(which.machine()) == platform,
                     "build host must natively match the target platform; no emulation or VPS build")
    sha = sh(root, ["git", "rev-parse", "HEAD"]).strip()
    contract.sha(sha)
    contract.require(not sh(root, ["git", "status", "--porcelain", "--untracked-files=normal"]).strip(),
                     "oci-build requires a clean tracked and untracked source tree")
    return sha


def build_pair(root, sha, configuration, platform, work):
    context = work / "context"
    context.mkdir()
    sh(root, ["git", "archive", "--format=tar", "-o", str(work / "source.tar"), sha])
    sh(root, ["tar", "-xf", str(work / "source.tar"), "-C", str(context)])
    local = {}
    for service in oci.SERVICES:
        tag = "localhost/cantos-oci/" + service + ":" + sha
        sh(root, ["podman", "build", "--format", "oci", "--platform", platform, "--pull=missing",
                  "-f", str(context / CONTAINERFILE.format(service)), "--build-arg", "CANTOS_REVISION=" + sha,
                  "--build-arg", "CANTOS_CONFIGURATION=" + configuration, "-t", tag, str(context)])
        inspection = json.loads(sh(root, ["podman", "image", "inspect", tag]))[0]
        local[service] = (tag, check_local_image(service, inspection, sha, configuration, platform))
    return local


def push(root, service, image_id, sha, authfile, work):
    """Push once. A failure after bytes may have left the registry changed: reconcile, never retry."""
    digestfile = work / (service + ".digest")
    sh(root, ["podman", "push", "--format", "oci", "--authfile", str(authfile), "--digestfile", str(digestfile),
              image_id, "docker://" + REGISTRY + service + ":sha-" + sha])
    return "sha256:" + contract.digest(digestfile.read_text().strip().removeprefix("sha256:"))


def private_attempt_directory(path):
    """Persist each new directory entry, so a host crash cannot lose the pre-push fence."""
    missing = []
    for parent in (path, *path.parents):
        contract.require(not parent.is_symlink(), "symlink in publication output path")
        if not parent.exists():
            missing.append(parent)
    for parent in reversed(missing):
        parent.mkdir(mode=0o700)
        descriptor = os.open(parent.parent, os.O_RDONLY)
        try:
            os.fsync(descriptor)
        finally:
            os.close(descriptor)


def save_attempt(output, record):
    """Atomically replace the run-owned journal; fsync intent before each possible remote write."""
    temporary = output / "publish-attempt.next"
    with temporary.open("xb") as stream:
        stream.write(contract.canonical(record) + b"\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, output / "publish-attempt.json")
    descriptor = os.open(output, os.O_RDONLY)
    try:
        os.fsync(descriptor)
    finally:
        os.close(descriptor)


def produce(root, args, env=os.environ):
    sha = preflight(root, args.platform)
    producer = producer_identity(args.producer, env, sha, args.build_id)
    configuration = configuration_identity(root, sha, args.platform)
    compat = compatibility(root, sha)
    members = host_members(root, sha)
    if args.publish:
        contract.require(args.authfile is not None, "explicit private authfile required")
        registry.load_authfile(args.authfile)  # Refuse unsafe files/helpers before any build or push.
    with tempfile.TemporaryDirectory(prefix="cantos-oci-build-") as temporary:
        work = Path(temporary)
        private_auth = None
        if args.publish:
            # A validated private snapshot, outside the source/build context, is used throughout.
            private_auth = work / "auth.json"
            record = {**contract.file_record(args.authfile, args.authfile.parent), "path": private_auth.name}
            contract.copy_verified(args.authfile, private_auth, record, limit=registry.MAX_AUTH_BYTES)
            private_auth.chmod(0o600)
            registry.load_authfile(private_auth)
        local = build_pair(root, sha, configuration, args.platform, work)
        contract.require(preflight(root, args.platform) == sha, "source changed during the build")
        if not args.publish:
            return {"kind": "cantos-native-oci-build", "operation": "BUILD_ONLY", "source_sha": sha,
                    "configuration_sha256": configuration, "platform": args.platform,
                    "images": {s: {"config_digest": local[s][1]} for s in oci.SERVICES},
                    "gates": {"build": "PASS", "publish": "NOT_RUN", "registry-readback": "NOT_RUN"}}
        output = root / args.output / sha / configuration / uuid.uuid4().hex
        contract.require(not output.exists(), "fresh publication attempt required")
        private_attempt_directory(output)
        attempt = {"schema_version": 1, "kind": "cantos-native-oci-publish-attempt",
                   "source_sha": sha, "configuration_sha256": configuration, "platform": args.platform,
                   "producer": producer, "release_eligible": False, "registry_readback": "NOT_RUN",
                   "images": {s: {"state": "not_started", "tag": REGISTRY + s + ":sha-" + sha,
                                  "config_digest": local[s][1], "digest": None} for s in oci.SERVICES}}
        save_attempt(output, attempt)
        images = {}
        for service in oci.SERVICES:
            attempt["images"][service]["state"] = "unknown"
            save_attempt(output, attempt)  # A crash from here onward requires reconciliation, never replay.
            digest = push(root, service, local[service][1], sha, private_auth, work)
            attempt["images"][service].update(state="confirmed", digest=digest)
            save_attempt(output, attempt)
            images[service] = {"ref": REGISTRY + service + "@" + digest, "config_digest": local[service][1]}
        manifest, bundle, evidence, records = candidate(sha, configuration, args.platform, producer, compat,
                                                       images, members)
        admission = {"schema_version": 1,
                     "manifest_sha256": hashlib.sha256(contract.canonical(manifest) + b"\n").hexdigest(),
                     "producer": producer}
        plan = oci.manifest_plan(manifest, admission, manifest_digest=admission["manifest_sha256"], source_sha=sha,
                                configuration_digest=configuration, platform=args.platform)
        # Preserve both registry digests and the unadmitted candidate even when readback is unavailable.
        digest = write_candidate(output / "candidate", manifest, bundle, evidence, records)
        oci.verify_bundle(root, output / "candidate/runtime.tar", manifest)
        attempt["registry_readback"] = "unknown"
        save_attempt(output, attempt)
        observed = registry.observe(manifest, plan, authfile=private_auth)
        attempt["registry_readback"] = "PASS"
        save_attempt(output, attempt)
    return {**observed, "kind": "cantos-native-oci-candidate", "operation": "CANDIDATE",
            "manifest_sha256": digest, "output": str(output / "candidate"),
            "gates": {**observed["gates"], "publish": "PASS", "native-qualification": "NOT_RUN"}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--producer", choices=("local", "github-actions"), required=True)
    parser.add_argument("--build-id")
    parser.add_argument("--publish", action="store_true", help="push to GHCR; requires --authfile")
    parser.add_argument("--authfile", type=Path)
    parser.add_argument("--output", type=Path, default=Path("artifacts/release-build/oci"))
    args = parser.parse_args()
    if args.publish != (args.authfile is not None):
        parser.error("--publish and --authfile are required together")
    if (args.producer == "local") != (args.build_id is not None):
        parser.error("--build-id is required for, and only for, the local producer")
    try:
        result = produce(Path(__file__).resolve().parents[1], args)
    except Exception as error:  # noqa: BLE001 - report a safe class, never remote or credential text
        parser.exit(1, "native OCI build refused: " + type(error).__name__ +
                    "; inspect retained publish-attempt.json before retrying; remote outcome may be unknown\n")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
