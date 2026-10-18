#!/usr/bin/env python3
"""Inspect a production Android release APK and AAB for preview-sandbox and test content.

`cargo xtask theatre-host android-release` is the command surface; this is its primitive.
It reads the actual packaged bytes: classes defined in every DEX file, archive entries and
their content hashes, Compose resource packages, and the manifest decoded by the installed
`aapt2` (the AAB's proto manifest is repacked as a proto APK for the same decoder). The
forbidden class inventory comes from the sandbox's own compiled classes, so it cannot drift
from source; an empty inventory or missing required class is a failure, never a pass.

Exit 0 means no finding; 1 means findings; 2 means the inputs could not be inspected.
"""

import argparse
import hashlib
import re
import struct
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

# Test, mock and preview-tooling namespaces that a production release must never define.
TEST_NAMESPACES = (
    "Lkotlin/test/", "Lorg/junit/", "Ljunit/", "Lkotlinx/coroutines/test/", "Lio/mockk/",
    "Lio/kotest/", "Lio/ktor/client/engine/mock/", "Lokio/fakefilesystem/",
    "Landroidx/compose/ui/test/", "Landroidx/compose/ui/tooling/", "Lorg/mockito/",
)
CANTOS_SANDBOX_SEGMENTS = {"mock", "demo", "sandbox", "preview", "workbench"}
CANTOS_SANDBOX_NAME = re.compile(r"(?:Mock|Fake|Demo)[A-Z]|CantosDemo")
SANDBOX_TOKEN = re.compile(r"mock|demo|sandbox|preview|workbench|fixture", re.I)
COMPONENT_TAGS = {"activity", "activity-alias", "service", "receiver", "provider"}
ANDROID_NAMESPACE = "http://schemas.android.com/apk/res/android:"


class InspectionError(Exception):
    """The artifact or an input is unreadable; never reported as a finding-free result."""


def uleb128(data, offset):
    value, shift = 0, 0
    while True:
        if offset >= len(data):
            raise InspectionError("truncated ULEB128 in DEX string data")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, offset
        shift += 7
        if shift > 28:
            raise InspectionError("oversized ULEB128 in DEX string data")


def dex_defined_classes(data):
    """Descriptors of classes defined (not merely referenced) by one DEX file."""
    if len(data) < 0x70 or data[:4] != b"dex\n" or data[7] != 0:
        raise InspectionError("not a DEX file")
    if struct.unpack_from("<I", data, 0x28)[0] != 0x12345678:
        raise InspectionError("unsupported DEX endianness")
    string_size, string_off, type_size, type_off = struct.unpack_from("<4I", data, 0x38)
    class_size, class_off = struct.unpack_from("<2I", data, 0x60)
    for size, offset, width in ((string_size, string_off, 4), (type_size, type_off, 4), (class_size, class_off, 32)):
        if offset + size * width > len(data):
            raise InspectionError("DEX section exceeds file size")

    def descriptor(type_index):
        if type_index >= type_size:
            raise InspectionError("DEX class refers to a missing type")
        string_index = struct.unpack_from("<I", data, type_off + 4 * type_index)[0]
        if string_index >= string_size:
            raise InspectionError("DEX type refers to a missing string")
        start = struct.unpack_from("<I", data, string_off + 4 * string_index)[0]
        _, start = uleb128(data, start)
        end = data.find(b"\0", start)
        if end < 0:
            raise InspectionError("unterminated DEX string")
        # Modified UTF-8; class descriptors are ASCII in practice and never need repair.
        return data[start:end].decode("utf-8", errors="surrogateescape")

    return {descriptor(struct.unpack_from("<I", data, class_off + 32 * index)[0]) for index in range(class_size)}


def class_inventory(directory):
    """Descriptors of the compiled `.class` files under one sandbox output directory."""
    root = Path(directory)
    if not root.is_dir():
        raise InspectionError(f"forbidden class directory is missing: {root}")
    return {"L" + path.relative_to(root).with_suffix("").as_posix() + ";"
            for path in root.rglob("*.class") if "META-INF" not in path.relative_to(root).parts}


def resource_hashes(directory):
    root = Path(directory)
    if not root.is_dir():
        raise InspectionError(f"forbidden resource directory is missing: {root}")
    return {hashlib.sha256(path.read_bytes()).hexdigest(): path.relative_to(root).as_posix()
            for path in sorted(root.rglob("*")) if path.is_file()}


def class_findings(classes, forbidden):
    findings = []
    for name in sorted(classes):
        if name in forbidden:
            findings.append(f"defines sandbox class {name}")
        elif name.startswith(TEST_NAMESPACES):
            findings.append(f"defines test/tooling class {name}")
        elif name.startswith("Lcom/cantos/"):
            parts = name[1:-1].split("/")
            simple = parts[-1].split("$")[0]
            if CANTOS_SANDBOX_SEGMENTS.intersection(part.lower() for part in parts[:-1]) \
                    or CANTOS_SANDBOX_NAME.match(simple):
                findings.append(f"defines Cantos mock/demo/preview class {name}")
    return findings


def parse_xmltree(text):
    """Elements as (depth, tag, {attribute: value}) from `aapt2 dump xmltree` output."""
    elements = []
    for line in text.splitlines():
        element = re.match(r"( *)E: ([\w:-]+) \(line=\d+\)$", line)
        if element:
            elements.append((len(element.group(1)), element.group(2), {}))
            continue
        attribute = re.match(r" *A: ([^=(]+)(?:\(0x[0-9a-f]+\))?=(.*)$", line)
        if attribute and elements:
            name = attribute.group(1).removeprefix(ANDROID_NAMESPACE)
            value = attribute.group(2)
            quoted = re.match(r'"(.*)" \(Raw: ".*"\)$', value) or re.match(r'"(.*)"$', value)
            raw = re.match(r'(.*?) \(Raw: "(.*)"\)$', value)
            elements[-1][2][name] = quoted.group(1) if quoted else raw.group(2) if raw else value
    if not elements or elements[0][1] != "manifest":
        raise InspectionError("aapt2 did not return a manifest element tree")
    return elements


def manifest_findings(elements, application_id, launcher, allowed_permissions):
    findings = []
    manifest = elements[0][2]
    if manifest.get("package") != application_id:
        findings.append(f"manifest package {manifest.get('package')!r} is not {application_id!r}")
    launchers, components = [], []
    current_component = None
    for index, (depth, tag, attributes) in enumerate(elements):
        if tag in COMPONENT_TAGS:
            current_component = (depth, attributes.get("name", ""))
            components.append(current_component[1])
        elif current_component and depth <= current_component[0]:
            current_component = None
        if tag == "application":
            for flag in ("debuggable", "testOnly"):
                if attributes.get(flag, "false") not in {"false", "0x0", "0"}:
                    findings.append(f"application declares {flag}={attributes[flag]}")
        if tag in {"uses-permission", "uses-permission-sdk-23"} and attributes.get("name") not in allowed_permissions:
            findings.append(f"requests unexpected permission {attributes.get('name')}")
        if tag == "category" and attributes.get("name") == "android.intent.category.LAUNCHER" and current_component:
            launchers.append(current_component[1])
        if tag == "instrumentation":
            findings.append("declares instrumentation")
    if launchers != [launcher]:
        findings.append(f"launcher components are {launchers}, expected exactly [{launcher!r}]")
    for name in components:
        if name.startswith("com.cantos.") and name != launcher:
            findings.append(f"declares unexpected Cantos component {name}")
    for _, tag, attributes in elements:
        for key, value in attributes.items():
            if SANDBOX_TOKEN.search(value):
                findings.append(f"manifest {tag}@{key} names mock/demo/sandbox/preview content: {value}")
    return findings


def catalog_origin_findings(elements, expected_origin):
    """Check the built manifest, so a configured build cannot silently ship unavailable."""
    origins = [attrs.get("value", "") for _, tag, attrs in elements
               if tag == "meta-data" and attrs.get("name") == "CantosApiOrigin"]
    findings = []
    if origins != [expected_origin]:
        findings.append("catalog API origin metadata does not match the requested build origin")
    permissions = {attrs.get("name") for _, tag, attrs in elements if tag == "uses-permission"}
    if "android.permission.INTERNET" not in permissions:
        findings.append("configured catalog has no INTERNET permission")
    for _, tag, attrs in elements:
        if tag == "application" and attrs.get("usesCleartextTraffic", "false") not in {"false", "0", "0x0"}:
            findings.append("release catalog permits cleartext traffic")
    return findings


def archive_findings(entries, forbidden_hashes, compose_package):
    findings = []
    for name, digest in sorted(entries.items()):
        if digest in forbidden_hashes:
            findings.append(f"packages sandbox resource bytes {forbidden_hashes[digest]} as {name}")
        if SANDBOX_TOKEN.search(name):
            findings.append(f"packages mock/demo/sandbox/preview path {name}")
        resource = re.match(r"(?:base/)?assets/composeResources/([^/]+)/", name)
        if resource and resource.group(1) != compose_package:
            findings.append(f"packages foreign Compose resource package {resource.group(1)}")
    return findings


def read_archive(path, dex_pattern, manifest_name):
    try:
        with zipfile.ZipFile(path) as archive:
            names = archive.namelist()
            entries = {name: hashlib.sha256(archive.read(name)).hexdigest()
                       for name in names if not name.endswith("/")}
            dex = [name for name in names if re.fullmatch(dex_pattern, name)]
            if not dex:
                raise InspectionError(f"{path} contains no DEX files")
            classes = set()
            for name in dex:
                classes |= dex_defined_classes(archive.read(name))
            if manifest_name not in names:
                raise InspectionError(f"{path} has no {manifest_name}")
            manifest = archive.read(manifest_name)
            proto = archive.read("base/resources.pb") if "base/resources.pb" in names else None
    except (OSError, zipfile.BadZipFile) as error:
        raise InspectionError(f"cannot read {path}: {error}") from error
    return entries, classes, manifest, proto


def dump_manifest(aapt2, archive, manifest, proto):
    """Decode with installed aapt2; a bundle's proto manifest is repacked as a proto APK."""
    with tempfile.TemporaryDirectory() as scratch:
        target = archive
        if proto is not None:
            target = Path(scratch) / "base-proto.apk"
            with zipfile.ZipFile(target, "w") as repacked:
                repacked.writestr("AndroidManifest.xml", manifest)
                repacked.writestr("resources.pb", proto)
        result = subprocess.run([str(aapt2), "dump", "xmltree", "--file", "AndroidManifest.xml", str(target)],
                                capture_output=True, text=True, check=False)
    if result.returncode:
        raise InspectionError(f"aapt2 could not decode the manifest of {archive}: {result.stderr.strip()}")
    return result.stdout


def inspect(args, dump=dump_manifest):
    forbidden = set()
    for directory in args.forbidden_classes:
        forbidden |= class_inventory(directory)
    if not forbidden:
        raise InspectionError("forbidden sandbox class inventory is empty; inspection would be vacuous")
    forbidden_hashes = {}
    for directory in args.forbidden_resources:
        forbidden_hashes.update(resource_hashes(directory))
    report = [f"Production Android release artifact inspection: {args.application_id}",
              f"forbidden sandbox classes: {len(forbidden)}; forbidden resource files: {len(forbidden_hashes)}"]
    findings, defined = [], {}
    for label, path, dex_pattern, manifest_name in (
            ("apk", args.apk, r"classes\d*\.dex", "AndroidManifest.xml"),
            ("aab", args.aab, r"base/dex/classes\d*\.dex", "base/manifest/AndroidManifest.xml")):
        entries, classes, manifest, proto = read_archive(path, dex_pattern, manifest_name)
        defined[label] = classes
        digest = hashlib.sha256(Path(path).read_bytes()).hexdigest()
        report.append(f"{label} {path} sha256={digest} entries={len(entries)} defined_classes={len(classes)}")
        own = [f"{label}: {finding}" for finding in (
            class_findings(classes, forbidden)
            + archive_findings(entries, forbidden_hashes, args.compose_resource_package)
            + manifest_findings(parse_xmltree(dump(args.aapt2, path, manifest, proto)), args.application_id,
                                args.launcher, set(args.allowed_permission)))]
        own += [f"{label}: required production class {name} is absent"
                for name in args.require_class if name not in classes]
        if getattr(args, "api_origin", None) is not None:
            own += [f"{label}: {finding}" for finding in catalog_origin_findings(
                parse_xmltree(dump(args.aapt2, path, manifest, proto)), args.api_origin)]
        findings += own
    if defined["apk"] != defined["aab"]:
        findings.append("apk and aab define different class sets; they are not the same release build")
    report += [f"FINDING {finding}" for finding in findings] or ["PASS: no sandbox, mock, demo or test content found"]
    return findings, report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--apk", required=True, type=Path)
    parser.add_argument("--aab", required=True, type=Path)
    parser.add_argument("--aapt2", required=True, type=Path)
    parser.add_argument("--application-id", required=True)
    parser.add_argument("--launcher", required=True, help="fully qualified launcher activity")
    parser.add_argument("--compose-resource-package", required=True)
    parser.add_argument("--forbidden-classes", action="append", required=True, type=Path)
    parser.add_argument("--forbidden-resources", action="append", default=[], type=Path)
    parser.add_argument("--require-class", action="append", default=[])
    parser.add_argument("--allowed-permission", action="append", default=[])
    parser.add_argument("--api-origin", help="expected production HTTPS catalog origin in both manifests")
    parser.add_argument("--report", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        findings, report = inspect(args)
    except InspectionError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 2
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text("\n".join(report) + "\n", encoding="utf-8")
    print("\n".join(report))
    print(f"Report: {args.report}")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
