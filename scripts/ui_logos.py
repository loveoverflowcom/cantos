#!/usr/bin/env python3
"""Map the checksum-pinned official Cantos SVG documents to local CMP vector data."""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "packages/ui-system/shared/assets/cantos-logo-kit-v2"
CMP_OUTPUT = ROOT / "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/generated/CantosLogos.kt"
MOBILE = ROOT / "apps/cantos-theatre/mobile"
ANDROID_RES = MOBILE / "androidApp/src/main/res"
IOS_ICONS = MOBILE / "iosApp/Cantos/AppIcon.icon"
LAUNCHER_PROVENANCE = MOBILE / "launcher-icons.provenance.json"
ANDROID_NAMESPACE = "http://schemas.android.com/apk/res/android"
# Native layout only: keep the original paths and stroke widths; the OS owns the tile mask.
LAUNCHER_SCALE = 1.2
LAUNCHER_OFFSET = 15.6  # (108 - 64 * 1.2) / 2
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


def launcher_symbol(theme):
    """Read the official symbol; never duplicate its geometry or palette in this generator."""
    checked_assets()
    suffix = "" if theme == "light" else f"-{theme}"
    svg = ET.fromstring((ASSETS / f"svg/cantos-symbol{suffix}.svg").read_bytes())
    shapes(svg)
    return svg


def launcher_svg(theme):
    """Full-bleed native tile, without a second rounded edge inside the system mask."""
    svg = launcher_symbol(theme)
    rect = next(node for node in svg if tag(node) == "rect")
    rect.attrib.pop("rx")
    return ET.tostring(svg, encoding="unicode")


def launcher_vector(theme):
    svg = launcher_symbol(theme)
    paths = [node for node in svg.iter() if tag(node) == "path"]
    lines = ['<?xml version="1.0" encoding="utf-8"?>',
             '<!-- Generated by native launcher mode in scripts/ui_logos.py; do not edit. -->',
             f'<vector xmlns:android="{ANDROID_NAMESPACE}" android:width="108dp"',
             '    android:height="108dp" android:viewportWidth="108" android:viewportHeight="108">',
             f'    <group android:scaleX="{LAUNCHER_SCALE}" android:scaleY="{LAUNCHER_SCALE}"',
             f'        android:translateX="{LAUNCHER_OFFSET}" android:translateY="{LAUNCHER_OFFSET}">']
    for path in paths:
        # Mono uses the official knockout strokes as positive tintable foreground ink.
        stroke = "#FFFFFF" if theme == "mono" else path.attrib["stroke"]
        lines += [f'        <path android:pathData="{path.attrib["d"]}"',
                  f'            android:fillColor="@android:color/transparent" android:strokeColor="{stroke}"',
                  f'            android:strokeWidth="{path.attrib["stroke-width"]}" android:strokeLineCap="round" />']
    return "\n".join(lines + ['    </group>', '</vector>', ''])


def launcher_text_outputs():
    outputs = {}
    for theme, qualifier in [("light", ""), ("dark", "-night")]:
        outputs[ANDROID_RES / f"drawable{qualifier}/ic_launcher_foreground.xml"] = launcher_vector(theme)
        background = next(node for node in launcher_symbol(theme) if tag(node) == "rect").attrib["fill"]
        outputs[ANDROID_RES / f"values{qualifier}/launcher_colors.xml"] = (
            '<?xml version="1.0" encoding="utf-8"?>\n'
            '<!-- Generated by native launcher mode in scripts/ui_logos.py; do not edit. -->\n'
            f'<resources><color name="ic_launcher_background">{background}</color></resources>\n')
    outputs[ANDROID_RES / "drawable/ic_launcher_monochrome.xml"] = launcher_vector("mono")
    adaptive = ('<?xml version="1.0" encoding="utf-8"?>\n'
                '<!-- Generated by native launcher mode in scripts/ui_logos.py; do not edit. -->\n'
                f'<adaptive-icon xmlns:android="{ANDROID_NAMESPACE}">\n'
                '    <background android:drawable="@color/ic_launcher_background" />\n'
                '    <foreground android:drawable="@drawable/ic_launcher_foreground" />\n'
                '    <monochrome android:drawable="@drawable/ic_launcher_monochrome" />\n'
                '</adaptive-icon>\n')
    outputs[ANDROID_RES / "mipmap-anydpi-v26/ic_launcher.xml"] = adaptive
    # Night outranks density/API during resource selection; retain adaptive masking in night.
    outputs[ANDROID_RES / "mipmap-night-anydpi-v26/ic_launcher.xml"] = adaptive
    # API23-25 accepts vector drawables. Night resources retain the same priority
    # as the adaptive v26+ pair, without density-specific raster copies.
    for theme, qualifier in [("light", ""), ("dark", "-night")]:
        vector = launcher_vector(theme)
        # The old opaque launcher filled the 64-unit source canvas. Preserve that
        # geometry on API23-25; only adaptive icons use the 66dp safe-circle inset.
        vector = vector.replace('android:scaleX="1.2" android:scaleY="1.2"',
                                'android:scaleX="1.6875" android:scaleY="1.6875"')
        vector = vector.replace('android:translateX="15.6" android:translateY="15.6"',
                                'android:translateX="0" android:translateY="0"')
        background = next(node for node in launcher_symbol(theme) if tag(node) == "rect").attrib["fill"]
        vector = vector.replace("    <group", f'    <path android:pathData="M0 0h108v108H0z" android:fillColor="{background}" />\n    <group', 1)
        outputs[ANDROID_RES / f"mipmap{qualifier}-anydpi/ic_launcher.xml"] = vector
    outputs.update(ios_launcher_outputs())
    return outputs


def ios_launcher_outlines(svg):
    """Expand the two canonical strokes; Icon Composer does not preserve their round caps.

    The C is the same r20 circular arc, bounded by radii 22.5/17.5 and r2.5 caps.
    Each vertical bar is the same width3 segment plus r1.5 endpoint caps.
    Refuse a changed source geometry rather than guessing a replacement silhouette.
    """
    paths = [node for node in svg.iter() if tag(node) == "path"]
    if [node.attrib["d"] for node in paths] != ["M42 15a20 20 0 1 0 0 34", "M27 27v10m7-16v22m7-17v12"]:
        raise ValueError("review changed canonical launcher strokes before outlining")
    if [node.attrib["stroke-width"] for node in paths] != ["5", "3"]:
        raise ValueError("review changed canonical launcher widths before outlining")
    dx = math.sqrt(20 ** 2 - 17 ** 2)
    outer_x, inner_x = 42 + dx / 8, 42 - dx / 8
    outline = (f"M{outer_x:.8f} 12.875 A22.5 22.5 0 1 0 {outer_x:.8f} 51.125 "
               f"A2.5 2.5 0 0 0 {inner_x:.8f} 46.875 "
               f"A17.5 17.5 0 1 1 {inner_x:.8f} 17.125 "
               f"A2.5 2.5 0 0 0 {outer_x:.8f} 12.875 Z")
    if any(node not in list(svg) for node in paths):
        raise ValueError("review changed canonical launcher stroke containers")
    group = svg
    for node in paths:
        group.remove(node)
    ET.SubElement(group, SVG_NAMESPACE + "path", {"d": outline, "fill": paths[0].attrib["stroke"]})
    for x, y, length in [(27, 27, 10), (34, 21, 22), (41, 26, 12)]:
        ET.SubElement(group, SVG_NAMESPACE + "path", {
            "d": f"M{x - 1.5} {y} a1.5 1.5 0 0 1 3 0 v{length} a1.5 1.5 0 0 1 -3 0 Z",
            "fill": paths[1].attrib["stroke"],
        })
    return svg


def ios_launcher_outputs():
    """SVG inputs only; Xcode owns native compiled renditions and compatibility PNGs."""
    outputs = {}
    fills = []
    for theme in ("light", "dark"):
        svg = launcher_symbol(theme)
        rect = next(node for node in svg if tag(node) == "rect")
        rgb = [int(rect.attrib["fill"][index:index + 2], 16) / 255 for index in (1, 3, 5)]
        fill = {"value": {"solid": "srgb:" + ",".join(f"{value:.5f}" for value in [*rgb, 1])}}
        if theme == "dark":
            fill["appearance"] = "dark"
        fills.append(fill)
        svg.remove(rect)
        svg = ios_launcher_outlines(svg)
        svg.set("width", "1024")
        svg.set("height", "1024")
        outputs[IOS_ICONS / f"Assets/cantos-{theme}.svg"] = ET.tostring(svg, encoding="unicode") + "\n"
    descriptor = {
        "fill-specializations": fills,
        "groups": [{
            "layers": [{"image-name": "cantos-light.svg", "image-name-specializations": [
                {"appearance": "dark", "value": "cantos-dark.svg"}], "name": "Cantos symbol", "glass": False}],
            "shadow": {"kind": "neutral", "opacity": 0},
            "translucency": {"enabled": False, "value": 0}, "specular": False,
        }],
        "supported-platforms": {"squares": ["iOS"]},
    }
    outputs[IOS_ICONS / "icon.json"] = json.dumps(descriptor, indent=2) + "\n"
    return outputs


def launcher_sources():
    manifest = checked_assets()
    return {f"svg/cantos-symbol{suffix}.svg": manifest["files"][f"svg/cantos-symbol{suffix}.svg"]["sha256"]
            for suffix in ("", "-dark", "-mono")}


def native_launchers(check):
    generated = {path: content.encode() for path, content in launcher_text_outputs().items()}
    expected = {
        "kit": "Cantos Logo Kit v2", "generator": "scripts/ui_logos.py --native-launcher",
        "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "sources": launcher_sources(),
        "native_adaptation": "OS-owned tile; canonical C/bar strokes unchanged. Android API23+ vector XML; API26+ adaptive XML; iOS Icon Composer SVG layers.",
        "android_foreground": {"canvas_dp": 108, "scale": LAUNCHER_SCALE, "translate_dp": LAUNCHER_OFFSET,
                               "safe_circle_diameter_dp": 66},
        "ios_compiler_boundary": "Owner permits compatibility PNGs generated by Xcode from the .icon/SVG source (#607). No source PNG or application-content exception. Xcode 26+ required; native/device appearance acceptance separate.",
        "outputs": {str(path.relative_to(ROOT)): hashlib.sha256(data).hexdigest()
                    for path, data in generated.items()},
    }
    if check:
        manifest = json.loads(LAUNCHER_PROVENANCE.read_text())
        if manifest["sources"] != launcher_sources():
            raise ValueError("launcher source provenance drift")
        if manifest != expected:
            raise ValueError("native launcher provenance drift")
        for path, data in generated.items():
            if path.read_bytes() != data:
                raise ValueError(f"native launcher resource drift: {path.relative_to(ROOT)}")
        print("PASS: native launcher SVG/XML sources, descriptors and output hashes agree")
        return
    for path, data in generated.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    LAUNCHER_PROVENANCE.write_text(json.dumps(expected, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--native-launcher", action="store_true")
    args = parser.parse_args()
    if args.native_launcher:
        native_launchers(args.check)
        return
    expected = render_kotlin()
    if args.check:
        if not CMP_OUTPUT.exists() or CMP_OUTPUT.read_text() != expected:
            raise SystemExit("FAIL: generated official logo drift")
        print("PASS: official logo checksums, vector geometry and transparent knockout mappings agree")
    else:
        CMP_OUTPUT.write_text(expected)


if __name__ == "__main__":
    main()
