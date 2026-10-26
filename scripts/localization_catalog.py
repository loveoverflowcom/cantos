#!/usr/bin/env python3
"""Pre-ADR localization experiment; never imported by a shipping application.

Validate the entire fixture and project typed, dependency-free Rust/Kotlin code.
This deliberately supports only text, integer cardinal one/other and closed select.
It is not an ICU runtime, BCP-47 registry, or locale-aware formatter.
"""
import argparse
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
from string import Formatter

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = Path("docs/evidence/fixtures/localization-foundation")
TAG = re.compile(r"[a-z]{2,3}(?:-[A-Z][a-z]{3})?(?:-(?:[A-Z]{2}|[0-9]{3}))?\Z")
IDENTIFIER = re.compile(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)*\Z")
KEY = re.compile(r"(?:common|studio|theatre)(?:\.[a-z][a-z0-9]*(?:_[a-z0-9]+)*)+\Z")


class CatalogError(ValueError):
    """Invalid developer-authored catalog or explicit lookup/argument refusal."""


def require(condition, code):
    if not condition:
        raise CatalogError(code)


def exact_fields(value, fields, location):
    require(isinstance(value, dict) and set(value) == set(fields), f"{location}: fields")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate key: {key}")
        result[key] = value
    return result


def load(path):
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)


def segments(template):
    """Parse only original source; inserted values never enter this parser."""
    require(isinstance(template, str) and template.strip(), "blank translation")
    result = []
    try:
        for literal, name, spec, conversion in Formatter().parse(template):
            require(name is None or (IDENTIFIER.fullmatch(name) and not spec and conversion is None),
                    "invalid placeholder")
            if literal:
                result.append(("text", literal))
            if name is not None:
                result.append(("arg", name))
    except ValueError as error:
        raise CatalogError("invalid placeholder") from error
    return result


def templates(pattern, args, locale):
    require(isinstance(pattern, dict), "pattern: fields")
    if set(pattern) == {"text"}:
        return [pattern["text"]], None
    kind = "plural" if "plural" in pattern else "select"
    exact_fields(pattern, {kind, "forms"}, "pattern")
    selector = pattern[kind]
    require(isinstance(selector, str) and selector in args, "unknown selector")
    if kind == "plural":
        require(args[selector] == "count", "plural requires count")
        categories = {"other"} | ({"one"} if locale["integerOne"] else set())
    else:
        require(isinstance(args[selector], list), "select requires closed enum")
        categories = set(args[selector])
    require(isinstance(pattern["forms"], dict) and set(pattern["forms"]) == categories,
            f"{kind} coverage")
    return list(pattern["forms"].values()), selector if kind == "select" else None


def validate(catalog):
    exact_fields(catalog, {"version", "status", "fallback", "cldr", "locales", "messages"}, "catalog")
    require(type(catalog["version"]) is int and catalog["version"] == 1, "catalog version")
    require(catalog["status"] == "proposal-fixture", "catalog status")
    require(catalog["cldr"] == "48/integer-cardinal-subset", "CLDR subset version")
    locales = catalog["locales"]
    require(isinstance(locales, list) and locales, "locale registry empty")
    tags, accepted = set(), set()
    for locale in locales:
        exact_fields(locale, {"tag", "autonym", "aliases", "testOnly", "integerOne"}, "locale")
        tag = locale["tag"]
        require(isinstance(tag, str) and TAG.fullmatch(tag), "noncanonical UI tag")
        require(tag not in tags, "duplicate UI tag")
        tags.add(tag)
        require(isinstance(locale["autonym"], str) and locale["autonym"].strip(), "blank autonym")
        require(type(locale["testOnly"]) is bool, "testOnly requires boolean")
        require(isinstance(locale["aliases"], list), "aliases require list")
        for alias in [tag, *locale["aliases"]]:
            require(isinstance(alias, str) and TAG.fullmatch(alias), "noncanonical UI alias")
            require(alias.lower() not in accepted, "ambiguous UI alias")
            accepted.add(alias.lower())
        one = locale["integerOne"]
        require(isinstance(one, list) and all(type(n) is int and 0 <= n <= 2**64 - 1 for n in one),
                "invalid integer plural rule")
        require(len(one) == len(set(one)), "duplicate integer plural operand")
    require(any(locale["tag"] == catalog["fallback"] and not locale["testOnly"] for locale in locales),
            "fallback must be a supported non-test UI locale")
    messages = catalog["messages"]
    require(isinstance(messages, dict) and messages, "messages empty")
    generated_names = set()
    generated_enums = set()
    for key, message in messages.items():
        require(KEY.fullmatch(key), f"invalid semantic key: {key}")
        name = symbol(key)
        require(name not in generated_names, "generated symbol collision")
        generated_names.add(name)
        exact_fields(message, {"args", "translations"}, key)
        args = message["args"]
        require(isinstance(args, dict), f"{key}: args")
        for arg, kind in args.items():
            # Prefix fields in generated code to avoid Rust/Kotlin keyword collisions.
            require(IDENTIFIER.fullmatch(arg), f"{key}: invalid argument")
            require(kind in ("text", "count") or (isinstance(kind, list) and kind
                    and all(isinstance(value, str) and IDENTIFIER.fullmatch(value) for value in kind)
                    and len(kind) == len(set(kind))), f"{key}: invalid argument type")
            if isinstance(kind, list):
                members = [symbol(value) for value in kind]
                require("Self" not in members and len(members) == len(set(members)),
                        f"{key}: generated enum collision")
                enum_name = name + symbol(arg)
                require(enum_name not in generated_enums, f"{key}: generated enum type collision")
                generated_enums.add(enum_name)
        translations = message["translations"]
        require(isinstance(translations, dict) and set(translations) == tags, f"{key}: locale/key coverage")
        signatures, parameters = set(), []
        for locale in locales:
            pattern = translations[locale["tag"]]
            values, selector = templates(pattern, args, locale)
            enum_args = {arg for arg, kind in args.items() if isinstance(kind, list)}
            require(enum_args == ({selector} if selector else set()), f"{key}: enum requires select")
            signatures.add((next(iter(sorted(set(pattern) - {"forms"}))), pattern.get("plural", pattern.get("select"))))
            for value in values:
                names = Counter(text for kind, text in segments(value) if kind == "arg")
                require(set(names) == set(args) - ({selector} if selector else set()), f"{key}: argument coverage")
                parameters.append(names)
        require(len(signatures) == 1, f"{key}: selector drift")
        require(all(names == parameters[0] for names in parameters), f"{key}: placeholder multiplicity")
    return catalog


@dataclass(frozen=True)
class Resolution:
    requested: str
    tag: str
    exact: bool


def resolve(catalog, requested, include_test=False):
    # Bounded RFC 4647-style lookup, not validation/canonicalization of content tags.
    require(isinstance(requested, str) and re.fullmatch(r"[A-Za-z]{2,8}(?:-[A-Za-z0-9]{1,8})*", requested),
            "invalid UI locale request")
    candidate = requested.lower()
    while candidate:
        for locale in catalog["locales"]:
            if locale["testOnly"] and not include_test:
                continue
            if candidate in [tag.lower() for tag in [locale["tag"], *locale["aliases"]]]:
                return Resolution(requested, locale["tag"], requested.lower() == locale["tag"].lower())
        parts = candidate.split("-")[:-1]
        if parts and len(parts[-1]) == 1:
            parts.pop()
        candidate = "-".join(parts)
    raise CatalogError(f"unsupported UI locale: {requested}; explicit fallback: {catalog['fallback']}")


def render(catalog, requested, key, args, include_test=False):
    resolution = resolve(catalog, requested, include_test)
    require(key in catalog["messages"], f"missing message: {key}")
    message = catalog["messages"][key]
    require(isinstance(args, dict) and set(args) == set(message["args"]), "named arguments mismatch")
    for name, kind in message["args"].items():
        value = args[name]
        valid = (isinstance(value, str) if kind == "text" else
                 type(value) is int and 0 <= value <= 2**64 - 1 if kind == "count" else value in kind)
        require(valid, f"argument type: {name}")
    pattern = message["translations"][resolution.tag]
    if "text" in pattern:
        template = pattern["text"]
    elif "plural" in pattern:
        locale = next(item for item in catalog["locales"] if item["tag"] == resolution.tag)
        category = "one" if args[pattern["plural"]] in locale["integerOne"] else "other"
        template = pattern["forms"][category]
    else:
        template = pattern["forms"][args[pattern["select"]]]
    return "".join(value if kind == "text" else str(args[value]) for kind, value in segments(template))


def symbol(key):
    return "".join(part[0].upper() + part[1:] for part in re.split(r"[._]", key))


def quoted(value, kotlin=False):
    escapes = {"\\": "\\\\", '"': '\\"', "\n": "\\n", "\r": "\\r", "\t": "\\t"}
    if kotlin:
        escapes["$"] = "\\$"
    result = []
    for char in value:
        if char in escapes:
            result.append(escapes[char])
        elif ord(char) < 32:
            result.append(f"\\u{ord(char):04x}" if kotlin else f"\\u{{{ord(char):x}}}")
        else:
            result.append(char)
    return '"' + "".join(result) + '"'


def expression(template, kotlin):
    pieces = []
    for kind, value in segments(template):
        if kind == "text":
            pieces.append(quoted(value, kotlin))
        elif kotlin:
            pieces.append(f"message.arg_{value}.toString()")
        else:
            pieces.append(f"arg_{value}.to_string()")
    if kotlin:
        return " + ".join(pieces) if pieces else '""'
    return "[" + ", ".join(piece + ".to_string()" if piece.startswith('"') else piece for piece in pieces) + "].concat()"


def projections(catalog):
    """Generate an isolated typed fixture, not application resource/runtime adoption."""
    outputs = {}
    digest = hashlib.sha256(json.dumps(catalog, sort_keys=True, ensure_ascii=False).encode()).hexdigest()
    for kotlin in (False, True):
        lines = [f"// Generated proposal fixture; DO NOT EDIT. catalog sha256: {digest}"]
        if kotlin:
            lines += ["package cantos.localization.fixture", "", "enum class Locale { " + ", ".join(f"L{n}" for n in range(len(catalog["locales"]))) + " }", "", "sealed interface Message {"]
        else:
            lines += ["#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "pub enum Locale { " + ", ".join(f"L{n}" for n in range(len(catalog["locales"]))) + " }", "", "#[derive(Clone, Debug)]", "pub enum Message {"]
        enums = []
        for key, message in sorted(catalog["messages"].items()):
            name, fields = symbol(key), []
            for arg, kind in sorted(message["args"].items()):
                if isinstance(kind, list):
                    typ = name + symbol(arg)
                    enums.append((typ, kind))
                else:
                    typ = ("String" if kind == "text" else "ULong") if kotlin else ("String" if kind == "text" else "u64")
                fields.append(f"val arg_{arg}: {typ}" if kotlin else f"arg_{arg}: {typ}")
            if kotlin:
                lines.append(f"    data class {name}({', '.join(fields)}) : Message" if fields else f"    data object {name} : Message")
            else:
                lines.append(f"    {name} {{ {', '.join(fields)} }}," if fields else f"    {name},")
        lines += ["}", ""]
        for name, values in enums:
            members = ", ".join(symbol(value) for value in values)
            lines += [f"enum class {name} {{ {members} }}" if kotlin else f"#[derive(Clone, Copy, Debug)]\npub enum {name} {{ {members} }}", ""]
        if kotlin:
            lines += ["fun tag(locale: Locale): String = when (locale) {"]
        else:
            lines += ["pub fn tag(locale: Locale) -> &'static str {", "    match locale {"]
        for n, locale in enumerate(catalog["locales"]):
            lines.append(f"    Locale.L{n} -> {quoted(locale['tag'], True)}" if kotlin else f"        Locale::L{n} => {quoted(locale['tag'])},")
        lines += ["}", ""] if kotlin else ["    }", "}", ""]
        # Lookup/persistence/formatting adapters deliberately stay outside this prototype.
        lines += ["fun render(locale: Locale, message: Message): String = when (message) {"] if kotlin else ["pub fn render(locale: Locale, message: &Message) -> String {", "    match message {"]
        for key, message in sorted(catalog["messages"].items()):
            name = symbol(key)
            fields = ", ".join("arg_" + arg for arg in sorted(message["args"]))
            if kotlin:
                lines.append(f"    {'is ' if fields else ''}Message.{name} -> when (locale) {{")
            else:
                lines.append(f"        Message::{name}{' { ' + fields + ' }' if fields else ''} => match locale {{")
            for n, locale in enumerate(catalog["locales"]):
                pattern = message["translations"][locale["tag"]]
                prefix = f"        Locale.L{n} -> " if kotlin else f"            Locale::L{n} => "
                suffix = "" if kotlin else ","
                if "text" in pattern:
                    lines.append(prefix + expression(pattern["text"], kotlin) + suffix)
                elif "plural" in pattern:
                    arg = pattern["plural"]
                    other = expression(pattern["forms"]["other"], kotlin)
                    if not locale["integerOne"]:
                        lines.append(prefix + other + suffix)
                    else:
                        condition = " || ".join(f"message.arg_{arg} == {n}uL" if kotlin else f"*arg_{arg} == {n}" for n in locale["integerOne"])
                        one = expression(pattern["forms"]["one"], kotlin)
                        lines.append(prefix + (f"if ({condition}) {one} else {other}" if kotlin else f"if {condition} {{ {one} }} else {{ {other} }}") + suffix)
                else:
                    arg = pattern["select"]
                    typ = name + symbol(arg)
                    lines.append(prefix + (f"when (message.arg_{arg}) {{" if kotlin else f"match arg_{arg} {{"))
                    for value, template in sorted(pattern["forms"].items()):
                        case = f"{typ}.{symbol(value)} -> " if kotlin else f"{typ}::{symbol(value)} => "
                        lines.append("                " + case + expression(template, kotlin) + suffix)
                    lines.append("            }" + suffix)
            lines.append("    }" if kotlin else "        },")
        lines += ["}"] if kotlin else ["    }", "}"]
        outputs["Catalog.kt" if kotlin else "catalog.rs"] = "\n".join(lines) + "\n"
    manifest = {"version": 1, "status": catalog["status"], "catalogSha256": digest,
                "fallback": catalog["fallback"], "locales": catalog["locales"],
                "messages": {key: message["args"] for key, message in sorted(catalog["messages"].items())}}
    outputs["manifest.json"] = json.dumps(manifest, sort_keys=True, ensure_ascii=False, indent=2) + "\n"
    return outputs


def repository_failures(root=ROOT):
    try:
        catalog = validate(load(root / FIXTURE / "catalog.json"))
        expected = projections(catalog)
        directory = root / FIXTURE / "generated"
        errors = []
        vectors = load(root / FIXTURE / "vectors.json")
        require(isinstance(vectors, list) and vectors, "fixture vectors empty")
        for row in vectors:
            require(isinstance(row, dict) and {"locale", "key", "args", "expected"} <= set(row)
                    and set(row) <= {"locale", "key", "args", "expected", "includeTest"}, "vector fields")
            require(type(row.get("includeTest", False)) is bool, "vector includeTest")
            actual = render(catalog, row["locale"], row["key"], row["args"], row.get("includeTest", False))
            require(actual == row["expected"], f"vector output: {row['key']}/{row['locale']}")
        if not directory.is_dir() or {path.name for path in directory.iterdir()} != set(expected):
            errors.append("localization fixture: generated inventory drift")
        for name, source in expected.items():
            path = directory / name
            if not path.is_file() or path.read_text(encoding="utf-8") != source:
                errors.append(f"localization fixture: generated drift: {name}")
        return errors
    except (ValueError, OSError) as error:
        return [f"localization fixture: {error}"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="check whole fixture and generated drift without writes")
    args = parser.parse_args()
    if args.check:
        errors = repository_failures()
        for error in errors:
            print(error)
        return bool(errors)
    catalog = validate(load(ROOT / FIXTURE / "catalog.json"))
    directory = ROOT / FIXTURE / "generated"
    directory.mkdir(exist_ok=True)
    for name, source in projections(catalog).items():
        (directory / name).write_text(source, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
