"""Bind the same-composition unshrunk audit to optimized release bytes; no build/upload."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import subprocess


def sha(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
        raise ValueError("missing/irregular/empty release evidence: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def source_snapshot(root):
    def git(*args):
        return subprocess.run(["git", *args], cwd=root, check=True, capture_output=True).stdout
    names = git("ls-files", "-z", "--cached", "--others", "--exclude-standard").split(b"\0")
    records = []
    for name in sorted(set(names) - {b""}):
        path = root / name.decode()
        data = str(path.readlink()).encode() if path.is_symlink() else path.read_bytes() if path.is_file() else b"DELETED"
        records.append((name.decode(), hashlib.sha256(data).hexdigest()))
    return {"source_sha": git("rev-parse", "HEAD").decode().strip(),
            "content_sha256": hashlib.sha256(json.dumps(records).encode()).hexdigest()}


def bind(root, mobile):
    app = mobile / "androidApp"
    audit, optimized = app / "build-audit", app / "build"
    records = [json.loads((b / "reports/release-boundary/pre-shrink.json").read_text()) for b in (audit, optimized)]
    for record in records:
        if record["status"] != "PASS" or record["variant"] != "release" or not record["class_count"]:
            raise ValueError("pre-shrink input inspection did not pass")
    for field in ("source_sha256", "classes_sha256", "inputs_sha256", "class_count"):
        if records[0][field] != records[1][field]:
            raise ValueError("audit/optimized pre-shrink mismatch: " + field)
    graphs = [sha(b / "reports/release-boundary/resolved-dependencies.txt") for b in (audit, optimized)]
    if graphs[0] != graphs[1]:
        raise ValueError("audit/optimized resolved graph mismatch")
    def record(path):
        return {"path": str(path.relative_to(mobile)), "sha256": sha(path), "bytes": path.stat().st_size}
    artifacts = {}
    for mode, base in (("audit", audit), ("optimized", optimized)):
        metadata = json.loads((base / "outputs/apk/release/output-metadata.json").read_text())
        outputs = metadata.get("elements", [])
        if (metadata.get("applicationId") != "com.cantos.theatre" or metadata.get("variantName") != "release"
                or len(outputs) != 1 or outputs[0].get("type") != "SINGLE" or outputs[0].get("filters")):
            raise ValueError("ambiguous/foreign release output")
        name = outputs[0]["outputFile"]
        if Path(name).name != name or not name.endswith(".apk"):
            raise ValueError("unsafe APK output")
        artifacts[mode] = {"apk": record(base / "outputs/apk/release" / name),
                           "aab": record(base / "outputs/bundle/release/androidApp-release.aab"),
                           "inspection": record(base / "reports/release-boundary/artifact-inspection.txt"),
                           "distributable": False if mode == "audit" else "requires-signature-and-operator-policy"}
    for mode, base in (("audit", audit), ("optimized", optimized)):
        text = (base / "reports/release-boundary/artifact-inspection.txt").read_text()
        if f"variant=release audit={mode == 'audit'}" not in text or "FINDING " in text or "PASS: no sandbox" not in text:
            raise ValueError("inspection did not pass for the declared audit mode")
        for kind in ("apk", "aab"):
            expected = artifacts[mode][kind]["sha256"]
            if not re.search(r"^" + kind + r" .+ sha256=" + expected + r" ", text, re.M):
                raise ValueError("inspection report/artifact hash mismatch")
    artifacts["optimized"]["mapping"] = record(optimized / "outputs/mapping/release/mapping.txt")
    source = subprocess.run(["git", "rev-parse", "HEAD"], cwd=root, check=True, text=True, capture_output=True).stdout.strip()
    return {"schema_version": 1, "source_sha": source,
            "configuration": {"application_id": "com.cantos.theatre", "build_type": "release",
                "optimization": "R8 + resource shrinking", "audit_marker": "CantosReleaseAudit",
                "policy_source": source}, "variant": "release", "inputs": records[0],
            "resolved_graph_sha256": graphs[0], "artifacts": artifacts,
            "differences": ["optimization flags", "audit manifest marker", "app output directory", "audit unsigned"],
            "size": {"metric": "APK archive bytes", "audit": artifacts["audit"]["apk"]["bytes"],
                     "optimized": artifacts["optimized"]["apk"]["bytes"],
                     "estimated_download": "NOT_RUN", "installed_footprint": "NOT_RUN"},
            "device_acceptance": "NOT_RUN", "firebase_distribution": "NOT_RUN"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mobile", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--api-origin", default="")
    parser.add_argument("--native-issuer", default="")
    parser.add_argument("--begin", action="store_true")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[1]
    snapshot_path = args.report.with_name("source-inputs.json")
    snapshot = source_snapshot(root)
    if args.begin:
        snapshot_path.parent.mkdir(parents=True, exist_ok=True)
        snapshot_path.write_text(json.dumps(snapshot) + "\n")
        print("PASS source input snapshot (no credentials recorded)")
        return
    if json.loads(snapshot_path.read_text()) != snapshot:
        raise ValueError("source changed during audit/release build")
    result = bind(root, args.mobile.resolve())
    result["source"] = snapshot
    result["configuration"].update(api_origin=args.api_origin, native_issuer=args.native_issuer)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2) + "\n")
    print("PASS audit/release identical pre-shrink inputs and resolved graph; exact artifact/mapping hashes recorded")


if __name__ == "__main__":
    main()
