"""Source image policy primitive for the existing repository gate (no decoder/CLI).

Checks suffixes AND signatures, plus SVG image references. A matching signature is
not full decoding, rights, bundle inspection or deployed acceptance. Legacy bytes may
appear only as explicitly named rejection vectors, never positive fixtures.
The canonical policy and transition boundary live in docs/design/runtime-images.md.
"""

from pathlib import Path
import re
import xml.etree.ElementTree as ET


LEGACY_SUFFIXES = {".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp", ".ico", ".tif", ".tiff", ".heic", ".heif", ".jxl", ".icns", ".avifs"}
DOC_ROOTS = ("docs/evidence/", "docs/design/", "docs/archive/")
LEGACY_REFERENCE = re.compile(r"(?:data:image/(?:png|jpe?g|webp|gif|bmp)|\.(?:png|jpe?g|webp|gif|bmp)(?:$|[?#]))", re.I)


def signature(data):
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        return ".png"
    if data.startswith(b"\xff\xd8\xff"):
        return ".jpg"
    if data[:6] in (b"GIF87a", b"GIF89a"):
        return ".gif"
    if data[:4] == b"RIFF" and data[8:12] == b"WEBP":
        return ".webp"
    if (len(data) >= 26 and data.startswith(b"BM")
            and 26 <= int.from_bytes(data[2:6], "little") <= len(data)
            and 14 <= int.from_bytes(data[10:14], "little") <= len(data)
            and int.from_bytes(data[14:18], "little") in {12, 16, 40, 52, 56, 64, 108, 124}):
        return ".bmp"
    if data[:4] in (b"II*\0", b"MM\0*"):
        return ".tiff"
    if data[:4] == b"\0\0\1\0":
        return ".ico"
    if data[:4] == b"icns":
        return ".icns"
    if len(data) >= 16 and data[4:8] == b"ftyp":
        size = int.from_bytes(data[:4], "big")
        if size < 16 or size % 4 or size > len(data):
            return ".bmff"
        brands = {data[8:12], *(data[index:index + 4] for index in range(16, size, 4))}
        if b"avif" in brands and b"avis" not in brands:
            return ".avif"
        if b"avis" in brands:
            return ".avifs"
        if brands & {b"heic", b"heix", b"hevc", b"hevx", b"mif1", b"msf1"}:
            return ".heif"
        return ".bmff"
    return None


def image_failure(name, data):
    """Return a source-format violation; bytes are not admitted as a runtime image."""
    suffix = Path(name).suffix.lower()
    kind = signature(data)
    if suffix in LEGACY_SUFFIXES or kind in LEGACY_SUFFIXES:
        return "legacy/unsupported raster; convert actual bytes to AVIF"
    if suffix == ".avif":
        if kind != ".avif":
            return "AVIF suffix does not match a still-AVIF ftyp signature"
    elif kind == ".avif":
        return "AVIF bytes must use the .avif suffix"
    if suffix == ".svg":
        try:
            svg = ET.fromstring(data)
        except ET.ParseError:
            return "malformed SVG"
        if svg.tag.rsplit("}", 1)[-1] != "svg":
            return "SVG suffix does not contain an SVG document"
        for node in svg.iter():
            for key, value in node.attrib.items():
                if key.rsplit("}", 1)[-1] in {"href", "src"} and LEGACY_REFERENCE.search(value):
                    return "SVG embeds or references a legacy raster"
    return None


def repository_failures(root, names):
    errors, debt = [], []
    for name in sorted(set(names)):
        path = root / name
        if not path.is_file():
            continue
        if name.startswith(DOC_ROOTS) and path.suffix.lower() in {".png", ".jpg", ".jpeg"}:
            continue
        data = path.read_bytes()
        failure = image_failure(name, data)
        if failure:
            errors.append(f"{name}: {failure}")
    return errors, debt
