"""Read-only AGP scoped-class/source/resource observer, invoked by theatre-host's Gradle tasks."""
import argparse
import hashlib
import json
from pathlib import Path
import zipfile

from inspect_android_release import (InspectionError, class_findings, resource_hashes,
                                     source_inventory, archive_findings, SANDBOX_TOKEN)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def inspect(mobile, variant, inputs):
    if variant not in {"debug", "release"}:
        raise InspectionError("only debug/release inputs are supported")
    forbidden = source_inventory(mobile / "preview-sandbox/src")
    forbidden_hashes = resource_hashes(mobile / "preview-sandbox/src/commonMain/composeResources")
    # Include source fixtures: a verbatim copied Kotlin fixture must fail even under a new path.
    for path in (mobile / "preview-sandbox/src").rglob("*"):
        if path.is_file():
            forbidden_hashes[digest(path.read_bytes())] = str(path.relative_to(mobile))
    fixture_roots = []
    for module in ("shared", "androidApp"):
        fixture_roots += list((mobile / module / "src").glob("*Test"))
    fixture_roots.append(mobile / "../../../packages/contracts/fixtures/theatre")
    for root in fixture_roots:
        if root.is_dir():
            for path in root.rglob("*"):
                if path.is_file():
                    forbidden_hashes[digest(path.read_bytes())] = "test fixture: " + path.name
    classes, findings, source = set(), [], []
    class_records, artifact_records = set(), set()
    for path in inputs:
        if path.is_dir():
            entries = [(p.relative_to(path).as_posix(), p.read_bytes()) for p in path.rglob("*.class")]
        elif path.is_file() and zipfile.is_zipfile(path):
            with zipfile.ZipFile(path) as archive:
                entries = [(n, archive.read(n)) for n in archive.namelist() if n.endswith(".class")]
                resources = {n: digest(archive.read(n)) for n in archive.namelist()
                             if not n.endswith(("/", ".class"))}
                findings += archive_findings(resources, forbidden_hashes, "com.cantos.theatre.resources")
                artifact_records.update(resources.items())
        else:
            raise InspectionError("missing or unreadable scoped class input")
        for name, data in entries:
            classes.add("L" + name.removesuffix(".class") + ";")
            class_records.add((name, digest(data)))
            artifact_records.add((name, digest(data)))
    if not classes or not any(c.startswith("Lcom/cantos/theatre/") for c in classes):
        raise InspectionError("pre-shrink class input is empty or has no Cantos classes")
    findings += class_findings(classes, forbidden)
    roots = [mobile / "shared/src" / s for s in ("commonMain", "nativeMain", "androidMain")]
    roots += [mobile / "androidApp/src/main", mobile / "androidApp/src" / variant]
    for root in roots:
        if not root.is_dir():
            continue
        for path in sorted(root.rglob("*")):
            if not path.is_file():
                continue
            name = path.relative_to(mobile).as_posix()
            sha = digest(path.read_bytes())
            source.append((name, sha))
            if sha in forbidden_hashes:
                findings.append("copied sandbox source/resource: " + name)
            if SANDBOX_TOKEN.search("/".join(path.relative_to(root).parts[:-1])):
                findings.append("fixture/mock source/resource path: " + name)
    def fingerprint(records):
        return digest(json.dumps(sorted(records), separators=(",", ":")).encode())
    return {"schema_version": 1, "variant": variant, "status": "FAIL" if findings else "PASS",
            "class_count": len(classes), "classes_sha256": fingerprint(class_records),
            "inputs_sha256": fingerprint(artifact_records),
            "source_sha256": fingerprint(source), "findings": findings}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mobile", required=True, type=Path)
    parser.add_argument("--variant", required=True)
    parser.add_argument("--classes", action="append", default=[], type=Path)
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args()
    result = inspect(args.mobile, args.variant, args.classes)
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2) + "\n")
    print(result["status"] + " pre-shrink " + args.variant + ": " + str(result["class_count"]) + " classes")
    if result["findings"]:
        raise InspectionError("; ".join(result["findings"]))


if __name__ == "__main__":
    main()
