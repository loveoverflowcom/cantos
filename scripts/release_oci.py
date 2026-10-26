"""Native OCI release v1: offline contract verification owned by xtask release.

This is the first #348 release-contract slice, not a publisher or deploy executor.
An explicit operator admission is the trust boundary. Producer assertions, labels
and checksums alone are not trusted provenance or real registry/runtime evidence.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import subprocess
import tarfile
import tempfile
import time

import release_artifact as contract


SERVICES = ("cantos-server", "cantos-identity")
PLATFORMS = ("linux/amd64", "linux/arm64")
GATES = ("native-isolation", "native-e2e", "migration-restore", "image-pair")
MAX_BUNDLE_BYTES = 16 * 1024**2
MAX_BUNDLE_ENTRIES = 256


def fields(value, names, reason):
    contract.require(isinstance(value, dict) and set(value) == set(names.split()), reason)


def identifier(value):
    contract.require(isinstance(value, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,99}", value),
                     "bounded nonsecret identifier required")
    return value


def oci_digest(value):
    contract.require(isinstance(value, str) and value.startswith("sha256:"), "OCI sha256 digest required")
    contract.digest(value[7:])
    return value


def image_ref(value, service):
    prefix = "ghcr.io/loveoverflowcom/" + service + "@"
    contract.require(isinstance(value, str) and value.startswith(prefix), "exact service GHCR digest ref required")
    oci_digest(value[len(prefix):])
    return value


def host_asset(name):
    """Only tracked helpers and configuration templates, never data or private config."""
    if name in ("REVISION", "infra/edge/Caddyfile", "infra/vps/config.example.env"):
        return True
    if re.fullmatch(r"infra/vps/bin/cantos-[a-z0-9.-]+", name):
        return True
    return bool(re.fullmatch(r"infra/vps/(?:quadlet/[a-z0-9.-]+\.(?:container|network)|"
                             r"systemd/[a-z0-9.-]+\.(?:service|timer)|"
                             r"config/[a-z0-9.-]+\.toml|kanidm/server\.toml\.template)", name))


def producer(value):
    contract.require(isinstance(value, dict), "explicit producer required")
    kind = value.get("kind")
    if kind == "local":
        fields(value, "kind build_id", "invalid local producer")
        identifier(value["build_id"])
    elif kind == "github-actions":
        fields(value, "kind workflow_sha workflow_path run_id run_attempt job_id", "invalid Actions producer")
        contract.sha(value["workflow_sha"])
        contract.require(isinstance(value["workflow_path"], str) and
                         re.fullmatch(r"\.github/workflows/[a-z0-9-]+\.ya?ml", value["workflow_path"]),
                         "repository workflow path required")
        for name in ("run_id", "run_attempt"):
            contract.require(isinstance(value[name], str) and re.fullmatch(r"[1-9][0-9]{0,19}", value[name]),
                             "positive Actions run identity required")
        identifier(value["job_id"])
    else:
        raise ValueError("producer must be local or github-actions; existing is a consumer lane")


def candidate_plan(manifest, *, manifest_digest, source_sha, configuration_digest, platform):
    """Validate candidate identity without inventing an operator admission."""
    fields(manifest, "schema_version kind repository source_sha configuration_sha256 platform producer "
           "compatibility images runtime_bundle evidence", "unsupported native OCI manifest fields")
    contract.require(type(manifest["schema_version"]) is int and manifest["schema_version"] == 1 and
                     manifest["kind"] == "cantos-native-oci" and manifest["repository"] == contract.REPOSITORY,
                     "unsupported native OCI manifest")
    contract.digest(manifest_digest)
    contract.require(manifest["source_sha"] == contract.sha(source_sha), "source SHA mismatch")
    contract.require(manifest["configuration_sha256"] == contract.digest(configuration_digest),
                     "configuration identity mismatch")
    contract.require(platform in PLATFORMS and manifest["platform"] == platform, "platform mismatch or unsupported")
    producer(manifest["producer"])
    compatibility = manifest["compatibility"]
    fields(compatibility, "rpc server_schema identity_schema security_state turso", "incomplete compatibility identity")
    for value in compatibility.values():
        identifier(value)
    images = manifest["images"]
    fields(images, "cantos-server cantos-identity", "exact Server/Identity image pair required")
    for service in SERVICES:
        item = images[service]
        fields(item, "ref config_digest", "single-platform image ref and config digest required; indexes unsupported in v1")
        image_ref(item["ref"], service)
        oci_digest(item["config_digest"])
        contract.require(item["ref"].split("@", 1)[1] != item["config_digest"],
                         "registry manifest digest cannot be the config/image ID")
    bundle = manifest["runtime_bundle"]
    fields(bundle, "path bytes sha256 entries", "invalid runtime bundle record")
    contract.require(bundle["path"] == "runtime.tar", "fixed runtime.tar name required")
    contract.digest(bundle["sha256"])
    contract.require(type(bundle["bytes"]) is int and 0 < bundle["bytes"] <= MAX_BUNDLE_BYTES,
                     "bounded host bundle required")
    entries = bundle["entries"]
    contract.require(isinstance(entries, list) and 2 <= len(entries) <= MAX_BUNDLE_ENTRIES,
                     "bounded complete host bundle entries required")
    names = set()
    total = 0
    for entry in entries:
        fields(entry, "path bytes sha256 mode", "invalid bundle member record")
        name = contract.safe_name(entry["path"])
        contract.require(host_asset(name), "unexpected host bundle asset")
        contract.require(name not in names, "duplicate bundle member record")
        names.add(name)
        contract.digest(entry["sha256"])
        contract.require(type(entry["bytes"]) is int and 0 <= entry["bytes"] <= MAX_BUNDLE_BYTES,
                         "invalid bundle member size")
        total += entry["bytes"]
        contract.require(type(entry["mode"]) is int and entry["mode"] in (0o644, 0o755),
                         "bundle modes must be 0644 or 0755")
        if name == "infra/vps/bin/cantos-vps":
            contract.require(entry["mode"] == 0o755, "runtime entrypoint must be executable (#313)")
    contract.require("REVISION" in names and any(n.startswith("infra/vps/") for n in names)
                     and total <= MAX_BUNDLE_BYTES, "incomplete or oversized host bundle")
    evidence = manifest["evidence"]
    fields(evidence, "path bytes sha256", "invalid producer evidence record")
    contract.require(evidence["path"] == "evidence.json" and type(evidence["bytes"]) is int and
                     0 < evidence["bytes"] <= contract.MAX_JSON, "bounded evidence.json required")
    contract.digest(evidence["sha256"])
    return {"schema_version": 1, "kind": "cantos-native-oci-offline-plan", "operation": "PLAN_ONLY",
            "source_sha": source_sha, "configuration_sha256": configuration_digest, "platform": platform,
            "manifest_sha256": manifest_digest, "images": {s: images[s]["ref"] for s in SERVICES},
            "gates": {"offline-contract": "PASS", "registry-access": "NOT_RUN",
                      "native-qualification": "NOT_RUN", "activation": "NOT_RUN", "cleanup": "NOT_RUN"}}


def manifest_plan(manifest, admission, **pins):
    """Admission remains mandatory for every existing release/host consumer."""
    plan = candidate_plan(manifest, **pins)
    fields(admission, "schema_version manifest_sha256 producer", "invalid producer admission fields")
    contract.require(type(admission["schema_version"]) is int and admission["schema_version"] == 1 and
                     admission["manifest_sha256"] == pins["manifest_digest"] and
                     admission["producer"] == manifest["producer"], "producer admission mismatch")
    return plan


def git(root, *args):
    # Object reads in a partial clone may otherwise fetch, authenticate and write
    # packs. An unsupported global option fails before the object command: never
    # fall back to an older Git that silently ignores an environment-only guard.
    env = dict(os.environ, GIT_NO_LAZY_FETCH="1", GIT_OPTIONAL_LOCKS="0",
               GIT_NO_REPLACE_OBJECTS="1", GIT_ALLOW_PROTOCOL="", GIT_TERMINAL_PROMPT="0")
    result = subprocess.run(["git", "--no-lazy-fetch", "--no-optional-locks", "--no-replace-objects", *args],
                            cwd=root, check=False, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=30, env=env)
    contract.require(result.returncode == 0,
                     "Git --no-lazy-fetch support and locally present source objects required")
    return result.stdout


def verify_bundle(root, path, manifest):
    """Validate a private byte snapshot; never extract or execute bundle members."""
    bundle = manifest["runtime_bundle"]
    expected = {k: bundle[k] for k in ("path", "bytes", "sha256")}
    records = {entry["path"]: entry for entry in bundle["entries"]}
    source_sha = manifest["source_sha"]
    contract.require(git(root, "cat-file", "-t", source_sha) == b"commit\n",
                     "source SHA must name a commit object")
    with tempfile.TemporaryDirectory(prefix="cantos-oci-verify-", dir=Path(tempfile.gettempdir()).resolve()) as temporary:
        snapshot = Path(temporary) / "runtime.tar"
        contract.copy_verified(path, snapshot, expected, limit=MAX_BUNDLE_BYTES)
        with tarfile.open(snapshot, "r:") as archive:
            entries = contract.archive_members(archive)
            contract.require(len(entries) == len(records) and {e.name for e in entries} == set(records),
                             "bundle member set mismatch")
            for entry in entries:
                record = records[entry.name]
                contract.require(entry.size == record["bytes"] and entry.mode == record["mode"],
                                 "bundle member size/mode mismatch")
                with archive.extractfile(entry) as stream:
                    data = stream.read(MAX_BUNDLE_BYTES + 1)
                contract.require(hashlib.sha256(data).hexdigest() == record["sha256"], "bundle member hash mismatch")
                if entry.name == "REVISION":
                    contract.require(data == (source_sha + "\n").encode() and entry.mode == 0o644,
                                     "bundle source revision mismatch")
                    continue
                tree = git(root, "ls-tree", source_sha, "--", entry.name).decode().split()
                mode = "100755" if entry.mode == 0o755 else "100644"
                contract.require(len(tree) == 4 and tree[0] == mode and tree[1] == "blob" and tree[3] == entry.name,
                                 "bundle member differs from committed path/mode")
                contract.require(git(root, "show", source_sha + ":" + entry.name) == data,
                                 "bundle member differs from committed source bytes")


def verify_evidence(path, manifest):
    record = manifest["evidence"]
    contract.require(contract.file_record(path, path.parent) == record, "producer evidence bytes/hash mismatch")
    evidence = contract.read_json(path, record["sha256"])
    fields(evidence, "schema_version source_sha configuration_sha256 platform images producer gates", "invalid producer evidence")
    contract.require(type(evidence["schema_version"]) is int and evidence["schema_version"] == 1 and
                     all(evidence[key] == manifest[key] for key in
                         ("source_sha", "configuration_sha256", "platform", "images", "producer")),
                     "producer evidence release identity mismatch")
    fields(evidence["gates"], " ".join(GATES), "complete native and post-publish gate evidence required")
    for gate in GATES:
        item = evidence["gates"][gate]
        fields(item, "status record_sha256", "gate status and immutable evidence record required")
        contract.require(item["status"] == "PASS", "producer gate did not PASS: " + gate)
        contract.digest(item["record_sha256"])
        record = contract.read_json(path.parent / "gate-records" / (gate + ".json"), item["record_sha256"])
        contract.require(isinstance(record, dict) and record.get("gate") == gate and record.get("status") == "PASS" and
                         all(record.get(key) == manifest[key] for key in
                             ("source_sha", "configuration_sha256", "platform", "images")),
                         "qualification record release identity mismatch: " + gate)


def host_plan(inventory, manifest, *, now):
    """Pure, operator-declared inventory plan. It grants no host/lock/activation authority."""
    fields(inventory, "schema_version host_id observed_at state platform cgroup_version services "
           "socket_directory journal lock current_manifest_sha256 previous_manifest_sha256 compatibility",
           "invalid host inventory fields")
    contract.require(type(inventory["schema_version"]) is int and inventory["schema_version"] == 1,
                     "unsupported host inventory")
    identifier(inventory["host_id"])
    contract.require(type(inventory["observed_at"]) is int and
                     0 <= now - inventory["observed_at"] <= 86400, "inventory must be observed within 24 hours")
    contract.require(inventory["state"] in ("UNKNOWN", "NEW", "LEGACY", "NATIVE_READY"), "invalid host state")
    contract.require(inventory["platform"] in PLATFORMS, "invalid host platform")
    contract.require(inventory["journal"] in ("UNKNOWN", "IDLE", "IN_PROGRESS", "RECOVERY_REQUIRED") and
                     inventory["lock"] in ("UNKNOWN", "AVAILABLE", "HELD"), "invalid host operation state")
    for key in ("current_manifest_sha256", "previous_manifest_sha256"):
        if inventory[key] is not None:
            contract.digest(inventory[key])
    def directory(value):
        contract.require(isinstance(value, str) and len(value) <= 200 and
                         re.fullmatch(r"/[A-Za-z0-9_./-]+", value) and
                         all(part not in ("", ".", "..") for part in value.split("/")[1:]),
                         "canonical absolute nonsecret directory required")
        return PurePosixPath(value)
    socket = directory(inventory["socket_directory"])
    fields(inventory["services"], " ".join(SERVICES), "exact host service ownership required")
    owners, databases = {}, []
    for service in SERVICES:
        item = inventory["services"][service]
        fields(item, "runtime_mode principal image_store quadlet_scope database_directory", "invalid service ownership")
        contract.require(item["runtime_mode"] in ("rootful", "rootless"), "explicit runtime mode required")
        identifier(item["principal"])
        identifier(item["image_store"])
        contract.require((item["runtime_mode"] == "rootful" and item["principal"] == "root" and
                          item["quadlet_scope"] == "system") or
                         (item["runtime_mode"] == "rootless" and item["principal"] != "root" and
                          item["quadlet_scope"] == "user"), "runtime principal and Quadlet scope mismatch")
        database = directory(item["database_directory"])
        for other in [socket, *databases]:
            contract.require(database != other and database not in other.parents and other not in database.parents,
                             "database and socket directories must be separate and non-overlapping")
        databases.append(database)
        owners[service] = {**item, "image": manifest["images"][service]["ref"]}
    blockers = []
    if inventory["state"] == "UNKNOWN":
        blockers.append("HOST_UNKNOWN")
    elif inventory["state"] == "LEGACY":
        blockers.append("EXPLICIT_CUTOVER_AND_NATIVE_RECOVERY_ACCEPTANCE_REQUIRED")
    elif inventory["state"] == "NEW":
        blockers.append("EXPLICIT_BOOTSTRAP_AND_DATA_TIER_APPROVAL_REQUIRED")
    if inventory["platform"] != manifest["platform"]:
        blockers.append("PLATFORM_MISMATCH")
    if type(inventory["cgroup_version"]) is not int or inventory["cgroup_version"] != 2:
        blockers.append("CGROUP_V2_REQUIRED")
    if inventory["compatibility"] != manifest["compatibility"]:
        blockers.append("SCHEMA_RPC_SECURITY_COMPATIBILITY_NOT_CONFIRMED")
    if inventory["journal"] != "IDLE" or inventory["lock"] != "AVAILABLE":
        blockers.append("RECONCILE_OPERATION_BEFORE_PREPARATION")
    if inventory["state"] == "NATIVE_READY" and inventory["current_manifest_sha256"] is None:
        blockers.append("CURRENT_RELEASE_UNKNOWN")
    return {"host_id": inventory["host_id"], "inventory_kind": "OPERATOR_DECLARED",
            "declared_host_state": inventory["state"], "readiness": "BLOCKED" if blockers else "REVIEW_REQUIRED",
            "blockers": blockers, "services": owners, "socket_directory": str(socket),
            "current_manifest_sha256": inventory["current_manifest_sha256"],
            "previous_manifest_sha256": inventory["previous_manifest_sha256"],
            "activation": "NOT_RUN", "cleanup": "NOT_RUN", "release_eligible": False,
            "remaining": ["fresh authenticated host observation", "Podman/Quadlet version and UID mapping qualification",
                          "capacity and image-store checks", "native bundle renderer and lock/generation consumer",
                          "migration/health/reboot/recovery rehearsal", "separate operator activation authority"]}


def verify(root, args):
    # Validate caller pins before reading untrusted records. No registry/SSH command exists here.
    contract.digest(args.manifest_digest)
    contract.digest(args.admission_digest)
    contract.sha(args.source_sha)
    contract.digest(args.configuration_digest)
    contract.require(args.platform in PLATFORMS, "platform mismatch or unsupported")
    manifest = contract.read_json(args.manifest, args.manifest_digest)
    admission = contract.read_json(args.admission, args.admission_digest)
    plan = manifest_plan(manifest, admission, manifest_digest=args.manifest_digest, source_sha=args.source_sha,
                         configuration_digest=args.configuration_digest, platform=args.platform)
    verify_bundle(root, args.manifest.parent / "runtime.tar", manifest)
    verify_evidence(args.manifest.parent / "evidence.json", manifest)
    if getattr(args, "host_inventory", None) is not None:
        contract.digest(args.host_inventory_digest)
        inventory = contract.read_json(args.host_inventory, args.host_inventory_digest)
        plan["host_plan"] = host_plan(inventory, manifest, now=time.time())
    return plan


def execute(root, args):
    # Admission, source/bundle and declared qualification guards run before a
    # network client or private authfile can be opened. Dry-run is strictly offline.
    plan = verify(root, args)
    if not getattr(args, "registry_readback", False) or args.dry_run:
        return plan
    import release_oci_registry
    manifest = contract.read_json(args.manifest, args.manifest_digest)
    return release_oci_registry.observe(manifest, plan, authfile=args.authfile)


def main():
    import release_oci_registry
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("manifest", "admission"):
        parser.add_argument("--" + name, required=True, type=Path)
    for name in ("manifest-digest", "admission-digest", "source-sha", "configuration-digest", "platform"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--registry-readback", action="store_true")
    parser.add_argument("--authfile", type=Path)
    parser.add_argument("--host-inventory", type=Path)
    parser.add_argument("--host-inventory-digest")
    args = parser.parse_args()
    if (args.host_inventory is None) != (args.host_inventory_digest is None):
        parser.error("--host-inventory and --host-inventory-digest are required together")
    if args.authfile and not args.registry_readback:
        parser.error("--authfile requires --registry-readback")
    try:
        plan = execute(Path(__file__).resolve().parents[1], args)
    except release_oci_registry.ReadbackError as error:
        parser.exit(1, "native OCI registry readback refused: " + str(error) + "\n")
    except (ValueError, KeyError, TypeError, OSError, tarfile.TarError, subprocess.TimeoutExpired):
        # Do not echo arbitrary manifest content, paths, credentials or subprocess output.
        parser.exit(1, "native OCI offline verification refused; check the admitted contract and local source assets\n")
    print(json.dumps(plan, sort_keys=True))


if __name__ == "__main__":
    main()
