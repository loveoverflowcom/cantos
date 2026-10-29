"""Release asset inventory primitive, orchestrated only by cargo xtask web-performance.

Compression lengths are local estimates. URL shapes, MIME expectations and loader text do
not establish HTTP headers, immutable cache eligibility, browser cache hits or startup time.
"""

import argparse
from datetime import datetime, timezone
import gzip
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
APPS = {"cantos-studio": "apps/cantos-studio/web",
        "cantos-theatre": "apps/cantos-theatre/web"}
MIME = {".wasm": "application/wasm", ".js": "text/javascript",
        ".css": "text/css", ".html": "text/html", ".woff2": "font/woff2",
        ".avif": "image/avif", ".svg": "image/svg+xml", ".webp": "image/webp", ".png": "image/png",
        ".jpg": "image/jpeg", ".json": "application/json", ".txt": "text/plain"}
HASH_SHAPE = re.compile(r"-[0-9a-f]{16}(?:_bg)?\.[^.]+$")
CSS_URL = re.compile(r"url\(\s*['\"]?([^'\"\s)]+)")
IMPORT_URL = re.compile(r"\b(?:from|import)\s*['\"]([^'\"]+)['\"]")
WASM_URL = re.compile(r"\bmodule_or_path\s*:\s*['\"]([^'\"]+)['\"]")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def command(*args, required=True):
    try:
        result = subprocess.run(args, cwd=ROOT, capture_output=True, check=True)
        return result.stdout.decode("utf-8").strip()
    except (OSError, subprocess.CalledProcessError):
        if required:
            raise ValueError(f"required provenance command failed: {args[0]}") from None
        return None


def source_snapshot():
    paths = subprocess.run(["git", "ls-files", "-z", "--cached", "--others",
                            "--exclude-standard"], cwd=ROOT, check=True,
                           capture_output=True).stdout.split(b"\0")
    fingerprint = hashlib.sha256()
    count = 0
    for raw_path in sorted(set(path for path in paths if path)):
        path = ROOT / os.fsdecode(raw_path)
        if path.is_symlink():
            value = os.fsencode(os.readlink(path))
        elif path.is_file():
            value = path.read_bytes()
        elif not path.exists():
            value = b"<deleted>"
        else:
            raise ValueError("source snapshot contains an unsupported filesystem entry")
        fingerprint.update(raw_path + b"\0" + digest(value).encode() + b"\0")
        count += 1
    return {"revision": command("git", "rev-parse", "HEAD"),
            "tree": command("git", "rev-parse", "HEAD^{tree}"),
            "dirty": bool(command("git", "status", "--porcelain")),
            "input_files": count, "input_sha256": fingerprint.hexdigest()}


class ShellReferences(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.urls = []
        self.inline_scripts = []
        self.preloads = []
        self.in_script = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "base":
            raise ValueError("HTML base URL requires an explicit deployment mapping; unsupported")
        if tag == "link" and attrs.get("rel") in ("stylesheet", "preload", "modulepreload"):
            if "href" in attrs:
                self.urls.append(attrs["href"])
                if attrs["rel"] != "stylesheet":
                    self.preloads.append({key: attrs.get(key) for key in
                                          ("rel", "href", "as", "type", "integrity")})
        if tag == "script":
            self.in_script = True
            if "src" in attrs:
                self.urls.append(attrs["src"])
        if tag == "img" and "src" in attrs:
            self.urls.append(attrs["src"])

    def handle_endtag(self, tag):
        if tag == "script":
            self.in_script = False

    def handle_data(self, data):
        if self.in_script:
            self.inline_scripts.append(data)
            self.urls.extend(IMPORT_URL.findall(data))
            self.urls.extend(WASM_URL.findall(data))


def local_reference(url, referring, dist):
    parsed = urlsplit(url)
    if parsed.scheme or parsed.netloc or not parsed.path:
        return None
    decoded = unquote(parsed.path)
    relative = decoded.lstrip("/") if decoded.startswith("/") else str(referring.parent / decoded)
    path = (dist / relative).resolve()
    try:
        return path.relative_to(dist.resolve()).as_posix()
    except ValueError:
        raise ValueError(f"asset reference escapes dist: {url}") from None


def inventory(dist, brotli_compress=None):
    if not dist.is_dir() or dist.is_symlink():
        raise ValueError("dist must be a real directory from the successful current build")
    paths = sorted(dist.rglob("*"))
    if any(path.is_symlink() for path in paths):
        raise ValueError("dist symlinks are unsupported; refuse to inspect outside public build output")
    files = {path.relative_to(dist).as_posix(): path for path in paths if path.is_file()}
    from runtime_images import image_failure
    for name, path in files.items():
        failure = image_failure(name, path.read_bytes())
        if failure:
            raise ValueError(f"runtime image policy: {name}: {failure}")
    if "index.html" not in files:
        raise ValueError("dist/index.html is missing")
    shell = ShellReferences()
    shell.feed(files["index.html"].read_text(encoding="utf-8"))
    direct = {"index.html"}
    external = []
    for url in shell.urls:
        path = local_reference(url, Path("index.html"), dist)
        if path is None:
            external.append(url)
        else:
            direct.add(path)
    missing = sorted(direct - files.keys())
    if missing:
        raise ValueError(f"HTML references missing local assets (or unsupported public URL mapping): {missing}")
    if not any(Path(path).suffix == ".wasm" for path in direct):
        raise ValueError("HTML has no linked WASM bundle; unsupported Trunk loader shape")
    css_refs = set()
    for name in direct:
        if Path(name).suffix == ".css":
            for url in CSS_URL.findall(files[name].read_text(encoding="utf-8")):
                reference = local_reference(url, Path(name), dist)
                if reference is not None:
                    css_refs.add(reference)
    missing = sorted(css_refs - files.keys())
    if missing:
        raise ValueError(f"CSS references missing local assets: {missing}")
    rows = []
    loader = []
    for name, path in files.items():
        data = path.read_bytes()
        suffix = path.suffix.lower()
        role = "html-linked" if name in direct else "css-referenced" if name in css_refs else "other-output"
        rows.append({"path": name, "kind": suffix.lstrip(".") or "other", "role": role,
                     "expected_content_type": MIME.get(suffix, "application/octet-stream"),
                     "raw_bytes": len(data), "gzip_bytes": len(gzip.compress(data, compresslevel=9, mtime=0)),
                     "brotli_bytes": len(brotli_compress(data)) if brotli_compress else None,
                     "sha256": digest(data), "hash_shaped_filename": bool(HASH_SHAPE.search(name)),
                     "immutable_cache_eligibility": "NOT_VERIFIED"})
        if suffix == ".js" and name in direct:
            text = data.decode("utf-8")
            loader.append({"path": name,
                           "instantiate_streaming_call_present": "WebAssembly.instantiateStreaming(" in text,
                           "compile_streaming_call_present": "WebAssembly.compileStreaming(" in text})
    totals = {}
    for role in ("all-output", "html-linked", "css-referenced", "other-output"):
        selected = [row for row in rows if role == "all-output" or row["role"] == role]
        totals[role] = {key: sum(row[key] for row in selected) if key != "brotli_bytes" or brotli_compress else None
                        for key in ("raw_bytes", "gzip_bytes", "brotli_bytes")}
        totals[role]["files"] = len(selected)
    return {"assets": rows, "totals": totals, "external_html_references": sorted(set(external)),
            "preloads": shell.preloads, "loader_static_observation": loader,
            "trunk_started_event_in_shell": any("TrunkApplicationStarted" in text for text in shell.inline_scripts)}


def compare_assets(previous, current):
    if previous.get("schema_version") != 1 or previous.get("app") != current["app"]:
        raise ValueError("baseline schema/app does not match this inventory")
    before = {row["path"]: row["sha256"] for row in previous["assets"]}
    after = {row["path"]: row["sha256"] for row in current["assets"]}
    return {"added_urls": sorted(after.keys() - before.keys()),
            "removed_urls": sorted(before.keys() - after.keys()),
            "same_url_changed_bytes": sorted(path for path in before.keys() & after.keys() if before[path] != after[path]),
            "raw_byte_delta": current["totals"]["all-output"]["raw_bytes"] - previous["totals"]["all-output"]["raw_bytes"],
            "deployment_or_rollback_observed": False}


def brotli_adapter():
    """Use an installed compressor only; never install optional dependencies."""
    try:
        import brotli
        return (lambda data: brotli.compress(data, quality=11),
                {"status": "measured", "quality": 11, "adapter": "python",
                 "version": getattr(brotli, "__version__", "unknown")})
    except ImportError:
        executable = shutil.which("brotli")
        if executable:
            version = command(executable, "--version")

            def compress(data):
                return subprocess.run([executable, "--quality=11", "--stdout"],
                                      input=data, capture_output=True, check=True).stdout

            return compress, {"status": "measured", "quality": 11,
                              "adapter": "cli", "version": version}
    return None, {"status": "UNSUPPORTED", "quality": 11,
                  "adapter": None, "version": None}


def prepare(app, output):
    overrides = sorted(key for key in os.environ if key.startswith("TRUNK_"))
    if overrides:
        raise ValueError(f"remove inherited Trunk overrides for a reproducible baseline: {overrides}")
    try:
        output.resolve().relative_to((ROOT / "artifacts/web-performance").resolve())
    except ValueError:
        raise ValueError("output must be a fresh directory beneath artifacts/web-performance") from None
    # Exclusive creation prevents mixing runs or replacing earlier evidence.
    output.mkdir(parents=True, exist_ok=False)
    metadata = {"schema_version": 1, "app": app, "source_before_build": source_snapshot(),
                "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                "build": {"argv": ["trunk", "build", "--locked", "--release", "--config",
                                   str(ROOT / APPS[app] / "Trunk.toml"), "--dist",
                                   str(output.resolve() / "dist"), "index.html"],
                          "environment": {"NO_COLOR": "true"},
                          "cwd": APPS[app], "profile": "release",
                          "mount_to_body_in_entrypoint": "leptos::mount::mount_to_body" in (ROOT / APPS[app] / "src/main.rs").read_text(encoding="utf-8"),
                          "rustup_toolchain": os.environ.get("RUSTUP_TOOLCHAIN"),
                          "flags": {key: value for key, value in os.environ.items() if key in
                                    ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS") or key.startswith("CARGO_PROFILE_RELEASE_")}},
                "tools": {"rustc": command("rustc", "--version"),
                          "cargo": command("cargo", "--version"),
                          "trunk": command("trunk", "--version"),
                          "wasm_opt_on_path": command("wasm-opt", "--version", required=False),
                          "python": sys.version.split()[0]},
                "build_input_sha256": {name: digest((ROOT / name).read_bytes()) for name in
                                       ("Cargo.lock", "Cargo.toml", f"{APPS[app]}/Cargo.toml",
                                        f"{APPS[app]}/Trunk.toml", f"{APPS[app]}/index.html")}}
    (output / "provenance.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")


def measure(app, output, baseline):
    metadata = json.loads((output / "provenance.json").read_text(encoding="utf-8"))
    if metadata["app"] != app or metadata["source_before_build"] != source_snapshot():
        raise ValueError("source changed during the build or provenance belongs to another app; rerun in a fresh directory")
    brotli_compress, brotli_metadata = brotli_adapter()
    report = dict(metadata, **inventory(output / "dist", brotli_compress))
    report["compression"] = {"gzip": "Python gzip level=9 mtime=0",
                             "brotli": brotli_metadata,
                             "scope": "local whole-file estimates; HTTP compression NOT_RUN"}
    report["limitations"] = ["Hash-shaped names do not prove content-addressed immutable URLs.",
                             "HTML-linked totals are a static subset; actual initial requests and transfer bytes are NOT_RUN.",
                             "Loader call text is a static observation; streaming execution and MIME headers are NOT_RUN.",
                             "Cold/warm/hot timing, V8 code cache, deployed cache headers and rollback are NOT_RUN.",
                             "wasm-opt PATH version may differ from Trunk's downloaded helper; effective optimizer version is not established."]
    if baseline:
        report["baseline_difference"] = compare_assets(json.loads(baseline.read_text(encoding="utf-8")), report)
        report["baseline_sha256"] = digest(baseline.read_bytes())
    with (output / "inventory.json").open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    print(f"Inventory: {output / 'inventory.json'}")
    for role, values in report["totals"].items():
        print(f"  {role}: {values['files']} files; raw={values['raw_bytes']} gzip={values['gzip_bytes']} brotli={values['brotli_bytes']}")
    print("Static inventory only; browser timings, deployed headers and cache hits NOT_RUN")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("prepare", "inventory"))
    parser.add_argument("--app", choices=APPS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    try:
        if args.phase == "prepare":
            prepare(args.app, args.output)
        else:
            measure(args.app, args.output, args.baseline)
    except (OSError, ValueError, KeyError, UnicodeError, subprocess.CalledProcessError) as error:
        print(f"web-performance: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
