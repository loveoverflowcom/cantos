#!/usr/bin/env python3
"""Generate shared semantic roles for Studio, Theatre Web and Theatre CMP."""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "contracts/design/cantos-tokens-0.2.0.json"
OUTPUT = ROOT / "apps/web/tokens.css"
THEATRE_OUTPUT = ROOT / "apps/theatre-web/tokens.css"
KOTLIN_OUTPUT = ROOT / "apps/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/design/GeneratedTokens.kt"


def variable(role):
    return "--cantos-" + re.sub(r"([a-z])([A-Z])", r"\1-\2", role).lower().replace(".", "-")


def render(raw):
    source = json.loads(raw)
    lines = [f"/* Generated from tokens v{source['version']} sha256:{hashlib.sha256(raw).hexdigest()}",
             "   by python3 scripts/studio_tokens.py. Do not edit. */",
             ":root {", "  color-scheme: light dark;"]
    for role, value in source["logical_dimensions"].items():
        if not isinstance(value, (int, float)) or value <= 0:
            raise ValueError("dimension must be positive")
        lines.append(f"  {variable(role)}: {value / 16:g}rem;")
    for role, value in source["themes"]["light"].items():
        if not re.fullmatch(r"#[0-9a-f]{6}", value):
            raise ValueError("unresolved or invalid color")
        lines.append(f"  {variable(role)}: {value};")
    lines += ["}", "@media (prefers-color-scheme: dark) {", "  :root {"]
    for role, value in source["themes"]["dark"].items():
        if not re.fullmatch(r"#[0-9a-f]{6}", value):
            raise ValueError("unresolved or invalid color")
        lines.append(f"    {variable(role)}: {value};")
    lines += ["  }", "}", ""]
    for theme in ("light", "dark"):
        lines.append(f'.studio-shell[data-theme="{theme}"], .theatre-shell[data-theme="{theme}"] {{')
        lines.append(f"  color-scheme: {theme};")
        for role, value in source["themes"][theme].items():
            lines.append(f"  {variable(role)}: {value};")
        lines += ["}", ""]
    return "\n".join(lines)


def render_kotlin(raw):
    source = json.loads(raw)
    render(raw)  # Reuse palette/dimension validation before producing either consumer.
    lines = [f"// Generated from tokens v{source['version']} sha256:{hashlib.sha256(raw).hexdigest()}",
             "// by python3 scripts/studio_tokens.py. Do not edit.",
             "package com.cantos.theatre.design", "",
             "import androidx.compose.ui.graphics.Color", "",
             "internal object GeneratedTokens {"]
    for theme, colors in source["themes"].items():
        lines.append(f"    object {theme.title()} {{")
        for role, value in colors.items():
            lines.append(f"        val {role.split('.')[-1]} = Color(0xff{value[1:]})")
        lines.append("    }")
    for role, value in source["logical_dimensions"].items():
        name = role.replace(".", "_")
        lines.append(f"    const val {name} = {float(value)}")
    return "\n".join(lines + ["}", ""])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    raw = SOURCE.read_bytes()
    outputs = {OUTPUT: render(raw), THEATRE_OUTPUT: render(raw), KOTLIN_OUTPUT: render_kotlin(raw)}
    if args.check:
        for path, expected in outputs.items():
            if path.read_text() != expected:
                raise SystemExit(f"FAIL: generated token drift: {path.relative_to(ROOT)}")
        print("PASS: token source, both CSS consumers and Kotlin agree")
    else:
        for path, expected in outputs.items():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(expected)


if __name__ == "__main__":
    main()
