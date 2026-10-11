#!/usr/bin/env python3
"""Generate the current Web token consumer; CMP has no runtime in this slice."""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "packages/design-tokens/studio-tokens-0.1.0.json"
OUTPUT = ROOT / "apps/cantos-studio/web/tokens.css"


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
        lines.append(f'.studio-shell[data-theme="{theme}"] {{')
        for role, value in source["themes"][theme].items():
            lines.append(f"  {variable(role)}: {value};")
        lines += ["}", ""]
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = render(SOURCE.read_bytes())
    if args.check:
        if OUTPUT.read_text() != expected:
            raise SystemExit("FAIL: generated token drift")
        print("PASS: token source and generated CSS agree")
    else:
        OUTPUT.write_text(expected)


if __name__ == "__main__":
    main()
