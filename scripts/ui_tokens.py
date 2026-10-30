#!/usr/bin/env python3
"""Validate canonical UI tokens and generate the Leptos and CMP mappings.

Generation is a pure transformation. The command validates every artifact before
staging per-file atomic replacements; it never writes a partly validated source.
Native dimensions stay logical dp/sp values; CSS units never cross renderers.
"""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import re
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "packages/ui-system/shared/tokens.json"
TEMPLATE = ROOT / "packages/ui-system/leptos/foundation.css.in"
OUTPUTS = {
    "css": ROOT / "packages/ui-system/leptos/tokens.css",
    "rust": ROOT / "packages/ui-system/leptos/src/tokens.rs",
    "foundation": ROOT / "packages/ui-system/leptos/foundation.css",
    "kotlin": ROOT / "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/generated/CantosTokens.kt",
}
VERSIONS = ("0.2.0", "0.2.1", "0.2.2", "0.3.0", "0.4.0", "0.5.0", "0.6.0", "0.7.0", "0.8.0", "0.9.0", "0.10.0", "0.11.0", "0.12.0", "0.13.0")
TYPES = {
    "color", "dimension", "number", "fontFamily", "fontWeight", "duration",
    "cubicBezier", "shadow",
}
FAMILIES = {
    "brand", "color", "type", "space", "shape", "elevation", "motion",
    "breakpoint", "size", "state",
}
REQUIRED_FAMILIES = FAMILIES - {"brand"}
# Uniform families allow semantic names at any depth; their type never changes.
# Mixed families below have only the documented v0.2.0 anatomy. A new subtype
# requires an explicit mapping here rather than an inferred CSS representation.
FAMILY_TYPES = {
    "color": "color", "brand": "color",
    "space": "dimension", "shape": "dimension", "size": "dimension",
    "breakpoint": "dimension", "elevation": "shadow",
}
TYPE_SUBTYPES = {"size": "dimension", "lineHeight": "number", "weight": "fontWeight"}
# A `cycle` is the period of a continuous loading animation. It is a duration, but under reduced
# motion the animation is stationary rather than zero-length, so it has no reduced-motion mapping.
MOTION_SUBTYPES = {"duration": "duration", "cycle": "duration", "easing": "cubicBezier"}
ROLE = re.compile(r"[a-z][a-zA-Z0-9]*(?:\.[a-z][a-zA-Z0-9]*)+")
COLOR = re.compile(r"#[0-9a-f]{6}")
TOP_LEVEL = {
    "version", "tokens", "themes", "aliases", "contrast_pairs",
    "reduced_motion", "density",
}
OPTIONAL_TOP_LEVEL = {"expressions"}
# An expression is an opt-in, renderer-neutral role remap for one consumer subtree (v0.7.0
# Theatre). Standard is the absence of an expression; it is never declared here.
EXPRESSION_NAME = re.compile(r"[a-z][a-z0-9]*")
EXPRESSION_TYPES = {"color", "fontWeight"}
REDUCED_ROLES = {
    "motion.feedback.duration", "motion.navigation.duration",
    "motion.emphasis.duration",
}
WEB_PREFIX = "--cantos-web-"  # renderer-local roles the Leptos template may declare itself
DEFAULT_SCOPE = ":where(.cantos-ui)"
SYSTEM_DARK_SCOPE = '.cantos-ui:not([data-theme="light"]):not([data-theme="dark"])'


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _reject_constant(value):
    raise ValueError(f"non-finite JSON number: {value}")


def load_source(raw):
    """Decode JSON without silently accepting duplicate keys or NaN/Infinity."""
    try:
        return json.loads(raw, object_pairs_hook=_unique_object,
                          parse_constant=_reject_constant)
    except (json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ValueError(f"invalid token JSON: {error}") from error


def variable(role):
    """Mechanically map a semantic role to its scoped CSS property name."""
    words = re.sub(r"([a-z0-9])([A-Z])", r"\1-\2", role)
    words = re.sub(r"([A-Z])([A-Z][a-z])", r"\1-\2", words)
    return "--cantos-" + words.replace(".", "-").lower()


def rust_variant(role):
    return "".join(word[0].upper() + word[1:]
                   for word in variable(role).removeprefix("--cantos-").split("-"))


def _finite(value):
    if type(value) not in (int, float):
        return False
    try:
        return math.isfinite(value)
    except OverflowError:
        return False


def _object(value, label, keys=None, optional=frozenset()):
    if not isinstance(value, dict):
        raise ValueError(f"{label}: expected an object")
    if keys is not None and not (keys <= set(value) <= keys | optional):
        raise ValueError(f"{label}: expected keys {', '.join(sorted(keys))}")


def _role(role):
    if not isinstance(role, str) or not ROLE.fullmatch(role):
        raise ValueError(f"invalid semantic role: {role!r}")
    if role.split(".")[0] not in FAMILIES:
        raise ValueError(f"unknown semantic family: {role}")


def _expected_type(role):
    parts = role.split(".")
    family = parts[0]
    if family in FAMILY_TYPES:
        return FAMILY_TYPES[family]
    if family == "type" and len(parts) == 3:
        if parts[1] == "family":
            return "fontFamily"
        if parts[2] in TYPE_SUBTYPES:
            return TYPE_SUBTYPES[parts[2]]
    if family == "motion" and len(parts) == 3 and parts[2] in MOTION_SUBTYPES:
        return MOTION_SUBTYPES[parts[2]]
    if (family == "state" and len(parts) == 2
            and parts[1].endswith("Opacity") and len(parts[1]) > len("Opacity")):
        return "number"
    raise ValueError(f"{role}: unrecognized semantic subtype")


def _color(value, label):
    if not isinstance(value, str) or not COLOR.fullmatch(value):
        raise ValueError(f"{label}: expected a resolved lowercase #rrggbb color")


def _font_family(value):
    if not isinstance(value, str) or len(value) > 256:
        return False
    # A bounded CSS family list, excluding escapes, comments and declarations.
    family = r'(?:"[A-Za-z][A-Za-z0-9 -]*"|\'[A-Za-z][A-Za-z0-9 -]*\'|[A-Za-z][A-Za-z0-9 -]*)'
    return re.fullmatch(rf"{family}(?: *, *{family})*", value) is not None


def _token(role, token):
    _role(role)
    _object(token, role, {"type", "value"})
    kind, value = token["type"], token["value"]
    if not isinstance(kind, str) or kind not in TYPES:
        raise ValueError(f"{role}: unknown token type")
    expected = _expected_type(role)
    if kind != expected:
        raise ValueError(f"{role}: expected {expected} type, got {kind}")

    if kind == "color":
        _color(value, role)
    elif kind in {"dimension", "number"}:
        if not _finite(value) or value < 0:
            raise ValueError(f"{role}: expected a finite nonnegative {kind}")
        if role.lower().endswith("opacity") and value > 1:
            raise ValueError(f"{role}: opacity must be in [0, 1]")
        if role.endswith(".lineHeight") and value <= 0:
            raise ValueError(f"{role}: line height must be positive")
        if role.startswith("breakpoint.") and value <= 0:
            raise ValueError(f"{role}: breakpoint must be positive")
    elif kind == "fontFamily":
        if not _font_family(value):
            raise ValueError(f"{role}: expected a safe CSS font family list")
    elif kind == "fontWeight":
        if type(value) is not int or not 100 <= value <= 900:
            raise ValueError(f"{role}: font weight must be an integer in [100, 900]")
    elif kind == "duration":
        if type(value) is not int or value < 0:
            raise ValueError(f"{role}: duration must be a nonnegative integer")
        if role.endswith(".cycle") and value == 0:
            raise ValueError(f"{role}: a continuous cycle must be positive")
    elif kind == "cubicBezier":
        if (not isinstance(value, list) or len(value) != 4
                or any(not _finite(number) or not 0 <= number <= 1 for number in value)):
            raise ValueError(f"{role}: cubic bezier must have four finite coordinates in [0, 1]")
    elif kind == "shadow":
        _object(value, role, {"x", "y", "blur", "spread", "color", "opacity"})
        for field in ("x", "y", "blur", "spread", "opacity"):
            if not _finite(value[field]):
                raise ValueError(f"{role}: shadow {field} must be finite")
        if value["blur"] < 0 or not 0 <= value["opacity"] <= 1:
            raise ValueError(f"{role}: shadow blur/opacity is outside its range")
        _color(value["color"], role)


def luminance(color):
    """WCAG relative luminance of an opaque sRGB color; no image compositing."""
    _color(color, "contrast")
    channels = [int(color[index:index + 2], 16) / 255 for index in (1, 3, 5)]
    linear = [channel / 12.92 if channel <= 0.04045
              else ((channel + 0.055) / 1.055) ** 2.4 for channel in channels]
    return sum(channel * weight
               for channel, weight in zip(linear, (0.2126, 0.7152, 0.0722)))


def contrast(first, second):
    darker, lighter = sorted((luminance(first), luminance(second)))
    return (lighter + 0.05) / (darker + 0.05)


def validate(source):
    """Refuse ambiguous mappings, unresolved values and unmet contrast floors."""
    _object(source, "source", TOP_LEVEL, OPTIONAL_TOP_LEVEL)
    if source["version"] not in VERSIONS:
        raise ValueError(f"unsupported token version: expected one of {sorted(VERSIONS)}")
    _object(source["tokens"], "tokens")
    _object(source["themes"], "themes", {"light", "dark"})
    for name, tokens in [("tokens", source["tokens"]), *source["themes"].items()]:
        _object(tokens, name)
        if not tokens:
            raise ValueError(f"{name}: token set must not be empty")
        for role, token in tokens.items():
            _token(role, token)
    light, dark = source["themes"]["light"], source["themes"]["dark"]
    if set(light) != set(dark):
        raise ValueError("themes: light and dark must have identical roles")
    if any(light[role]["type"] != dark[role]["type"] for role in light):
        raise ValueError("themes: light and dark token types must match")
    if set(source["tokens"]) & set(light):
        raise ValueError("tokens: shared and themed roles must not overlap")
    canonical = source["tokens"] | light
    if not REQUIRED_FAMILIES <= {role.split(".")[0] for role in canonical}:
        raise ValueError("source: missing required semantic token family")
    for role in ("breakpoint.medium", "breakpoint.expanded"):
        if role not in source["tokens"]:
            raise ValueError(f"source: missing shared {role}")
    if (source["tokens"]["breakpoint.medium"]["value"]
            >= source["tokens"]["breakpoint.expanded"]["value"]):
        raise ValueError("breakpoints: medium must precede expanded")

    _object(source["aliases"], "aliases")
    for role, target in source["aliases"].items():
        _role(role)
        if role in canonical:
            raise ValueError(f"{role}: alias overlaps a canonical role")
        if not isinstance(target, str) or target not in canonical:
            raise ValueError(f"{role}: alias must target a canonical role directly")
        expected = _expected_type(role)
        if canonical[target]["type"] != expected:
            raise ValueError(f"{role}: alias requires {expected} type, target {target} has {canonical[target]['type']}")
    roles = sorted(set(canonical) | set(source["aliases"]))
    for mapping, label in ((variable, "CSS"), (rust_variant, "Rust")):
        names = {}
        for role in roles:
            name = mapping(role)
            if name in names:
                raise ValueError(f"{label} name collision: {names[name]} and {role}")
            names[name] = role

    pairs = source["contrast_pairs"]
    if not isinstance(pairs, list) or not pairs:
        raise ValueError("contrast_pairs: expected at least one pair")
    seen_pairs = set()
    for pair in pairs:
        if not isinstance(pair, list) or len(pair) != 3:
            raise ValueError("contrast_pairs: expected [foreground, background, floor]")
        foreground, background, floor = pair
        if not _finite(floor) or floor not in (3, 4.5):
            raise ValueError("contrast_pairs: WCAG floor must be 3 or 4.5")
        for role in (foreground, background):
            if not isinstance(role, str) or role not in canonical or canonical[role]["type"] != "color":
                raise ValueError("contrast_pairs: pair must reference canonical color roles")
        if (foreground, background) in seen_pairs:
            raise ValueError("contrast_pairs: duplicate foreground/background pair")
        seen_pairs.add((foreground, background))
        for theme, tokens in source["themes"].items():
            colors = source["tokens"] | tokens
            ratio = contrast(colors[foreground]["value"], colors[background]["value"])
            if ratio < floor:
                raise ValueError(f"{theme}: {foreground} on {background} contrast {ratio:.3f} < {floor:g}")

    _validate_expressions(source, canonical, light)

    # Every shared duration needs an explicit reduced-motion mapping. This also
    # preserves the required historical intents while accepting scoped new ones.
    duration_roles = {role for role, token in source["tokens"].items()
                      if token["type"] == "duration" and not role.endswith(".cycle")}
    _object(source["reduced_motion"], "reduced_motion", REDUCED_ROLES | duration_roles)
    for role, value in source["reduced_motion"].items():
        if role not in source["tokens"] or source["tokens"][role]["type"] != "duration":
            raise ValueError(f"{role}: reduced motion requires a shared duration token")
        if role.endswith(".cycle"):
            raise ValueError(f"{role}: a continuous cycle is stationary under reduced motion, not zero")
        if type(value) is not int or value != 0:
            raise ValueError(f"{role}: reduced motion duration must be zero")
    _object(source["density"], "density", {"compact", "comfortable"})
    for density, overrides in source["density"].items():
        _object(overrides, f"density.{density}", {"size.control", "space.controlInline"})
        for role, value in overrides.items():
            if role not in source["tokens"] or source["tokens"][role]["type"] != "dimension":
                raise ValueError(f"{role}: density requires a shared dimension token")
            _token(role, {"type": "dimension", "value": value})
        if overrides["size.control"] < 44:
            raise ValueError(f"density.{density}: control hit area must be at least 44 logical units")
    return source


def _validate_expressions(source, canonical, themed):
    """An expression may only swap one role for a same-type, same-scope canonical role.

    Every contrast pair that the swap touches is re-measured in both themes, so an opt-in
    palette can never weaken a floor the standard roles already guarantee.
    """
    expressions = source.get("expressions", {})
    _object(expressions, "expressions")
    for name, mapping in expressions.items():
        if not isinstance(name, str) or not EXPRESSION_NAME.fullmatch(name) or name == "standard":
            raise ValueError(f"expressions: invalid expression name {name!r}")
        _object(mapping, f"expressions.{name}")
        if not mapping:
            raise ValueError(f"expressions.{name}: remap must not be empty")
        for role, target in mapping.items():
            _role(role)
            if not isinstance(target, str) or role not in canonical or target not in canonical:
                raise ValueError(f"expressions.{name}: {role} must map canonical role to canonical role")
            if role == target or target in mapping:
                raise ValueError(f"expressions.{name}: {role} must not chain or map to itself")
            kind = canonical[role]["type"]
            if kind != canonical[target]["type"] or kind not in EXPRESSION_TYPES:
                raise ValueError(f"expressions.{name}: {role} requires a same-type color or fontWeight target")
            if (role in themed) != (target in themed):
                raise ValueError(f"expressions.{name}: {role} and {target} must share theme scope")
        for foreground, background, floor in source["contrast_pairs"]:
            swapped = (mapping.get(foreground, foreground), mapping.get(background, background))
            if swapped == (foreground, background):
                continue
            for theme, tokens in source["themes"].items():
                colors = source["tokens"] | tokens
                ratio = contrast(colors[swapped[0]]["value"], colors[swapped[1]]["value"])
                if ratio < floor:
                    raise ValueError(f"{theme}: expression {name} {swapped[0]} on {swapped[1]} contrast {ratio:.3f} < {floor:g}")


def _number(value):
    return "0" if value == 0 else format(value, ".12g")


def css_value(token):
    kind, value = token["type"], token["value"]
    if kind == "dimension":
        return _number(value / 16) + "rem"
    if kind in {"number", "fontWeight"}:
        return _number(value)
    if kind == "duration":
        return f"{value}ms"
    if kind == "cubicBezier":
        return "cubic-bezier(" + ", ".join(_number(number) for number in value) + ")"
    if kind == "shadow":
        offsets = " ".join(_number(value[field] / 16) + "rem"
                           for field in ("x", "y", "blur", "spread"))
        channels = ", ".join(str(int(value["color"][index:index + 2], 16)) for index in (1, 3, 5))
        return f"{offsets} rgba({channels}, {_number(value['opacity'])})"
    return value


def _digest(source):
    canonical = json.dumps(source, sort_keys=True, separators=(",", ":"),
                           ensure_ascii=False, allow_nan=False).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def _header(source, suffix=""):
    return (f"Generated from Cantos UI tokens v{source['version']} sha256:{_digest(source)}{suffix}\n"
            "   by python3 scripts/ui_tokens.py. Do not edit.")


def _declarations(tokens, indent="  "):
    return [f"{indent}{variable(role)}: {css_value(tokens[role])};" for role in sorted(tokens)]


def _css(source):
    lines = ["/* " + _header(source) + " */", DEFAULT_SCOPE + " {", "  color-scheme: light;"]
    lines += _declarations(source["tokens"] | source["themes"]["light"])
    lines += [f"  {variable(role)}: var({variable(target)});"
              for role, target in sorted(source["aliases"].items())]
    lines += ["}", "", "@media (prefers-color-scheme: dark) {", f"  {SYSTEM_DARK_SCOPE} {{",
              "    color-scheme: dark;"]
    lines += _declarations(source["themes"]["dark"], "    ")
    lines += ["  }", "}", ""]
    for theme in ("light", "dark"):
        lines += [f'.cantos-ui[data-theme="{theme}"] {{', f"  color-scheme: {theme};"]
        lines += _declarations(source["themes"][theme])
        lines += ["}", ""]
    for density, overrides in sorted(source["density"].items()):
        lines.append(f'.cantos-ui[data-density="{density}"] {{')
        lines += _declarations({role: {"type": "dimension", "value": value}
                               for role, value in overrides.items()})
        lines += ["}", ""]
    for name, mapping in sorted(source.get("expressions", {}).items()):
        # Declared after every theme block. The second selector outranks the system-dark
        # block so the remap survives it; var() then resolves each target for the live theme.
        scope = f'.cantos-ui[data-expression="{name}"]'
        lines += [f"{scope},", f'{scope}:not([data-theme="light"]):not([data-theme="dark"]) {{']
        lines += [f"  {variable(role)}: var({variable(target)});" for role, target in sorted(mapping.items())]
        lines += ["}", ""]
    lines += ["@media (prefers-reduced-motion: reduce) {", f"  {DEFAULT_SCOPE},",
              '  .cantos-ui[data-theme="light"],', '  .cantos-ui[data-theme="dark"],',
              f"  {SYSTEM_DARK_SCOPE} {{"]
    lines += [f"    {variable(role)}: 0ms;" for role in sorted(source["reduced_motion"])]
    lines += ["  }", "}", ""]
    return "\n".join(lines)


def _rust(source):
    canonical = source["tokens"] | source["themes"]["light"]
    roles = sorted(set(canonical) | set(source["aliases"]))
    lines = ["// " + _header(source).replace("\n   ", "\n// "), "",
             f'pub const VERSION: &str = "{source["version"]}";', "",
             "/// Canonical roles and migration aliases from the shared source.",
             "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]", "pub enum Token {"]
    lines += [f"    {rust_variant(role)}," for role in roles]
    names = ", ".join(json.dumps(name) for name in sorted(source.get("expressions", {})))
    lines += ["}", "",
              "/// Opt-in role remaps (`data-expression`); Standard is no expression.",
              f"pub const EXPRESSIONS: &[&str] = &[{names}];", "",
              "pub const ALL: &[Token] = &["]
    lines += [f"    Token::{rust_variant(role)}," for role in roles]
    lines += ["];", "", "impl Token {", "    pub const ALL: &'static [Self] = ALL;", "",
              "    pub const fn css_variable(self) -> &'static str {", "        match self {"]
    for role in roles:
        arm = f'            Self::{rust_variant(role)} => "{variable(role)}",'
        # Preserve the pinned rustfmt layout when expressive role names exceed its width.
        if len(arm) > 100:
            lines += [f"            Self::{rust_variant(role)} => {{",
                      f'                "{variable(role)}"', "            }"]
        else:
            lines.append(arm)
    lines += ["        }", "    }", "",
              "    /// Static lookup: System resolves to Light; CSS follows the system preference.",
              "    /// Density and reduced motion are CSS scope overrides, absent from this lookup.",
              "    pub const fn css_value(self, theme: crate::Theme) -> &'static str {",
              "        match self {"]
    for role in roles:
        variant = rust_variant(role)
        if role in source["aliases"]:
            target = rust_variant(source["aliases"][role])
            lines.append(f"            Self::{variant} => Self::{target}.css_value(theme),")
        elif role in source["tokens"]:
            value = json.dumps(css_value(source["tokens"][role]))
            lines.append(f"            Self::{variant} => {value},")
        else:
            lines.append(f"            Self::{variant} => match theme {{")
            dark = json.dumps(css_value(source["themes"]["dark"][role]))
            light = json.dumps(css_value(source["themes"]["light"][role]))
            lines += [f"                crate::Theme::Dark => {dark},",
                      f"                crate::Theme::Light | crate::Theme::System => {light},", "            },"]
    lines += ["        }", "    }", "}", ""]
    return "\n".join(lines)


def _foundation(source, template):
    if not isinstance(template, str):
        raise ValueError("foundation template must be UTF-8 text")

    def replace(match):
        role = match.group(1).strip()
        if not role.startswith("breakpoint.") or role not in source["tokens"]:
            raise ValueError(f"foundation template: unknown breakpoint placeholder {role}")
        return css_value(source["tokens"][role])

    rendered = re.sub(r"\{\{([^{}]+)\}\}", replace, template)
    if "{{" in rendered or "}}" in rendered:
        raise ValueError("foundation template: malformed placeholder")
    canonical = {variable(role) for role in source["tokens"] | source["themes"]["light"] | source["aliases"]}
    declared = set(re.findall(r"(?<![\w-])(--cantos-[a-z0-9-]+)\s*:", rendered))
    shadowing = {name for name in declared if not name.startswith(WEB_PREFIX) or name in canonical}
    if shadowing:
        raise ValueError(f"foundation template: may declare only new {WEB_PREFIX}* roles, not {', '.join(sorted(shadowing))}")
    defined = canonical | declared
    used = set(re.findall(r"var\(\s*(--cantos-[a-z0-9-]+)", rendered))
    if not used <= defined:
        raise ValueError(f"foundation template: undefined CSS roles {', '.join(sorted(used - defined))}")
    template_digest = hashlib.sha256(template.encode("utf-8")).hexdigest()
    header = _header(source, f" template-sha256:{template_digest}")
    return "/* " + header + " */\n" + rendered.rstrip() + "\n"


def kotlin_string(value):
    """Quote data, including Kotlin's interpolation marker, without executable content."""
    return json.dumps(str(value), ensure_ascii=False).replace("$", "\\$")


def logical_value(token):
    """Native token inspection preserves logical numbers, never rem or CSS shadows."""
    value = token["value"]
    if isinstance(value, (dict, list)):
        return json.dumps(value, sort_keys=True, separators=(",", ":"))
    return _number(value) if type(value) in (int, float) else str(value)


def _kotlin(source):
    canonical = source["tokens"] | source["themes"]["light"]
    roles = sorted(set(canonical) | set(source["aliases"]))
    lines = ["// " + _header(source).replace("\n   ", "\n// "),
             "package com.cantos.ui.generated", "",
             "data class TokenValue(", "    val name: String, val kind: String,",
             "    val light: String, val dark: String,",
             "    val compact: String?, val comfortable: String?, val aliasOf: String?", ")", "",
             "object CantosTokens {", f'    const val version: String = "{source["version"]}"',
             "    val entries: List<TokenValue> = listOf("]
    for role in roles:
        target = source["aliases"].get(role, role)
        light = source["tokens"].get(target, source["themes"]["light"].get(target))
        dark = source["tokens"].get(target, source["themes"]["dark"].get(target))
        values = [role, light["type"], logical_value(light), logical_value(dark)]
        args = [kotlin_string(value) for value in values]
        for density in ("compact", "comfortable"):
            value = source["density"][density].get(target)
            args.append(kotlin_string(_number(value)) if value is not None else "null")
        args.append(kotlin_string(target) if target != role else "null")
        lines.append("        TokenValue(" + ", ".join(args) + "),")
    reduced = ", ".join(kotlin_string(name) for name in sorted(source["reduced_motion"]))
    expressions = source.get("expressions", {})
    remaps = ", ".join(
        kotlin_string(name) + " to mapOf(" + ", ".join(
            kotlin_string(role) + " to " + kotlin_string(target)
            for role, target in sorted(mapping.items())) + ")"
        for name, mapping in sorted(expressions.items()))
    names = ", ".join(kotlin_string(name) for name in ["standard", *sorted(expressions)])
    lines += ["    )", "    val names: List<String> = entries.map { it.name }",
              "    private val byName = entries.associateBy { it.name }",
              f"    private val reducedRoles = setOf({reduced})",
              "    /** Opt-in role remaps; standard is the absence of an expression and is never remapped. */",
              f"    val expressionNames: List<String> = listOf({names})",
              f"    private val expressionRoles: Map<String, Map<String, String>> = mapOf({remaps})", "",
              "    /** The canonical role a consumer of [expression] reads for [name]; unmapped roles are unchanged. */",
              "    fun role(name: String, expression: String = \"standard\"): String {",
              "        require(expression in expressionNames) { \"Unknown expression: $expression\" }",
              "        return expressionRoles[expression]?.get(name) ?: name",
              "    }", "",
              "    fun raw(name: String, theme: String = \"light\", density: String = \"compact\"): String {",
              "        require(theme == \"light\" || theme == \"dark\") { \"Unknown theme: $theme\" }",
              "        require(density == \"compact\" || density == \"comfortable\") { \"Unknown density: $density\" }",
              "        val token = requireNotNull(byName[name]) { \"Unknown token: $name\" }",
              "        val override = if (density == \"comfortable\") token.comfortable else token.compact",
              "        return override ?: if (theme == \"dark\") token.dark else token.light", "    }", "",
              "    fun color(name: String, theme: String): String {",
              "        require(byName[name]?.kind == \"color\") { \"Not a color token: $name\" }",
              "        return raw(name, theme)", "    }", "",
              "    fun number(name: String, density: String = \"compact\", reducedMotion: Boolean = false): Double {",
              "        require(byName[name]?.kind in setOf(\"dimension\", \"number\", \"fontWeight\", \"duration\")) {",
              "            \"Not a numeric token: $name\"", "        }",
              "        return if (reducedMotion && name in reducedRoles) 0.0 else raw(name, density = density).toDouble()",
              "    }", "", "    fun text(name: String): String = raw(name)", "}", ""]
    return "\n".join(lines)


def outputs(raw, template, output_root=ROOT):
    """Return every generated path/content after validating the whole source."""
    source = validate(load_source(raw))
    contents = {"css": _css(source), "rust": _rust(source),
                "foundation": _foundation(source, template), "kotlin": _kotlin(source)}
    return {Path(output_root) / path.relative_to(ROOT): contents[name]
            for name, path in OUTPUTS.items()}


def check_outputs(expected):
    """Return every missing/drifted output without changing the filesystem."""
    failures = []
    for path, content in expected.items():
        if not path.is_file():
            failures.append(f"missing: {path}")
        elif path.read_text(encoding="utf-8") != content:
            failures.append(f"drift: {path}")
    return failures


def write_outputs(expected):
    """Stage all outputs first, then replace each file atomically."""
    staged = []
    try:
        for path, content in expected.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", newline="\n",
                                             prefix=f".{path.name}.", dir=path.parent,
                                             delete=False) as temporary:
                staged.append((Path(temporary.name), path))
                temporary.write(content)
        for temporary, path in staged:
            os.replace(temporary, path)
    finally:
        for temporary, _ in staged:
            temporary.unlink(missing_ok=True)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--source", type=Path, default=SOURCE)
    parser.add_argument("--template", type=Path, default=TEMPLATE)
    parser.add_argument("--output-root", type=Path, default=ROOT)
    args = parser.parse_args(argv)
    try:
        expected = outputs(args.source.read_bytes(), args.template.read_text(encoding="utf-8"),
                           args.output_root)
        if args.check:
            failures = check_outputs(expected)
            if failures:
                print("FAIL: generated UI token drift\n" + "\n".join(failures), file=sys.stderr)
                return 1
            print("PASS: canonical UI tokens and all generated mappings agree")
        else:
            write_outputs(expected)
            print(f"PASS: generated {len(expected)} UI system artifacts")
    except (ValueError, OSError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
