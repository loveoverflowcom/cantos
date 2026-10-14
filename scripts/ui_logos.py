#!/usr/bin/env python3
"""Map the checksum-pinned official Cantos SVG documents to local CMP vector data."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "packages/ui-system/shared/assets/cantos-logo-kit-v2"
CMP_OUTPUT = ROOT / "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/generated/CantosLogos.kt"
SVG_NAMESPACE = "{http://www.w3.org/2000/svg}"
ALLOWED = {
    "svg": {"viewBox", "width", "height", "role", "aria-labelledby"},
    "title": {"id"}, "g": {"transform", "fill"}, "defs": set(),
    "mask": {"id"},
    "rect": {"width", "height", "rx", "fill", "mask"},
    "path": {"d", "fill", "stroke", "stroke-width", "stroke-linecap"},
}


def tag(node):
    return node.tag.removeprefix(SVG_NAMESPACE)


def validate_anatomy(node):
    name = tag(node)
    if name not in ALLOWED or not set(node.attrib) <= ALLOWED[name]:
        raise ValueError("unsupported logo SVG anatomy")
    if node.text and node.text.strip() and name != "title":
        raise ValueError("logo SVG text must be outlined")
    for child in node:
        validate_anatomy(child)


def checked_assets():
    manifest = json.loads((ASSETS / "provenance.json").read_text())
    for name, entry in manifest["files"].items():
        if hashlib.sha256((ASSETS / name).read_bytes()).hexdigest() != entry["sha256"]:
            raise ValueError(f"official logo source hash mismatch: {name}")
    return manifest


def color(value):
    if value is None or value == "none":
        return None
    if not re.fullmatch(r"#[0-9A-Fa-f]{6}", value):
        raise ValueError(f"unsupported logo color: {value}")
    return "0xFF" + value[1:].upper() + "L"


def shapes(svg):
    """Preserve filled outlines, rounded rectangles, translated strokes and mask holes."""
    validate_anatomy(svg)
    masks = {node.attrib["id"]: node for node in svg.iter() if tag(node) == "mask"}
    if set(masks) - {"knockout"}:
        raise ValueError("unsupported logo mask")
    result = []

    def walk(node, fill=None, offset=(0.0, 0.0), knockout=False):
        name = tag(node)
        if name in {"title", "defs"}:
            return
        if name == "g":
            transform = node.attrib.get("transform")
            if transform:
                translation = re.fullmatch(r"translate\((\d+) (\d+)\)", transform)
                if not translation:
                    raise ValueError("unsupported logo transform")
                offset = tuple(a + float(b) for a, b in zip(offset, translation.groups()))
            for child in node:
                walk(child, node.attrib.get("fill", fill), offset, knockout)
            return
        if name == "svg":
            for child in node:
                walk(child, fill, offset)
            return
        if name not in {"path", "rect"}:
            raise ValueError("unmapped logo anatomy")
        attributes = {"translateX": offset[0], "translateY": offset[1], "knockout": knockout}
        if name == "path":
            attributes["pathData"] = node.attrib["d"]
            if node.attrib.get("stroke-linecap", "butt") not in {"butt", "round"}:
                raise ValueError("unsupported logo stroke cap")
            attributes["roundCap"] = node.attrib.get("stroke-linecap") == "round"
            attributes["strokeWidth"] = float(node.attrib.get("stroke-width", 0))
        else:
            attributes.update(width=float(node.attrib["width"]), height=float(node.attrib["height"]),
                              radius=float(node.attrib.get("rx", 0)))
        if knockout:
            if node.attrib.get("stroke") != "black" or node.attrib.get("fill") != "none":
                raise ValueError("unsupported knockout: expected black stroked holes")
            attributes["stroke"] = "0xFF000000L"
        else:
            attributes["fill"] = color(node.attrib.get("fill", fill))
            attributes["stroke"] = color(node.attrib.get("stroke"))
        result.append(attributes)
        if "mask" in node.attrib:
            if node.attrib["mask"] != "url(#knockout)" or "knockout" not in masks or name != "rect":
                raise ValueError("unsupported logo mask target")
            mask_children = list(masks["knockout"])
            if not mask_children or tag(mask_children[0]) != "rect" or mask_children[0].attrib != {
                    "width": "64", "height": "64", "rx": "22", "fill": "white"}:
                raise ValueError("unsupported logo mask boundary")
            if len(mask_children) != 3:
                raise ValueError("unsupported logo mask holes")
            for hole in mask_children[1:]:
                walk(hole, offset=offset, knockout=True)

    walk(svg)
    return result


def kotlin_value(name, value):
    if value is None:
        return "null"
    if isinstance(value, bool):
        return str(value).lower()
    if isinstance(value, float):
        return f"{value:g}f" if "." in f"{value:g}" else f"{value:.1f}f"
    if name in {"fill", "stroke"}:
        return value
    return json.dumps(value)


def render_kotlin():
    manifest = checked_assets()
    lines = ["// Generated by python3 scripts/ui_logos.py. Do not edit.",
             "// Official Cantos Logo Kit v2. Rights/checksums: shared/assets/cantos-logo-kit-v2/.",
             "package com.cantos.ui.generated", "",
             "internal data class CantosLogoShape(",
             "    val pathData: String? = null, val fill: Long? = null, val stroke: Long? = null,",
             "    val strokeWidth: Float = 0f, val roundCap: Boolean = false,",
             "    val width: Float = 0f, val height: Float = 0f, val radius: Float = 0f,",
             "    val translateX: Float = 0f, val translateY: Float = 0f, val knockout: Boolean = false,",
             ")", "", "internal data class CantosLogoAsset(",
             "    val file: String, val width: Float, val height: Float, val shapes: List<CantosLogoShape>,",
             ")", "", "internal object CantosLogos {", "    val assets: List<CantosLogoAsset> = listOf("]
    for variant in manifest["variants"]:
        svg = ET.fromstring((ASSETS / "svg" / variant["file"]).read_bytes())
        width, height = variant["viewBox"]
        if svg.attrib.get("viewBox") != f"0 0 {width} {height}":
            raise ValueError("logo viewBox disagrees with official manifest")
        lines.append(f'        CantosLogoAsset("{variant["file"]}", {width}.0f, {height}.0f, listOf(')
        for shape in shapes(svg):
            arguments = ", ".join(f"{name} = {kotlin_value(name, value)}" for name, value in shape.items()
                                  if value is not None and value is not False and value != 0.0)
            lines.append(f"            CantosLogoShape({arguments}),")
        lines.append("        )),")
    lines += ["    )", "", "    fun asset(file: String): CantosLogoAsset =",
              '        requireNotNull(assets.find { it.file == file }) { "Unknown official logo: $file" }', "}", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = render_kotlin()
    if args.check:
        if not CMP_OUTPUT.exists() or CMP_OUTPUT.read_text() != expected:
            raise SystemExit("FAIL: generated official logo drift")
        print("PASS: official logo checksums, vector geometry and transparent knockout mappings agree")
    else:
        CMP_OUTPUT.write_text(expected)


if __name__ == "__main__":
    main()
