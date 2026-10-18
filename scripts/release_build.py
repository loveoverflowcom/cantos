"""Implementation primitive for cargo xtask release-build; no cloud write or signing."""

import argparse
import hashlib
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


API_ORIGIN = "https://api.cantos.cc"
MOBILE = Path("apps/cantos-theatre/mobile")
TARGETS = ("api", "theatre-web", "studio-web", "android")


def run(root, argv, *, capture=False, env=None):
    # Never print inherited environment or credential-bearing commands.
    result = subprocess.run(argv, cwd=root, check=True, env=env,
                            text=True, stdout=subprocess.PIPE if capture else None)
    return result.stdout.strip() if capture else None


def source_revision(root):
    sha = run(root, ["git", "rev-parse", "HEAD"], capture=True)
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("full source SHA required")
    if run(root, ["git", "status", "--porcelain", "--untracked-files=normal"], capture=True):
        raise ValueError("release-build requires a clean tracked and untracked source tree")
    expected = os.environ.get("CIRCLE_SHA1")
    if expected and expected != sha:
        raise ValueError("CircleCI source SHA differs from checkout")
    return sha


def file_record(path, output):
    if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
        raise ValueError("artifact must be a nonempty regular file")
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return {"path": path.relative_to(output).as_posix(), "bytes": path.stat().st_size,
            "sha256": digest.hexdigest()}


def web_archive(dist, destination):
    if not (dist / "index.html").is_file():
        raise ValueError("Web build has no index.html")
    files = sorted(dist.rglob("*"))
    if any(path.is_symlink() for path in files):
        raise ValueError("Web bundle contains a symlink")
    with tarfile.open(destination, "w:gz") as archive:
        for path in files:
            if path.is_file():
                if path.suffix not in {".html", ".js", ".wasm", ".css", ".svg", ".png", ".ico", ".webp", ".woff", ".woff2", ".ttf", ".json", ".txt"}:
                    raise ValueError("unexpected Web asset type")
                archive.add(path, arcname=path.relative_to(dist), recursive=False)


def android_version(root):
    text = (root / MOBILE / "androidProductionApp/build.gradle.kts").read_text()
    code = re.search(r"^\s*versionCode = (\d+)\s*$", text, re.M)
    name = re.search(r'^\s*versionName = "([^"\n]+)"\s*$', text, re.M)
    if not code or not name:
        raise ValueError("cannot establish Android version from production host")
    return {"name": name[1], "code": int(code[1])}


def build(root, target, sha, output):
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
            (output / "image.json").write_text(json.dumps({"id": inspection["Id"],
                "architecture": "amd64", "os": "linux", "source_sha": sha}, indent=2) + "\n")
        return {"name": tomllib.loads((root / "services/cantos-api/Cargo.toml").read_text())["package"]["version"]}
    if target in ("theatre-web", "studio-web"):
        app = "cantos-" + target.removesuffix("-web")
        dist = root / "apps" / app / "web/dist"
        if dist.exists():
            raise ValueError("Web dist already exists; use a fresh checkout to exclude stale assets")
        env = dict(os.environ, CANTOS_API_ORIGIN=API_ORIGIN)
        if any(key.startswith("TRUNK_") for key in env):
            raise ValueError("ambient TRUNK overrides are refused")
        run(root, ["cargo", "xtask", "build", app, "--web", "--release"], env=env)
        web_archive(dist, output / f"{target}.tar.gz")
        return {"name": tomllib.loads((root / "apps" / app / "web/Cargo.toml").read_text())["package"]["version"]}
    run(root, ["cargo", "xtask", "theatre-host", "android-release", "--api-origin", API_ORIGIN])
    base = root / MOBILE / "androidProductionApp/build"
    for relative in ("outputs/apk/release/androidProductionApp-release-unsigned.apk",
                     "outputs/bundle/release/androidProductionApp-release.aab",
                     "reports/release-boundary/artifact-inspection.txt",
                     "reports/release-boundary/resolved-dependencies.txt"):
        source = base / relative
        if not source.is_file() or source.is_symlink():
            raise ValueError("missing or invalid production Android output")
        shutil.copyfile(source, output / source.name)
    return android_version(root)


def package(root, target, *, dry_run=False):
    # Planning is useful in a dirty development checkout; execution refuses it.
    if dry_run:
        print(f"NOT_RUN release-build {target}: clean HEAD -> artifacts/release-build/{target}/<SHA>")
        return
    sha = source_revision(root)
    output = root / "artifacts/release-build" / target / sha
    output.mkdir(parents=True, exist_ok=False)
    version = build(root, target, sha, output)
    if source_revision(root) != sha:
        raise ValueError("source changed during release build; no manifest accepted")
    artifacts = [file_record(path, output) for path in sorted(output.iterdir())]
    if not artifacts:
        raise ValueError("build produced no artifacts")
    manifest = {"schema_version": 1, "target": target, "source_sha": sha,
        "repository": "https://github.com/loveoverflowcom/cantos", "version": version,
        "rust": run(root, ["rustc", "--version"], capture=True),
        "host": {"system": platform.system(), "machine": platform.machine()},
        "api_origin": None if target == "api" else API_ORIGIN,
        "artifacts": artifacts, "deployed": False,
        "runtime_acceptance": "NOT_RUN"}
    if target == "android":
        manifest.update(application_id="com.cantos.theatre", signing="unsigned",
                        native_issuer=None, live_login_acceptance="NOT_RUN")
        manifest["gradle"] = run(root, ["gradle", "--version"], capture=True)
        manifest["android_build_tools"] = "35.0.0"
    if target.endswith("-web"):
        manifest["trunk"] = run(root, ["trunk", "--version"], capture=True)
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"PASS release artifacts: {output.relative_to(root)} (build only)")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("target", choices=TARGETS)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    package(Path(__file__).resolve().parents[1], args.target, dry_run=args.dry_run)


if __name__ == "__main__":
    main()
