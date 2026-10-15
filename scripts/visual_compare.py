#!/usr/bin/env python3
"""Offline PNG adapter for cargo xtask visual-compare. No network or baseline writes."""

import argparse
from datetime import datetime, timezone
import hashlib
import io
import json
from pathlib import Path
import re
import struct
import sys
import time

from visual_compare_core import Rect, analyze_mask

ROOT = Path(__file__).resolve().parents[1]
PILLOW_VERSION = "11.3.0"
PAIR_FIELDS = ("viewport", "pixel_ratio", "font", "theme", "locale", "text_scale", "fixture", "state")
CAPTURE_FIELDS = ("source_sha", "renderer", *PAIR_FIELDS, "captured_at")


def digest(data):
    return hashlib.sha256(data).hexdigest()


def pillow():
    try:
        import PIL
        from PIL import Image, ImageChops, ImageDraw
    except ImportError as error:
        raise ImportError("optional Pillow 11.3.0 required; see tools/visual-compare/README.md") from error
    if PIL.__version__ != PILLOW_VERSION:
        raise ImportError(f"Pillow {PILLOW_VERSION} required, found {PIL.__version__}; no install attempted")
    return Image, ImageChops, ImageDraw


def read_png(path, Image):
    path = Path(path)
    with path.open("rb") as source:
        data = source.read(64 * 1024 * 1024 + 1)
    if len(data) > 64 * 1024 * 1024:
        raise ValueError("PNG exceeds 64 MiB input bound")
    if len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[12:16] != b"IHDR":
        raise ValueError("PNG signature/IHDR required")
    width, height = struct.unpack(">II", data[16:24])
    if width == 0 or height == 0 or width * height > 16_000_000:
        raise ValueError("PNG dimensions invalid or above 16 million pixels")
    if len(data) < 29 or data[24] not in (1, 2, 4, 8):
        raise ValueError("PNG requires at most 8-bit channels; 16-bit quantization is not supported")
    with Image.open(io.BytesIO(data)) as image:
        if image.format != "PNG" or getattr(image, "n_frames", 1) != 1:
            raise ValueError("input must be a single-frame PNG")
        if image.width * image.height > 16_000_000:
            raise ValueError("PNG exceeds 16 million pixels")
        return data, image.convert("RGBA")


def changed_mask(reference, actual, threshold, ImageChops):
    # Premultiplication ignores hidden RGB at alpha=0; the separate alpha channel still
    # detects opacity changes. No resizing, color-profile transform or AA heuristic.
    channels = ImageChops.difference(reference.convert("RGBa"), actual.convert("RGBa")).split()
    delta = channels[0]
    for channel in channels[1:]:
        delta = ImageChops.lighter(delta, channel)
    return delta, delta.point(lambda value: 255 if value > threshold else 0).tobytes()


def unique_fields(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate metadata key: {key}")
        result[key] = value
    return result


def reject_nonfinite(value):
    raise ValueError(f"non-finite metadata number: {value}")


def metadata_from(path):
    if path is None:
        return {}
    if path.stat().st_size > 64 * 1024:
        raise ValueError("metadata exceeds 64 KiB")
    value = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_fields,
                       parse_constant=reject_nonfinite)
    if not isinstance(value, dict) or set(value) - {"baseline", "reference", "actual", "masks"}:
        raise ValueError("metadata keys: baseline, reference, actual, masks")
    for side in ("baseline", "reference", "actual"):
        if not isinstance(value.get(side, {}), dict):
            raise ValueError(f"{side} metadata must be an object")
    masks = value.get("masks", [])
    if not isinstance(masks, list) or len(masks) > 128:
        raise ValueError("masks must be a list of at most 128 reviewed rectangles")
    for mask in masks:
        if not isinstance(mask, dict) or set(mask) != {
            "x", "y", "width", "height", "reason", "reviewed_by", "review_record"
        }:
            raise ValueError("mask requires rectangle, reason, reviewed_by and review_record")
        Rect(**{key: mask[key] for key in ("x", "y", "width", "height")})
        for key in ("reason", "reviewed_by", "review_record"):
            if not isinstance(mask[key], str) or not mask[key].strip() or mask[key] == "unknown":
                raise ValueError(f"mask {key} must name real reviewed evidence")
    return value


def provenance(metadata, reference_bytes, actual_bytes):
    result = {}
    for side, data in (("reference", reference_bytes), ("actual", actual_bytes)):
        given = metadata.get(side, {})
        if given.get("sha256") not in (None, "unknown", digest(data)):
            raise ValueError(f"{side} SHA-256 does not match supplied PNG bytes")
        result[side] = {field: given.get(field, "unknown") if given.get(field) is not None else "unknown"
                        for field in CAPTURE_FIELDS}
        result[side]["sha256"] = digest(data)
    baseline = metadata.get("baseline", {})
    if baseline.get("sha256") not in (None, "unknown", digest(reference_bytes)):
        raise ValueError("baseline SHA-256 does not match reference PNG")
    approval = baseline.get("approval", {"status": "unreviewed", "owner": "unknown", "record": "unknown"})
    if not isinstance(approval, dict) or approval.get("status") not in ("approved", "calibration", "unreviewed"):
        raise ValueError("baseline approval status must be approved, calibration or unreviewed")
    if approval["status"] in ("approved", "calibration"):
        if any(not isinstance(approval.get(k), str) or approval[k].strip() in ("", "unknown")
               for k in ("owner", "record")):
            raise ValueError("baseline approval/calibration requires an owner and review record")
        if baseline.get("sha256") != digest(reference_bytes):
            raise ValueError("approved/calibration baseline requires exact SHA-256 pin")
        if any(not isinstance(baseline.get(k), str) or baseline[k].strip() in ("", "unknown")
               for k in ("id", "revision")):
            raise ValueError("approved/calibration baseline requires versioned id and revision")
    result["baseline"] = dict(id=baseline.get("id", "unknown"), revision=baseline.get("revision", "unknown"),
                              sha256=digest(reference_bytes), approval=approval)
    return result


def fresh_output(path, scenario):
    parent = (ROOT / "artifacts/ui-evidence").resolve()
    if path is None:
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
        path = parent / f"{stamp}-{time.time_ns()}-{scenario}" / "comparison"
    path = path.resolve()
    if parent not in path.parents:
        raise ValueError("output must be a new directory below artifacts/ui-evidence")
    path.mkdir(parents=True, exist_ok=False)
    return path


def write_report(output, report):
    (output / "report.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    lines = [f"# Pixel comparison — {report['scenario']}", "", f"Result: `{report['result']}`.", "",
             "Advisory pixels only. No UX, accessibility, behavior, native-device or baseline approval.", "",
             "![Reference](reference.png) ![Actual](actual.png)", ""]
    if "diff_pixels" in report:
        lines += [f"Changed: {report['diff_pixels']} / {report['total_pixels']} pixels; "
                  f"diff_ratio={report['diff_ratio']:.8f} (whole-image denominator).",
                  f"Excluded: {report['excluded_pixels']}; compared: {report['compared_pixels']}.", "",
                  "![Overlay with numbered regions](overlay.png)", "![Difference](diff.png)",
                  "![Mask](mask.png) ![Heatmap](heatmap.png)", "",
                  "| ID | x, y, width, height (px) | Pixels | Local ratio | Area hint, not severity |",
                  "| --- | --- | --- | --- | --- |"]
        for region in report["regions"]:
            lines.append(f"| {region['id']} | {region['x']}, {region['y']}, {region['width']}, {region['height']} "
                         f"| {region['pixels']} | {region['local_ratio']:.6f} | {region['severity_hint']} |")
        for region in report["regions"]:
            for name, file in region.get("images", {}).items():
                lines.append(f"\n![Region {region['id']} {name}]({file})\n")
    lines += ["", "Provenance, thresholds, mask review records, hashes and pairing gaps: [report.json](report.json).",
              "Images must be opened and findings written separately; this report does not claim inspection."]
    (output / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")


def compare(reference_path, actual_path, scenario, metadata, output_path=None, threshold=0, tile_size=16):
    if not re.fullmatch(r"[a-zA-Z0-9][a-zA-Z0-9_-]{0,79}", scenario):
        raise ValueError("scenario must be a 1–80 character identifier")
    if type(threshold) is not int or not 0 <= threshold <= 255:
        raise ValueError("threshold must be an integer from 0 to 255")
    Image, ImageChops, ImageDraw = pillow()
    reference_bytes, reference = read_png(reference_path, Image)
    actual_bytes, actual = read_png(actual_path, Image)
    prov = provenance(metadata, reference_bytes, actual_bytes)
    mismatches = [field for field in PAIR_FIELDS if prov["reference"][field] != "unknown"
                  and prov["actual"][field] != "unknown" and prov["reference"][field] != prov["actual"][field]]
    unknowns = [f"{side}.{field}" for side in ("reference", "actual") for field in CAPTURE_FIELDS
                if prov[side][field] in (None, "unknown")]
    masks = metadata.get("masks", [])
    policy = dict(scenario=scenario, baseline_sha256=digest(reference_bytes), threshold=threshold,
                  tile_size=tile_size, masks=masks,
                  comparator_version=PILLOW_VERSION,
                  comparator_sha256=digest(Path(__file__).read_bytes()),
                  core_sha256=digest((ROOT / "scripts/visual_compare_core.py").read_bytes()),
                  pairing={side: {field: prov[side][field] for field in PAIR_FIELDS}
                           for side in ("reference", "actual")})
    report = dict(schema_version=1, scenario=scenario, provenance=prov,
                  compared_at=datetime.now(timezone.utc).isoformat(),
                  comparator={"name": "Pillow premultiplied-RGBA max-channel", "version": PILLOW_VERSION,
                              "threshold": threshold, "tile_size": tile_size, "anti_alias_filter": "none",
                              "color_management": "encoded channels; no ICC/gamma transform",
                              "source_sha256": digest(Path(__file__).read_bytes()),
                              "core_sha256": digest((ROOT / "scripts/visual_compare_core.py").read_bytes())},
                  masks=masks, pairing_mismatches=mismatches, provenance_gaps=unknowns,
                  recheck_policy_sha256=digest(json.dumps(policy, sort_keys=True).encode()),
                  dimensions={"reference": list(reference.size), "actual": list(actual.size)},
                  images_inspected=False)
    if reference.size != actual.size or mismatches:
        report["result"] = "dimension_mismatch" if reference.size != actual.size else "scenario_mismatch"
        output = fresh_output(output_path, scenario)
        (output / "reference.png").write_bytes(reference_bytes)
        (output / "actual.png").write_bytes(actual_bytes)
        write_report(output, report)
        return output, report
    delta, changed = changed_mask(reference, actual, threshold, ImageChops)
    rectangles = tuple(Rect(**{k: mask[k] for k in ("x", "y", "width", "height")}) for mask in masks)
    measured, visible = analyze_mask(*reference.size, changed, rectangles, tile_size)
    report.update(measured)
    report["result"] = "different" if report["diff_pixels"] else "equal"
    report["regions_omitted"] = max(0, len(report["regions"]) - 256)
    report["regions"] = report["regions"][:256]
    output = fresh_output(output_path, scenario)
    (output / "reference.png").write_bytes(reference_bytes)
    (output / "actual.png").write_bytes(actual_bytes)
    mask = Image.frombytes("L", reference.size, visible)
    mask.save(output / "mask.png")
    def flattened(image):
        return Image.alpha_composite(Image.new("RGBA", image.size, "white"), image).convert("RGB")
    left, right = flattened(reference), flattened(actual)
    overlay = Image.blend(left, right, .5)
    diff = Image.blend(right, Image.new("RGB", right.size, "white"), .65)
    diff.paste((255, 0, 80), mask=mask)
    diff.save(output / "diff.png")
    heatmap = Image.new("RGB", reference.size, "black")
    heatmap.paste(Image.merge("RGB", (delta, Image.new("L", delta.size), Image.new("L", delta.size))), mask=mask)
    heatmap.save(output / "heatmap.png")
    draw = ImageDraw.Draw(overlay)
    for rect in rectangles:
        draw.rectangle((rect.x, rect.y, rect.x + rect.width - 1, rect.y + rect.height - 1), outline="cyan", width=2)
    for region in report["regions"]:
        x, y, w, h = (region[k] for k in ("x", "y", "width", "height"))
        draw.rectangle((x, y, x + w - 1, y + h - 1), outline="red", width=2)
        draw.text((x, max(0, y - 12)), str(region["id"]), fill="red")
    overlay.save(output / "overlay.png")
    for region in report["regions"][:3]:
        x, y, w, h = (region[k] for k in ("x", "y", "width", "height"))
        box = (max(0, x - 8), max(0, y - 8), min(reference.width, x + w + 8), min(reference.height, y + h + 8))
        region["images"] = {}
        for name, source in (("reference", left), ("actual", right), ("overlay", overlay)):
            filename = f"region-{region['id']}-{name}.png"
            source.crop(box).save(output / filename)
            region["images"][name] = filename
    report["artifact_sha256"] = {p.name: digest(p.read_bytes()) for p in sorted(output.glob("*.png"))}
    write_report(output, report)
    return output, report


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--actual", required=True, type=Path)
    parser.add_argument("--scenario", required=True)
    parser.add_argument("--metadata", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--threshold", type=int, default=0)
    parser.add_argument("--tile-size", type=int, default=16)
    args = parser.parse_args(argv)
    try:
        output, report = compare(args.reference, args.actual, args.scenario, metadata_from(args.metadata),
                                 args.output, args.threshold, args.tile_size)
        print(f"{report['result']}: {output / 'report.json'}")
        return 2 if report["result"].endswith("mismatch") else 0
    except ImportError as error:
        print(f"UNSUPPORTED: {error}", file=sys.stderr)
        return 3
    except (OSError, ValueError, TypeError, KeyError, RecursionError) as error:
        print(f"visual-compare: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
