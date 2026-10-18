"""Local APK size report primitive, orchestrated only by cargo xtask theatre-size.

Reads one APK and writes a versioned JSON report plus a text summary beneath the ignored
artifacts/apk-size tree. It never uploads, signs, installs or logs in. APK file bytes, the
Analyzer's estimated download size and installed footprint are separate metrics: installed
footprint is NOT_MEASURED (it needs a named device), and DEX package attribution is an
uncompressed estimate, never a compressed download saving.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import zipfile


ROOT = Path(__file__).resolve().parents[1]
OUTPUT_ROOT = ROOT / "artifacts/apk-size"
SCHEMA_VERSION = 1
FONT_SUFFIXES = (".ttf", ".otf", ".ttc", ".woff", ".woff2")
GROUPS = ("dex", "native_libs", "resources_arsc", "res", "fonts", "assets", "manifest",
          "signature_and_meta_inf", "other")
# Prefixes the issue asks to prove or reject with bytes. Package rows are inclusive of their
# children and an uncompressed DEX estimate; they never equal compressed download savings.
NOTABLE_DEX_PACKAGES = ("androidx.compose.material.icons", "androidx.compose.ui.tooling",
                        "org.jetbrains.compose", "kotlin", "kotlinx")
TOP_ENTRIES = 15
TOP_DEX_PACKAGES = 20
MAX_DEX_PACKAGE_DEPTH = 4
TOOL_TIMEOUT_SECONDS = 600
SIGNER_DIGEST = re.compile(r"Signer #(\d+) certificate SHA-256 digest:\s*([0-9a-fA-F]{64})")


class ToolFailure(ValueError):
    """An external analyzer was missing, failed or printed an unparseable result."""


# --- pure core -------------------------------------------------------------------------


def classify(name):
    """Map one ZIP entry name to a (group, detail) pair; detail is the ABI for native libs."""
    lowered = name.lower()
    if "/" not in name and re.fullmatch(r"classes\d*\.dex", lowered):
        return "dex", None
    if name.startswith("lib/"):
        parts = name.split("/")
        return "native_libs", parts[1] if len(parts) > 2 else "unknown"
    if name == "resources.arsc":
        return "resources_arsc", None
    if name == "AndroidManifest.xml":
        return "manifest", None
    if name.startswith("META-INF/"):
        return "signature_and_meta_inf", None
    if lowered.endswith(FONT_SUFFIXES):
        return "fonts", None
    if name.startswith("res/"):
        return "res", None
    if name.startswith("assets/"):
        return "assets", None
    return "other", None


def summarize_entries(entries, apk_file_bytes):
    """Group ZIP metadata. Overhead is file bytes minus entry compressed bytes: it holds local
    headers, the central directory, signing blocks and alignment padding, and is never added
    to the entry totals."""
    groups = {name: {"files": 0, "compressed_bytes": 0, "uncompressed_bytes": 0} for name in GROUPS}
    abis = {}
    for entry in entries:
        group, detail = classify(entry["name"])
        targets = [groups[group]]
        if group == "native_libs":
            targets.append(abis.setdefault(detail, {"files": 0, "compressed_bytes": 0, "uncompressed_bytes": 0}))
        for target in targets:
            target["files"] += 1
            target["compressed_bytes"] += entry["compressed_bytes"]
            target["uncompressed_bytes"] += entry["uncompressed_bytes"]
    compressed = sum(entry["compressed_bytes"] for entry in entries)
    ranked = sorted(entries, key=lambda entry: (-entry["compressed_bytes"], entry["name"]))
    return {"groups": groups, "native_libs_by_abi": dict(sorted(abis.items())),
            "entry_count": len(entries),
            "entry_compressed_bytes": compressed,
            "entry_uncompressed_bytes": sum(entry["uncompressed_bytes"] for entry in entries),
            "container_overhead_bytes": apk_file_bytes - compressed,
            "top_entries": [dict(entry, group=classify(entry["name"])[0]) for entry in ranked[:TOP_ENTRIES]]}


def parse_integer(text, label):
    stripped = text.strip()
    if not re.fullmatch(r"\d+", stripped):
        raise ToolFailure(f"{label} did not print one non-negative integer")
    return int(stripped)


def parse_boolean(text, label):
    stripped = text.strip().lower()
    if stripped not in ("true", "false"):
        raise ToolFailure(f"{label} did not print true or false")
    return stripped == "true"


def parse_summary(text):
    fields = text.strip().split("\t")
    if len(fields) != 3 or not fields[0] or not fields[1].isdigit():
        raise ToolFailure("apkanalyzer apk summary did not print application id, version code and version name")
    return {"application_id": fields[0], "version_code": int(fields[1]), "version_name": fields[2]}


def parse_dex_packages(text):
    """Parse `apkanalyzer dex packages --defined-only` P rows into inclusive package sizes."""
    packages = {}
    for line in text.splitlines():
        if not line.startswith("P "):
            continue
        fields = line.split("\t")
        head = fields[0].split()
        if len(fields) != 4 or len(head) != 3 or not head[2].isdigit() \
                or not fields[1].isdigit() or not fields[2].isdigit() or not fields[3]:
            raise ToolFailure(f"unparseable DEX package row: {line[:80]!r}")
        packages[fields[3]] = {"defined_methods": int(head[2]), "referenced_methods": int(fields[1]),
                               "estimated_dex_bytes": int(fields[2])}
    if "<TOTAL>" not in packages:
        raise ToolFailure("apkanalyzer dex packages printed no <TOTAL> row")
    total = packages.pop("<TOTAL>")
    named = {name: row for name, row in packages.items() if name.count(".") < MAX_DEX_PACKAGE_DEPTH}
    ranked = sorted(named.items(), key=lambda item: (-item[1]["estimated_dex_bytes"], item[0]))
    notable = {}
    for prefix in NOTABLE_DEX_PACKAGES:
        row = packages.get(prefix)
        notable[prefix] = None if row is None else dict(
            row, share_of_dex_estimate=round(row["estimated_dex_bytes"] / total["estimated_dex_bytes"], 4)
            if total["estimated_dex_bytes"] else None)
    return {"total": total, "notable": notable,
            "top_packages_inclusive": [dict(row, package=name) for name, row in ranked[:TOP_DEX_PACKAGES]],
            "scope": "defined classes; uncompressed DEX estimate; package rows include their children; "
                     "not compressed download savings"}


def parse_signer_digests(text):
    return sorted({digest.lower() for _, digest in SIGNER_DIGEST.findall(text)})


def size_units(byte_count):
    return {"bytes": byte_count, "decimal_mb": round(byte_count / 1_000_000, 3),
            "mib": round(byte_count / 1_048_576, 3)}


def build_report(identity, zip_summary, analyzer, signing, provenance, captured_at):
    """Assemble the versioned report. `analyzer` and `signing` carry their own status so a
    missing tool is visible instead of silently producing a smaller report."""
    partial = analyzer["status"] != "measured"
    return {"schema_version": SCHEMA_VERSION, "captured_at_utc": captured_at,
            "status": "PARTIAL" if partial else "MEASURED",
            "apk": {"sha256": identity["sha256"], "file": size_units(identity["file_bytes"])},
            "provenance": provenance,
            "identity": analyzer.get("identity"),
            "debuggable": analyzer.get("debuggable"),
            "signing": signing,
            "metrics": {
                "apk_file_bytes": identity["file_bytes"],
                "analyzer_file_size_bytes": analyzer.get("file_size_bytes"),
                "analyzer_estimated_download_bytes": analyzer.get("download_size_bytes"),
                "installed_footprint": "NOT_MEASURED (needs a named device and data/cache conditions)"},
            "zip": zip_summary,
            "dex": analyzer.get("dex"),
            "tools": analyzer["tools"],
            "analyzer_status": analyzer["status"],
            "analyzer_problems": analyzer.get("problems", []),
            "limitations": [
                "APK file bytes, estimated download size and installed footprint are different metrics.",
                "Container overhead is file bytes minus entry compressed bytes and is not added to any group.",
                "DEX package bytes are an uncompressed estimate of defined classes, not download savings.",
                "Resolved Gradle dependency graph is not read here; inspect it with the owning Gradle build."]}


def _delta(before, after):
    if not isinstance(before, int) or not isinstance(after, int):
        return None
    percent = None if before == 0 else round((after - before) * 100 / before, 2)
    return {"before": before, "after": after, "delta_bytes": after - before, "delta_percent": percent}


def compare_reports(before, after):
    """Absolute and percent deltas per metric plus every input difference. Different kinds of
    metric are never compared with each other; a side that lacks a metric yields null."""
    for label, report in (("baseline", before), ("current", after)):
        if report.get("schema_version") != SCHEMA_VERSION:
            raise ValueError(f"{label} report has an unsupported schema_version")
    differences = {}
    pairs = {
        "application_id": lambda r: (r.get("identity") or {}).get("application_id"),
        "version_code": lambda r: (r.get("identity") or {}).get("version_code"),
        "version_name": lambda r: (r.get("identity") or {}).get("version_name"),
        "debuggable": lambda r: r.get("debuggable"),
        "variant": lambda r: r["provenance"].get("variant"),
        "gradle_task": lambda r: r["provenance"].get("gradle_task"),
        "source_revision": lambda r: r["provenance"].get("source_revision"),
        "signer_sha256": lambda r: (r.get("signing") or {}).get("signer_sha256"),
        "analyzer_version": lambda r: r["tools"].get("apkanalyzer"),
    }
    for key, get in pairs.items():
        left, right = get(before), get(after)
        if left != right:
            differences[key] = {"before": left, "after": right}
    group_deltas = {name: _delta(before["zip"]["groups"][name]["compressed_bytes"],
                                 after["zip"]["groups"][name]["compressed_bytes"]) for name in GROUPS}
    before_dex, after_dex = before.get("dex"), after.get("dex")
    icons = None
    if before_dex and after_dex:
        key = "androidx.compose.material.icons"
        icons = _delta(((before_dex["notable"].get(key)) or {}).get("estimated_dex_bytes"),
                       ((after_dex["notable"].get(key)) or {}).get("estimated_dex_bytes"))
    return {"input_differences": differences,
            "metrics": {"apk_file_bytes": _delta(before["metrics"]["apk_file_bytes"], after["metrics"]["apk_file_bytes"]),
                        "analyzer_estimated_download_bytes": _delta(
                            before["metrics"]["analyzer_estimated_download_bytes"],
                            after["metrics"]["analyzer_estimated_download_bytes"])},
            "group_compressed_bytes": group_deltas,
            "material_icons_dex_estimate_bytes": icons,
            "comparable": not differences,
            "note": "differing inputs mean the delta mixes effects; see input_differences"}


def render_summary(report, comparison=None):
    lines = [f"APK size report ({report['status']}) sha256={report['apk']['sha256']}"]
    file = report["apk"]["file"]
    lines.append(f"  file: {file['bytes']} bytes = {file['decimal_mb']} MB = {file['mib']} MiB")
    identity = report["identity"]
    if identity:
        lines.append(f"  package {identity['application_id']} {identity['version_name']} ({identity['version_code']}); "
                     f"debuggable={report['debuggable']}")
    download = report["metrics"]["analyzer_estimated_download_bytes"]
    lines.append(f"  estimated download: {download if download is not None else 'NOT_MEASURED'}; installed footprint: NOT_MEASURED")
    lines.append(f"  provenance: variant={report['provenance']['variant']} source={report['provenance']['source_revision']}")
    lines.append("  group                    files   compressed  uncompressed")
    for name, row in report["zip"]["groups"].items():
        lines.append(f"  {name:<24}{row['files']:>6}{row['compressed_bytes']:>13}{row['uncompressed_bytes']:>14}")
    lines.append(f"  container overhead (file - entry compressed): {report['zip']['container_overhead_bytes']}")
    if report["dex"]:
        for prefix, row in report["dex"]["notable"].items():
            shown = "absent" if row is None else f"{row['estimated_dex_bytes']} est. DEX bytes"
            lines.append(f"  dex {prefix}: {shown}")
    for problem in report["analyzer_problems"]:
        lines.append(f"  PROBLEM: {problem}")
    if comparison:
        lines.append("  baseline comparison:")
        for name, row in comparison["metrics"].items():
            lines.append(f"    {name}: " + ("NOT_COMPARABLE" if row is None else
                         f"{row['delta_bytes']:+d} bytes ({row['delta_percent']}%)"))
        for name, row in comparison["group_compressed_bytes"].items():
            if row and row["delta_bytes"]:
                lines.append(f"    {name}: {row['delta_bytes']:+d} compressed bytes")
        for name, row in comparison["input_differences"].items():
            lines.append(f"    INPUT DIFFERS {name}: {row['before']!r} -> {row['after']!r}")
    return "\n".join(lines) + "\n"


# --- filesystem and process adapters ----------------------------------------------------


def file_identity(apk):
    if apk.is_symlink() or not apk.is_file():
        raise ValueError(f"APK is missing or not a regular file: {apk}")
    size = apk.stat().st_size
    if size == 0:
        raise ValueError(f"APK is empty: {apk}")
    digest = hashlib.sha256()
    with apk.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            digest.update(chunk)
    return {"sha256": digest.hexdigest(), "file_bytes": size}


def read_entries(apk):
    try:
        with zipfile.ZipFile(apk) as archive:
            bad = archive.testzip()
            if bad is not None:
                raise ValueError(f"APK entry fails its CRC check: {bad}")
            entries = [{"name": info.filename, "compressed_bytes": info.compress_size,
                        "uncompressed_bytes": info.file_size}
                       for info in archive.infolist() if not info.is_dir()]
    except zipfile.BadZipFile as error:
        raise ValueError(f"APK is not a readable ZIP archive: {error}") from None
    if not entries:
        raise ValueError("APK contains no entries")
    return entries


def find_android_tool(name, relative):
    found = shutil.which(name)
    if found:
        return found
    for variable in ("ANDROID_HOME", "ANDROID_SDK_ROOT"):
        root = os.environ.get(variable)
        if root:
            for candidate in sorted(Path(root).glob(relative), reverse=True):
                if candidate.is_file() and os.access(candidate, os.X_OK):
                    return str(candidate)
    return None


def run_tool(argv):
    try:
        result = subprocess.run(argv, capture_output=True, timeout=TOOL_TIMEOUT_SECONDS, check=False)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise ToolFailure(f"{Path(argv[0]).name} could not run: {error}") from None
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", "replace").strip().splitlines()[-1:] or ["no stderr"]
        raise ToolFailure(f"{Path(argv[0]).name} {' '.join(argv[1:3])} exited {result.returncode}: {detail[0][:160]}")
    return result.stdout.decode("utf-8", "replace")


def run_analyzer(apk):
    executable = find_android_tool("apkanalyzer", "cmdline-tools/*/bin/apkanalyzer")
    if executable is None:
        return {"status": "UNSUPPORTED", "tools": {"apkanalyzer": None},
                "problems": ["apkanalyzer is not on PATH or beneath ANDROID_HOME/ANDROID_SDK_ROOT cmdline-tools"]}
    analyzer = {"status": "measured", "tools": {"apkanalyzer": executable}, "problems": []}
    steps = (
        ("identity", lambda: parse_summary(run_tool([executable, "apk", "summary", str(apk)]))),
        ("file_size_bytes", lambda: parse_integer(run_tool([executable, "apk", "file-size", str(apk)]), "apk file-size")),
        ("download_size_bytes", lambda: parse_integer(run_tool([executable, "apk", "download-size", str(apk)]), "apk download-size")),
        ("debuggable", lambda: parse_boolean(run_tool([executable, "manifest", "debuggable", str(apk)]), "manifest debuggable")),
        ("dex", lambda: parse_dex_packages(run_tool([executable, "dex", "packages", "--defined-only", str(apk)]))),
    )
    for key, step in steps:
        try:
            analyzer[key] = step()
        except ToolFailure as error:
            analyzer["problems"].append(f"{key}: {error}")
    if analyzer["problems"]:
        analyzer["status"] = "FAILED"
    return analyzer


def read_signing(apk):
    executable = find_android_tool("apksigner", "build-tools/*/apksigner")
    if executable is None:
        return {"status": "NOT_CHECKED", "signer_sha256": None, "reason": "apksigner not found"}
    try:
        digests = parse_signer_digests(run_tool([executable, "verify", "--print-certs", str(apk)]))
    except ToolFailure as error:
        return {"status": "NOT_VERIFIED", "signer_sha256": None, "reason": str(error)}
    if not digests:
        return {"status": "NOT_VERIFIED", "signer_sha256": None, "reason": "apksigner printed no certificate digest"}
    return {"status": "verified", "signer_sha256": digests}


def gradle_provenance(apk, source_revision):
    """Variant and version only from the AGP metadata beside the APK naming exactly this file;
    anything else is `unknown`, never inferred from the file name or the checked-out branch."""
    provenance = {"variant": "unknown", "gradle_task": "unknown",
                  "source_revision": source_revision or "unknown", "source_dirty": "unknown",
                  "origin": "explicit APK path"}
    metadata = apk.parent / "output-metadata.json"
    try:
        data = json.loads(metadata.read_text(encoding="utf-8"))
        elements = [element for element in data.get("elements", []) if element.get("outputFile") == apk.name]
        if len(elements) == 1 and isinstance(data.get("variantName"), str):
            provenance["variant"] = data["variantName"]
            provenance["origin"] = "Gradle output-metadata.json beside the APK"
    except (OSError, ValueError, AttributeError):
        pass
    return provenance


def fresh_output(path):
    resolved = (path if path.is_absolute() else ROOT / path).resolve()
    try:
        resolved.relative_to(OUTPUT_ROOT.resolve())
    except ValueError:
        raise ValueError("output must be a fresh directory beneath artifacts/apk-size") from None
    return resolved


def load_baseline(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise ValueError(f"baseline report is unreadable: {error}") from None


def run(apk, output, baseline, source_revision):
    out = fresh_output(output)
    if out.exists():
        raise ValueError(f"output already exists; evidence is never overwritten: {out}")
    identity = file_identity(apk)
    entries = read_entries(apk)
    baseline_report = load_baseline(baseline) if baseline else None
    analyzer = run_analyzer(apk)
    report = build_report(identity, summarize_entries(entries, identity["file_bytes"]), analyzer,
                          read_signing(apk), gradle_provenance(apk, source_revision),
                          datetime.now(timezone.utc).isoformat())
    comparison = None
    if baseline_report is not None:
        comparison = compare_reports(baseline_report, report)
        report["baseline"] = {"sha256": file_identity(baseline)["sha256"], "comparison": comparison}
    out.mkdir(parents=True, exist_ok=False)
    with (out / "report.json").open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    summary = render_summary(report, comparison)
    (out / "summary.txt").write_text(summary, encoding="utf-8")
    print(f"Report: {out / 'report.json'}")
    sys.stdout.write(summary)
    return 0 if report["status"] == "MEASURED" else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apk", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, help="earlier report.json for deltas")
    parser.add_argument("--source-revision", help="known source commit of this APK; default unknown")
    args = parser.parse_args()
    try:
        return run(args.apk, args.output, args.baseline, args.source_revision)
    except (OSError, ValueError, KeyError, UnicodeError) as error:
        print(f"theatre-size: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
