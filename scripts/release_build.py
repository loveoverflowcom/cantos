"""Implementation primitive for cargo xtask release-build; no cloud write or signing."""

import argparse
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import time
import uuid

import release_artifact as contract


API_ORIGIN = "https://api.cantos.cc"
MOBILE = Path("apps/cantos-theatre/mobile")
TARGETS = ("api", "theatre-web", "studio-web", "android")


def run(root, argv, *, capture=False, env=None):
    # Never print inherited environment or credential-bearing commands.
    result = subprocess.run(argv, cwd=root, check=True, env=env,
                            text=True, stdout=subprocess.PIPE if capture else None)
    return result.stdout.strip() if capture else None


def source_revision(root, request=None):
    sha = run(root, ["git", "rev-parse", "HEAD"], capture=True)
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("full source SHA required")
    if run(root, ["git", "status", "--porcelain", "--untracked-files=normal"], capture=True):
        raise ValueError("release-build requires a clean tracked and untracked source tree")
    expected = os.environ.get("CIRCLE_SHA1")
    if request:
        contract.check_request(request)
        if sha != request["source_sha"]:
            raise ValueError("release checkout differs from resolved source")
        run(root, ["git", "cat-file", "-e", request["config_sha"] + "^{commit}"])
        run(root, ["git", "merge-base", "--is-ancestor", sha, "refs/remotes/origin/develop"])
    elif expected and expected != sha:
        raise ValueError("CircleCI source SHA differs from checkout")
    return sha


def file_record(path, output):
    return contract.file_record(path, output)


def web_archive(dist, destination):
    if not (dist / "index.html").is_file():
        raise ValueError("Web build has no index.html")
    files = sorted(dist.rglob("*"))
    if len(files) > contract.MAX_ENTRIES:
        raise ValueError("Web bundle exceeds entry limit")
    if any(path.is_symlink() or not (path.is_file() or path.is_dir()) for path in files):
        raise ValueError("Web bundle contains an irregular file or symlink")
    with tarfile.open(destination, "w:gz") as archive:
        for path in files:
            if path.is_file():
                contract.regular(path)
                if path.suffix not in contract.WEB_SUFFIXES:
                    raise ValueError("unexpected Web asset type")
                archive.add(path, arcname=path.relative_to(dist), recursive=False)
    with tarfile.open(destination, "r:gz") as archive:
        contract.archive_members(archive, web=True)


def android_version(root):
    text = (root / MOBILE / "androidApp/build.gradle.kts").read_text()
    code = re.search(r"^\s*versionCode = (\d+)\s*$", text, re.M)
    name = re.search(r'^\s*versionName = "([^"\n]+)"\s*$', text, re.M)
    if not code or not name:
        raise ValueError("cannot establish Android version from the Android application")
    return {"name": name[1], "code": int(code[1])}


def runtime_archive(context, destination):
    with tarfile.open(destination, "w") as archive:
        paths = [context / "REVISION", context / "infra/edge/Caddyfile",
                 context / "services/cantos-server/schema/legacy-epoch-1-manifest.json"]
        paths += sorted((context / "infra/vps").rglob("*"))
        for path in paths:
            if path.is_dir():
                continue
            contract.regular(path)
            archive.add(path, arcname=path.relative_to(context), recursive=False)
    with tarfile.open(destination) as archive:
        contract.archive_members(archive, runtime=True)


def build(root, target, sha, output, origin=API_ORIGIN):
    if target == "api":
        if platform.system() != "Linux" or platform.machine() != "x86_64":
            raise ValueError("VPS image build requires Linux x86_64; no emulation or Mac Podman setup")
        with tempfile.TemporaryDirectory(prefix="cantos-release-") as temporary:
            work = Path(temporary)
            context = work / "context"
            context.mkdir()
            run(root, ["git", "archive", "--format=tar", "-o", str(work / "source.tar"), sha])
            run(root, ["tar", "-xf", str(work / "source.tar"), "-C", str(context)])
            image = f"localhost/cantos-api:{sha}"
            run(root, ["podman", "build", "--platform=linux/amd64", "--pull=missing",
                       "-f", str(context / "infra/vps/image/Containerfile"),
                       "--build-arg", f"CANTOS_REVISION={sha}", "-t", image, str(context)])
            inspection = json.loads(run(root, ["podman", "image", "inspect", image], capture=True))[0]
            if (inspection["Architecture"] != "amd64" or inspection["Os"] != "linux"
                    or inspection["Labels"]["org.opencontainers.image.revision"] != sha):
                raise ValueError("built image identity does not match requested source/platform")
            run(root, ["podman", "save", "--format=docker-archive", "-o", str(work / "cantos-api.tar"), image])
            run(root, ["zstd", "-q", "-T2", str(work / "cantos-api.tar"), "-o", str(output / "cantos-api.tar.zst")])
            # Inspect output has no runtime env, mounts, credentials or account details.
            if inspection["Labels"].get("io.cantos.schema-epoch") != "2":
                raise ValueError("built image schema epoch mismatch")
            (output / "image.json").write_text(json.dumps({"id": contract.image_id(inspection["Id"]),
                "architecture": "amd64", "os": "linux", "source_sha": sha,
                "schema_epoch": 2}, indent=2) + "\n")
            (context / "REVISION").write_text(sha + "\n")
            runtime_archive(context, output / "runtime.tar")
        return {"name": tomllib.loads((root / "services/cantos-server/Cargo.toml").read_text())["package"]["version"]}
    if target in ("theatre-web", "studio-web"):
        app = "cantos-" + target.removesuffix("-web")
        dist = root / "apps" / app / "web/dist"
        if dist.exists():
            raise ValueError("Web dist already exists; use a fresh checkout to exclude stale assets")
        env = dict(os.environ, CANTOS_API_ORIGIN=origin)
        if any(key.startswith("TRUNK_") for key in env):
            raise ValueError("ambient TRUNK overrides are refused")
        run(root, ["cargo", "xtask", "build", app, "--web", "--release"], env=env)
        web_archive(dist, output / f"{target}.tar.gz")
        return {"name": tomllib.loads((root / "apps" / app / "web/Cargo.toml").read_text())["package"]["version"]}
    run(root, ["cargo", "xtask", "theatre-host", "android-release", "--api-origin", origin])
    base = root / MOBILE / "androidApp/build"
    evidence = json.loads((base / "reports/release-boundary/release-evidence.json").read_text())
    if (evidence["configuration"]["api_origin"] != origin or
            evidence["configuration"].get("native_issuer", "") != "" or
            Path(evidence["artifacts"]["optimized"]["apk"]["path"]).name != "androidApp-release-unsigned.apk"):
        raise ValueError("Android snapshot configuration differs from the requested unsigned package")
    snapshot = Path(contract.safe_name(evidence["snapshot_directory"]))
    if snapshot.parts[:2] != ("artifacts", "android-release") or len(snapshot.parts) != 3:
        raise ValueError("invalid Android snapshot directory")
    snapshot = root / snapshot
    if contract.read_json(snapshot / "release-evidence.json") != evidence:
        raise ValueError("Android snapshot record differs from the completed build record")
    contract.validate_android_outputs(snapshot, sha)
    for name in contract.EXPECTED["android"]:
        source = snapshot / name
        contract.regular(source)
        shutil.copyfile(source, output / name)
    contract.validate_android_outputs(output, sha)
    return android_version(root)


def package(root, target, *, dry_run=False, origin=None, request=None, base=None):
    # Planning is useful in a dirty development checkout; execution refuses it.
    if dry_run:
        print(f"NOT_RUN release-build {target}: clean HEAD -> artifacts/release-build/{target}/<SHA>/<CONFIG>/<ATTEMPT>")
        return
    if target.endswith("-web"):
        origin = contract.api_origin(os.environ.get("CANTOS_API_ORIGIN") if origin is None else origin)
        overrides = [key for key in os.environ if key.startswith(("TRUNK_", "CARGO_PROFILE_"))
                     or key in {"RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_TARGET"}]
        if overrides:
            raise ValueError("ambient Web build configuration overrides are refused")
    else:
        origin = contract.api_origin(API_ORIGIN if origin is None else origin) if target == "android" else None
    sha = source_revision(root, request)
    configuration = contract.configuration(root, target, origin)
    config_digest = contract.fingerprint(configuration)
    output = (base or root / "artifacts/release-build") / target / sha / config_digest / uuid.uuid4().hex
    output.mkdir(parents=True, exist_ok=False)
    version = build(root, target, sha, output, origin)
    if source_revision(root, request) != sha:
        raise ValueError("source changed during release build; no manifest accepted")
    artifacts = [file_record(path, output) for path in sorted(output.iterdir())]
    if not artifacts:
        raise ValueError("build produced no artifacts")
    manifest = {"schema_version": 2, "target": target, "source_sha": sha,
        "repository": "https://github.com/loveoverflowcom/cantos", "version": version,
        "rust": run(root, ["rustc", "--version"], capture=True),
        "host": {"system": platform.system(), "machine": platform.machine()},
        "api_origin": origin, "configuration": configuration,
        "configuration_sha256": config_digest, "checkout_sha": sha,
        "config_sha": request["config_sha"] if request else sha,
        "initial_checkout_sha": request["initial_checkout_sha"] if request else sha,
        "producer": request["producer"] if request else None,
        "request_sha256": contract.fingerprint(request) if request else None,
        "created_at": time.time(),
        "artifacts": artifacts, "deployed": False,
        "runtime_acceptance": "NOT_RUN"}
    if manifest["rust"].split()[1] != "1.98.1":
        raise ValueError("release requires the repository Rust 1.98.1 pin")
    if target == "android":
        manifest.update(application_id="com.cantos.theatre", signing="unsigned",
                        native_issuer=None, live_login_acceptance="NOT_RUN")
        manifest["gradle"] = run(root, ["gradle", "--version"], capture=True)
        manifest["android_build_tools"] = "35.0.0"
    if target.endswith("-web"):
        manifest["trunk"] = run(root, ["trunk", "--version"], capture=True)
        if manifest["trunk"] != "trunk 0.21.14":
            raise ValueError("release requires Trunk 0.21.14")
    manifest_path = output / ".pending-manifest.json"
    contract.write_new(manifest_path, manifest)
    # The final marker must never accept malformed, missing or extra build outputs.
    try:
        if request:
            validate_outputs(manifest_path, request)
        else:
            contract.validate(manifest_path, file_record(manifest_path, output)["sha256"], target,
                              sha, origin, request_path=None, local=True, _finalizing=True)
    except Exception:
        manifest_path.unlink()
        raise
    if source_revision(root, request) != sha:
        manifest_path.unlink()
        raise ValueError("source changed before manifest finalization")
    manifest_path = manifest_path.rename(output / "manifest.json")
    print(f"PASS release artifacts: {output.relative_to(root)} (build only)")
    return manifest_path


def validate_outputs(path, request):
    # Use the actual immutable request rather than trusting manifest metadata alone.
    with tempfile.TemporaryDirectory(prefix="cantos-request-") as temporary:
        request_path = Path(temporary) / "request.json"
        contract.write_new(request_path, request)
        manifest = contract.read_json(path)
        return contract.validate(path, file_record(path, path.parent)["sha256"], manifest["target"],
                                 request["source_sha"], manifest["api_origin"], request_path=request_path, _finalizing=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=TARGETS)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--api-origin")
    args = parser.parse_args()
    if args.api_origin is not None and not args.target.endswith("-web"):
        parser.error("--api-origin is a Web build input; Android retains its existing production origin")
    package(Path(__file__).resolve().parents[1], args.target, dry_run=args.dry_run, origin=args.api_origin)


if __name__ == "__main__":
    main()
