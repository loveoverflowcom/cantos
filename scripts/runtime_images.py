"""Source image policy primitive for the existing repository gate (no decoder/CLI).

Checks suffixes AND signatures, plus SVG image references. A matching signature is
not full decoding, rights, bundle inspection or deployed acceptance. Legacy bytes may
appear only as explicitly named rejection vectors, never positive fixtures.
The canonical policy and transition boundary live in docs/design/runtime-images.md.
"""

from pathlib import Path
import hashlib
import re
import xml.etree.ElementTree as ET


LEGACY_SUFFIXES = {".png", ".jpg", ".jpeg", ".avif", ".gif", ".bmp", ".ico", ".tif", ".tiff", ".heic", ".heif", ".jxl", ".icns", ".avifs"}
DOC_ROOTS = ("docs/evidence/", "docs/design/", "docs/archive/")
LEGACY_REFERENCE = re.compile(r"(?:data:image/(?:png|jpe?g|webp|avifs?|gif|bmp)|\.(?:png|jpe?g|avifs?|gif|bmp)(?:$|[?#]))", re.I)


# Frozen actual-byte rejection vectors. Never consumed as runtime or positive media.
AVIF_REJECTIONS = {
    "apps/cantos-theatre/mobile/shared/src/desktopTest/resources/avif/grid-2x2-120x100.avif":
        "b7689baf90d1b5ea24c0721072416710ca7a75b88747621a3be483fc208de296",
    "apps/cantos-theatre/mobile/shared/src/desktopTest/resources/avif/grid-alpha-2x2-120x100.avif":
        "e4d69222be0f7d9a9370f964b7905ad90123d41adabb0fe53d3d30a61ab129dc",
    "apps/cantos-theatre/mobile/shared/src/desktopTest/resources/avif/pillow-alpha-64x48.avif":
        "2a204a341629a08142895345749cb9df1eb3c36692c9c60e538839d652336482",
    "scripts/fixtures/runtime-images/red-2x3.avif":
        "d200d30121283e2053867e2aa720e3b4b8e9fed50d3c006e3c893e36ed2f99d7",
    "services/cantos-server/src/contexts/media/image_intake/fixtures/ffmpeg-yuv420p-2x3.avif":
        "7c2306b2967d74b97c08809e1aa73b4923ab830a71cdecdf7acf2f0ba6873257",
    "services/cantos-server/src/contexts/media/image_intake/fixtures/grid-2x2-120x100.avif":
        "b7689baf90d1b5ea24c0721072416710ca7a75b88747621a3be483fc208de296",
    "services/cantos-server/src/contexts/media/image_intake/fixtures/grid-alpha-2x2-120x100.avif":
        "e4d69222be0f7d9a9370f964b7905ad90123d41adabb0fe53d3d30a61ab129dc",
    "services/cantos-server/src/contexts/media/image_intake/fixtures/pillow-alpha-64x48.avif":
        "2a204a341629a08142895345749cb9df1eb3c36692c9c60e538839d652336482",
    "services/cantos-server/src/contexts/media/image_intake/fixtures/pillow-rgb-2x3.avif":
        "f94b8de623a2287d6f5512a30bbc3e2fbb4e2025c2e4b628918806018e16a7f3",
    "services/cantos-server/src/contexts/media/image_intake/fixtures/python-rgb-2x3.avif":
        "ed4244268e5d34fc1e9ed66e65822d5c58333a4de4376c0d14023ebff966891f",
    "services/cantos-server/src/contexts/media/image_intake/fixtures/python-rgba16-2x3.avif":
        "d200d30121283e2053867e2aa720e3b4b8e9fed50d3c006e3c893e36ed2f99d7",
}

# Only the existing CMP paths/bytes may survive the Server/Web integration commit.
# Once the WebP fallback is present, the repository refuses every one of these paths.
CMP_TRANSITION = {
    "apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/composeResources/files/artwork/den_khuya.avif":
        "d053df56699afb2c1ff6031d3a34d636a86c802473b69f2e383bf2e3c76e392f",
    "apps/cantos-theatre/mobile/preview-sandbox/src/commonMain/composeResources/files/artwork/son_ha_moon.avif":
        "2abd065841decb84fbe8741f725ed553534f60b50e907ff34d0524dc5a95c171",
    "apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/artwork/cantos_cover_fallback.avif":
        "dfcf6b4d8405426bbd833db6629b6b14c9ca9a4990d9c85bd82321d759b5e4a5",
    "apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/artwork/library_expressive.avif":
        "88fdbb675291ac016955976b91504c9e5a423293160b9336f563228853cb0f2c",
}
CMP_WEBP_FALLBACK = (
    "apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/files/artwork/"
    "cantos_cover_fallback.webp"
)


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


def opaque_vp8(data):
    if len(data) < 30 or data[:4] != b"RIFF" or data[8:16] != b"WEBPVP8 ":
        return False
    chunk = int.from_bytes(data[16:20], "little")
    return (chunk >= 10 and len(data) == 20 + chunk + (chunk % 2)
            and int.from_bytes(data[4:8], "little") == len(data) - 8
            and (not chunk % 2 or data[-1] == 0)
            and data[20] & 1 == 0 and data[23:26] == b"\x9d\x01\x2a"
            and all(1 <= (int.from_bytes(data[offset:offset+2], "little") & 0x3fff) <= 4096
                    for offset in (26, 28)))


def image_failure(name, data):
    """Return a source-format violation; bytes are not admitted as a runtime image."""
    suffix = Path(name).suffix.lower()
    kind = signature(data)
    if suffix in LEGACY_SUFFIXES or kind in LEGACY_SUFFIXES:
        return "legacy/unsupported raster; convert actual bytes to opaque lossy WebP"
    if suffix == ".webp":
        if not opaque_vp8(data):
            return "WebP suffix requires one opaque still VP8 chunk"
    elif kind == ".webp":
        return "WebP bytes must use the .webp suffix"
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
    transitioning = not (root / CMP_WEBP_FALLBACK).exists()
    for name in sorted(set(names)):
        path = root / name
        if not path.is_file():
            continue
        if name.startswith(DOC_ROOTS) and path.suffix.lower() in {".png", ".jpg", ".jpeg"}:
            continue
        data = path.read_bytes()
        if name in AVIF_REJECTIONS and hashlib.sha256(data).hexdigest() == AVIF_REJECTIONS[name]:
            continue
        if transitioning and name in CMP_TRANSITION and hashlib.sha256(data).hexdigest() == CMP_TRANSITION[name]:
            debt.append(name)
            continue
        failure = image_failure(name, data)
        if failure:
            errors.append(f"{name}: {failure}")
    return errors, debt
