"""Version-2 release contract. Hashes bind bytes; the caller must trust the producer.

Only reviewed, same-workflow CircleCI workspaces or explicitly acknowledged local
artifacts are accepted. There is deliberately no cross-pipeline/latest lookup.
"""

import hashlib
import ipaddress
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import stat
import tarfile
import tempfile
import time
from urllib.parse import urlsplit


REPOSITORY = "https://github.com/loveoverflowcom/cantos"
MAX_ENTRIES = 10000
MAX_BYTES = 2 * 1024**3
MAX_JSON = 2 * 1024**2
MAX_AGE = 24 * 3600
WEB_SUFFIXES = {".html", ".js", ".wasm", ".css", ".svg", ".png", ".ico",
                ".webp", ".woff", ".woff2", ".ttf", ".json", ".txt"}
EXPECTED = {
    "api": {"cantos-api.tar.zst", "image.json", "runtime.tar"},
    "theatre-web": {"theatre-web.tar.gz"},
    "studio-web": {"studio-web.tar.gz"},
    "android": {"androidApp-release-unsigned.apk", "androidApp-release.aab",
                "artifact-inspection.txt", "resolved-dependencies.txt", "mapping.txt",
                "pre-shrink-inputs.json", "release-evidence.json", "audit-release-unsigned.apk",
                "audit-release.aab", "audit-inspection.txt", "audit-dependencies.txt"},
}


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def sha(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{40}", value), "full source SHA required")
    return value


def digest(value):
    require(isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value), "SHA-256 required")
    return value


def image_id(value):
    # Podman reports a bare config ID; the release contract uses the explicit algorithm.
    require(isinstance(value, str), "invalid image ID")
    return "sha256:" + digest(value.removeprefix("sha256:"))


def api_origin(value):
    require(isinstance(value, str), "explicit HTTPS API origin required")
    parsed = urlsplit(value)
    require(parsed.scheme == "https" and parsed.netloc and not parsed.path and not parsed.query
            and not parsed.fragment and not parsed.username and not parsed.password,
            "exact HTTPS API origin required")
    require(re.fullmatch(r"[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?(?::[1-9][0-9]{0,4})?", parsed.netloc),
            "canonical lowercase HTTPS API origin required")
    require(parsed.port is None or parsed.port <= 65535, "invalid HTTPS port")
    require(all(re.fullmatch(r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?", label)
                for label in parsed.hostname.split(".")), "invalid API hostname")
    port = "" if parsed.port is None or parsed.port == 443 else ":" + str(parsed.port)
    require(value == "https://" + parsed.hostname + port, "exact canonical HTTPS API origin required")
    last_label = parsed.hostname.rsplit(".", 1)[-1]
    if last_label.isdigit() or re.fullmatch(r"0x[0-9a-f]*", last_label):
        try:
            address = ipaddress.IPv4Address(parsed.hostname)
        except ipaddress.AddressValueError:
            raise ValueError("canonical API hostname required") from None
        require(str(address) == parsed.hostname, "canonical API hostname required")
    return value


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()


def fingerprint(value):
    return hashlib.sha256(canonical(value)).hexdigest()


def regular(path, limit=MAX_BYTES):
    # Check every parent too; a regular child of a symlink is not trusted input.
    for part in (path, *path.parents):
        require(not part.is_symlink(), "symlink in release path")
    info = path.stat()
    require(stat.S_ISREG(info.st_mode) and info.st_nlink == 1 and 0 < info.st_size <= limit,
            "artifact must be a bounded nonempty regular file without hardlinks")
    return info


def file_record(path, output):
    before = regular(path)
    hasher = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            hasher.update(chunk)
    after = path.stat()
    require((after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns) ==
            (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns),
            "artifact changed during hashing")
    return {"path": path.relative_to(output).as_posix(), "bytes": before.st_size,
            "sha256": hasher.hexdigest()}


def read_json(path, expected_digest=None):
    before = regular(path, MAX_JSON)
    with path.open("rb") as handle:
        data = handle.read(MAX_JSON + 1)
    require(len(data) <= MAX_JSON and path.stat().st_mtime_ns == before.st_mtime_ns
            and path.stat().st_size == len(data), "JSON changed or exceeds release limit")
    if expected_digest:
        require(hashlib.sha256(data).hexdigest() == expected_digest, "manifest digest mismatch")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            require(key not in result, "duplicate JSON key")
            result[key] = value
        return result
    return json.loads(data, object_pairs_hook=unique)


def write_new(path, value):
    with path.open("xb") as handle:
        handle.write(canonical(value) + b"\n")


def safe_name(name):
    require(isinstance(name, str) and len(name) <= 512 and "\\" not in name,
            "invalid archive path")
    parts = PurePosixPath(name).parts
    require(parts and not name.startswith("/") and all(p not in (".", "..") for p in name.split("/"))
            and not any(ord(c) < 32 for c in name) and ":" not in name,
            "unsafe archive path")
    return name


def archive_members(archive, *, web=False, runtime=False, image=False):
    entries, names, directories, total = [], set(), set(), 0
    for entry in archive:
        name = entry.name.rstrip("/") if image and entry.isdir() else entry.name
        safe_name(name)
        if image and entry.isdir():
            require(name not in directories and name not in names, "duplicate archive entry")
            directories.add(name)
            require(len(directories) <= MAX_ENTRIES, "archive exceeds release limits")
            continue
        require(entry.isfile() and not entry.issym() and not entry.islnk(), "archive entry must be regular")
        require(entry.name not in names and entry.name not in directories, "duplicate archive entry")
        names.add(entry.name)
        total += entry.size
        require(len(names) + len(directories) <= MAX_ENTRIES and 0 <= entry.size <= MAX_BYTES and total <= MAX_BYTES,
                "archive exceeds release limits")
        if web:
            require(PurePosixPath(entry.name).suffix in WEB_SUFFIXES and
                    not any(p.startswith(".") for p in PurePosixPath(entry.name).parts),
                    "unexpected Web asset type")
        if runtime:
            require(entry.name == "REVISION" or entry.name.startswith("infra/vps/") or
                    entry.name in {"infra/edge/Caddyfile", "services/cantos-api/schema/legacy-epoch-1-manifest.json"},
                    "unexpected runtime asset")
        entries.append(entry)
    # Reject file/directory prefix conflicts before writing any member.
    require(not any(str(parent) in names for name in names for parent in PurePosixPath(name).parents
                    if str(parent) != "."), "archive path prefix collision")
    require(all(any(name.startswith(directory + "/") for name in names) for directory in directories),
            "unexpected image archive directory")
    if web:
        require("index.html" in names, "Web build has no index.html")
    if runtime:
        require({"REVISION", "infra/vps/bin/cantos-vps", "infra/edge/Caddyfile",
                 "services/cantos-api/schema/legacy-epoch-1-manifest.json"} <= names, "incomplete runtime bundle")
    return entries


def copy_verified(source, destination, expected):
    regular(source)
    with source.open("rb") as stream, destination.open("xb") as target:
        total = 0
        while chunk := stream.read(1024 * 1024):
            total += len(chunk)
            require(total <= MAX_BYTES, "artifact changed or exceeds release limit")
            target.write(chunk)
    require(file_record(destination, destination.parent) == expected, "artifact changed before staging")


def extract(archive_path, destination, *, web=False, runtime=False, expected=None):
    if expected is not None:
        # Never extract different bytes after validation, even if the input is replaced.
        with tempfile.TemporaryDirectory(prefix="cantos-verified-archive-") as temporary:
            snapshot = Path(temporary) / archive_path.name
            copy_verified(archive_path, snapshot, expected)
            return extract(snapshot, destination, web=web, runtime=runtime)
    regular(archive_path)
    with tarfile.open(archive_path, "r:*") as archive:
        entries = archive_members(archive, web=web, runtime=runtime)
        destination.mkdir(parents=True, exist_ok=False)
        for entry in entries:
            path = destination / entry.name
            path.parent.mkdir(parents=True, exist_ok=True)
            with archive.extractfile(entry) as source, path.open("xb") as target:
                shutil.copyfileobj(source, target)
            path.chmod(0o700 if runtime and entry.mode & 0o111 else 0o600)


def configuration(root, component, origin):
    # Tracked policy and toolchain bytes participate in the configuration identity.
    paths = ["rust-toolchain.toml", "scripts/release_build.py", "scripts/release_artifact.py"]
    if component == "api":
        paths += ["infra/vps/image/Containerfile"]
    elif component.endswith("-web"):
        paths += ["infra/firebase/hosting/firebase.json", f"apps/cantos-{component[:-4]}/web/Trunk.toml"]
    else:
        paths += ["apps/cantos-theatre/mobile/androidApp/build.gradle.kts",
                  "scripts/android_release_evidence.py", "scripts/android_boundary.py",
                  "scripts/inspect_android_release.py", "xtask/src/theatre_host.rs"]
    return {"component": component, "api_origin": origin, "profile": "release",
            "platform": "linux/amd64" if component == "api" else component,
            "policy": {p: file_record(root / p, root)["sha256"] for p in paths}}


def ci_identity(env=os.environ):
    keys = {"pipeline_id": "CANTOS_PIPELINE_ID", "workflow_id": "CIRCLE_WORKFLOW_ID",
            "job_id": "CIRCLE_WORKFLOW_JOB_ID", "job_number": "CIRCLE_BUILD_NUM",
            "pipeline_number": "CANTOS_PIPELINE_NUMBER"}
    values = {key: env.get(name) for key, name in keys.items()}
    require(all(isinstance(v, str) and re.fullmatch(r"[A-Za-z0-9-]{1,100}", v) for v in values.values()),
            "complete CircleCI pipeline/workflow/job identity required")
    return values


def config_identity(env=os.environ):
    # CIRCLE_SHA1 is the checkout revision, not necessarily the configuration revision.
    # Require provider-supplied config metadata; unsupported integrations fail closed.
    value = sha(env.get("CANTOS_CONFIG_SHA"))
    repository = env.get("CANTOS_CONFIG_REPOSITORY", "").removesuffix(".git")
    require(repository == REPOSITORY and env.get("CANTOS_CONFIG_REF") == "refs/heads/develop",
            "reviewed same-repository develop configuration metadata required")
    return value


def check_request(request, env=os.environ):
    require(request["schema_version"] == 1 and request["repository"] == REPOSITORY, "unsupported release request")
    sha(request["source_sha"])
    sha(request["config_sha"])
    sha(request["initial_checkout_sha"])
    require(request["config_sha"] == config_identity(env) and
            request["initial_checkout_sha"] == env.get("CIRCLE_SHA1") and env.get("CIRCLE_BRANCH") == "develop",
            "CircleCI config revision/branch differs from release request")
    identity = ci_identity(env)
    require(all(request["producer"][k] == identity[k] for k in ("pipeline_id", "workflow_id", "pipeline_number")),
            "foreign pipeline/workflow release request")
    age = time.time() - request["created_at"]
    require(0 <= age <= MAX_AGE, "stale release request; create a new pipeline")
    api_origin(request["api_origin"])


def validate(manifest_path, expected_digest, component, source_sha, origin, *, request_path=None, local=False, _finalizing=False):
    require(manifest_path.name == (".pending-manifest.json" if _finalizing else "manifest.json"),
            "final manifest.json marker required")
    sha(source_sha)
    digest(expected_digest)
    manifest = read_json(manifest_path, expected_digest)
    require(manifest["schema_version"] == 2, "only manifest v2 is promotable; rebuild v1")
    require(manifest["repository"] == REPOSITORY and manifest["target"] == component and
            manifest["source_sha"] == source_sha, "wrong component/source/repository")
    require(manifest["api_origin"] == origin, "wrong API origin")
    require(manifest["configuration_sha256"] == fingerprint(manifest["configuration"]), "configuration digest mismatch")
    config = manifest["configuration"]
    require(config["component"] == component and config["api_origin"] == origin and config["profile"] == "release",
            "wrong release configuration")
    require(manifest["checkout_sha"] == source_sha and manifest["deployed"] is False,
            "invalid release source/state")
    age = time.time() - manifest["created_at"]
    require(0 <= age <= MAX_AGE, "stale artifact; rebuild for a new approval")
    if request_path:
        require(not local, "local and CircleCI provenance modes conflict")
        request = read_json(request_path)
        check_request(request)
        require(component in request["components"] and request["source_sha"] == source_sha and
                request["api_origin"] == (origin or request["api_origin"]) and
                manifest["request_sha256"] == fingerprint(request) and
                manifest["config_sha"] == request["config_sha"] and
                manifest["initial_checkout_sha"] == request["initial_checkout_sha"] and
                manifest["producer"] == request["producer"], "foreign-run artifact")
    else:
        require(local and not os.environ.get("CIRCLECI"), "explicit local artifact acknowledgement or CI request required")
    records = manifest["artifacts"]
    require(isinstance(records, list) and len(records) == len(EXPECTED[component]), "unexpected artifact count")
    names = [r["path"] for r in records]
    require(set(names) == EXPECTED[component] and len(names) == len(set(names)), "unexpected artifact files")
    require({p.name for p in manifest_path.parent.iterdir()} == set(names) | {manifest_path.name}, "unexpected release files")
    total = 0
    for record in records:
        require(file_record(manifest_path.parent / record["path"], manifest_path.parent) == record,
                "artifact bytes/hash mismatch")
        total += record["bytes"]
    require(total <= MAX_BYTES, "release exceeds size limit")
    if component.endswith("-web"):
        with tarfile.open(manifest_path.parent / names[0], "r:gz") as archive:
            archive_members(archive, web=True)
    if component == "api":
        image = read_json(manifest_path.parent / "image.json")
        require(image["source_sha"] == source_sha and image["architecture"] == "amd64" and
                image["os"] == "linux" and image["schema_epoch"] == 2 and
                re.fullmatch(r"sha256:[0-9a-f]{64}", image["id"]), "wrong image identity/platform/schema")
        with tarfile.open(manifest_path.parent / "runtime.tar") as archive:
            entries = archive_members(archive, runtime=True)
            revision = next(e for e in entries if e.name == "REVISION")
            require(archive.extractfile(revision).read(128).decode().strip() == source_sha, "runtime revision mismatch")
    return manifest
