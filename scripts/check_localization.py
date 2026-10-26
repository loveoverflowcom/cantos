#!/usr/bin/env python3
"""Bounded localization regression check, used by the existing repository gate.

Only added lines in the five UI scopes are linted for displayed literals, locale
maps and raw error sinks. Changed bilingual catalogs are checked as whole files.
The independent pre-ADR semantic fixture is also validated in full, including
its resource-derived locale registry and deterministic Rust/Kotlin projections.
Accepted Web resource projections are validated byte-for-byte by the whole-resource guard.
Legacy CMP pair-shaped owners retain their existing checks until #507.
This is a token/context check, not Rust/Kotlin type checking or runtime evidence:
indirect data flow, arbitrary wrapper components and mapping completeness still
need exhaustive compiler matches and renderer/resource tests. No dependencies.
"""
from collections import Counter
from dataclasses import dataclass
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import re
import subprocess
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
POLICY = ROOT / "scripts/localization_allowlist.json"
SCOPES = (
    "apps/cantos-theatre/web/", "apps/cantos-studio/web/",
    "apps/cantos-theatre/mobile/", "packages/ui-system/leptos/", "packages/ui-system/cmp/",
)
SUFFIXES = {".rs", ".kt", ".html", ".xml"}
VISIBLE_ATTRIBUTES = {
    "label", "title", "placeholder", "tooltip", "alt", "description", "help", "hint",
    "aria-label", "aria-description", "aria-valuetext", "contentDescription", "stateDescription",
    "error", "errorMessage", "supportingText", "disabledReason", "accessibleName", "name",
    "accessible_name", "disabled_reason", "error_message", "supporting_text",
    "expandedDescription", "collapsedDescription", "emptyDescription", "closeLabel", "paneTitle",
}
# Actual shared CMP public APIs and native text sinks. Positional content/model
# values are deliberately absent; named label/a11y fields are checked separately.
UI_POSITIONS = {
    **{name: {0} for name in (
        "Text", "BasicText", "AnnotatedString", "AccessibleName", "DisabledReason",
        "Button", "TextField", "ControlledTextField", "NumberField", "TextArea",
        "Checkbox", "Radio", "Switch", "Select", "FilterChip", "FieldHelp", "Tooltip",
    )},
    "Dialog": {1, 2}, "SelectOption": {1},
}
UI_CALLS = set(UI_POSITIONS)
TOKEN = re.compile(
    r'(?P<comment>//[^\n]*|/\*[\s\S]*?\*/)'
    r'|(?P<string>r(?P<hash>\#*)"[\s\S]*?"(?P=hash)|"""[\s\S]*?"""|"(?:\\.|[^"\\])*")'
    r"|(?P<char>'(?:\\.|[^'\\])')|(?P<word>[A-Za-z_][\w-]*)|(?P<symbol>[^\s])",
    re.UNICODE,
)
PLACEHOLDER = re.compile(r"(?<!\{)\{([A-Za-z_][A-Za-z_0-9]*)\}(?!\})")
RAW_ERROR = re.compile(
    r"\b(?:error|err|failure|api_error|apiError|http_error|httpError)\s*\.\s*"
    r"(?:to_string\s*\(\s*\)|toString\s*\(\s*\)|message\b|body\b|responseText\b)"
)


@dataclass(frozen=True)
class Token:
    kind: str
    value: str
    start: int
    end: int
    line: int
    last_line: int | None = None


def string_value(value):
    if value.startswith('r'):
        return value[value.index('"') + 1:value.rindex('"')]
    if value.startswith('"""'):
        return value[3:-3]
    try:
        return json.loads(value)
    except ValueError:
        # Rust-specific escapes are still compared consistently on both sides.
        return value[1:-1]


def tokenize(source):
    tokens = []
    masked = list(source)
    for match in TOKEN.finditer(source):
        kind = match.lastgroup
        # The named raw-string delimiter is an internal regex group.
        if match.group("string") is not None:
            kind = "string"
        value = match.group()
        if kind in {"comment", "string", "char"}:
            masked[match.start():match.end()] = ["\n" if char == "\n" else " " for char in value]
        if kind != "comment":
            tokens.append(Token(kind, string_value(value) if kind == "string" else value,
                                match.start(), match.end(), source.count("\n", 0, match.start()) + 1,
                                source.count("\n", 0, match.end()) + 1))
    return tokens, "".join(masked)


def empty_policy():
    return {"owners": [], "exceptions": []}


def validate_policy(policy):
    errors = []
    for group in ("owners", "exceptions"):
        if not isinstance(policy.get(group), list):
            errors.append(f"localization allow-list: {group} must be a list")
            continue
        seen = set()
        for entry in policy[group]:
            required = {"path", "reason"} | ({"literal", "context"} if group == "exceptions" else {"mode"})
            if not isinstance(entry, dict) or any(not isinstance(entry.get(key), str) or not entry[key].strip() for key in required):
                errors.append(f"localization allow-list: {group} entry needs nonblank {sorted(required)}")
                continue
            if not in_scope(entry["path"]) or "*" in entry["path"] or ".." in Path(entry["path"]).parts:
                errors.append(f"localization allow-list: exact UI path required: {entry['path']}")
            if group == "owners" and entry["mode"] not in {"bilingual", "fixed-english-tool", "generated"}:
                errors.append(f"localization allow-list: invalid owner mode: {entry['path']}")
            if group == "owners" and entry["mode"] == "generated":
                from localization_resources import TARGETS
                if Path(entry["path"]) not in TARGETS.values():
                    errors.append(f"localization allow-list: generated owner has no drift guard: {entry['path']}")
            identity = (entry["path"], entry.get("literal"), entry.get("context"))
            if identity in seen:
                errors.append(f"localization allow-list: duplicate {group} entry: {entry['path']}")
            seen.add(identity)
    return errors


def exempt(path, token, source, policy):
    context = source.splitlines()[token.line - 1].strip()
    return any(item.get("path") == path and item.get("literal") == token.value
               and item.get("context") == context and item.get("reason", "").strip()
               for item in policy["exceptions"])


def in_scope(path):
    return path.startswith(SCOPES) and Path(path).suffix in SUFFIXES


def closing(tokens, start, opening="{", closing="}"):
    depth = 0
    for index in range(start, len(tokens)):
        if tokens[index].kind != "symbol":
            continue
        if tokens[index].value == opening:
            depth += 1
        elif tokens[index].value == closing:
            depth -= 1
            if depth == 0:
                return index
    return len(tokens) - 1


def view_ranges(tokens):
    ranges = []
    for index in range(len(tokens) - 2):
        if [token.value for token in tokens[index:index + 3]] == ["view", "!", "{"]:
            end = closing(tokens, index + 2)
            ranges.append((tokens[index + 2].end, tokens[end].start))
    return ranges


def pair_failures(path, line, key, values):
    prefix = f"{path}:{line}: localization {key}: "
    if len(values) < 2:
        return [prefix + "requires two translations (vi-VN and en)"]
    if any(not value.strip() for value in values):
        return [prefix + "blank translation"]
    parameters = [Counter(PLACEHOLDER.findall(value)) for value in values]
    if any(names != parameters[0] for names in parameters[1:]):
        return [prefix + "named placeholder mismatch between locales"]
    return []


def check_catalog(path, source):
    """Supported existing shapes: resources!, paired match arms, Kotlin enums, launch XML."""
    if path.endswith(".xml"):
        try:
            root = ET.fromstring(source)
        except ET.ParseError as error:
            return [f"{path}: localization XML: {error}"]
        values = {item.attrib["name"]: "".join(item.itertext()) for item in root.findall("string")}
        errors = []
        for key in sorted({name.removesuffix("_en") for name in values}):
            errors += pair_failures(path, 1, key, [values[name] for name in (key, key + "_en") if name in values])
        return errors
    tokens, masked = tokenize(source)
    errors = []
    # The invocation grammar already pins the two locales; inspect the actual literals.
    for index in range(len(tokens) - 2):
        if [token.value for token in tokens[index:index + 3]] != ["resources", "!", "{"]:
            continue
        end = closing(tokens, index + 2)
        cursor = index + 3
        while cursor < end:
            if cursor + 3 < end and [token.value for token in tokens[cursor + 1:cursor + 4]] == ["=", ">", "("]:
                last = closing(tokens, cursor + 3, "(", ")")
                values = [token.value for token in tokens[cursor + 4:last] if token.kind == "string"]
                errors += pair_failures(path, tokens[cursor].line, tokens[cursor].value, values)
                cursor = last + 1
            else:
                cursor += 1
    arms = {}
    arm = re.compile(r"\(\s*(\w+)\s*::\s*(\w+)\s*,\s*(false|true|Locale\s*::\s*(?:Vietnamese|English))\s*\)\s*=>\s*\{?\s*$")
    for token in tokens:
        if token.kind != "string":
            continue
        match = arm.search(masked[max(0, token.start - 180):token.start])
        if match:
            enum, key, locale = match.groups()
            locale = "en" if locale == "true" or "English" in locale else "vi"
            arms.setdefault((enum, key), {})[locale] = token
    for (enum, key), locales in arms.items():
        errors += pair_failures(path, next(iter(locales.values())).line, f"{enum}::{key}",
                                [locales[lang].value for lang in ("vi", "en") if lang in locales])
    # Package-local specimen owner matches (locale, typed key), the reverse
    # order of the app catalogs. Locale names are explicit, never inferred text.
    reverse = re.compile(r"\(\s*Locale\s*::\s*(Vi|En)\s*,\s*(\w+)\s*::\s*(\w+)\s*\)\s*=>\s*\{?\s*$")
    reverse_arms = {}
    for token in tokens:
        if token.kind != "string":
            continue
        match = reverse.search(masked[max(0, token.start - 180):token.start])
        if match:
            locale, enum, key = match.groups()
            reverse_arms.setdefault((enum, key), {})[locale] = token
    for (enum, key), locales in reverse_arms.items():
        errors += pair_failures(path, next(iter(locales.values())).line, f"{enum}::{key}",
                                [locales[lang].value for lang in ("Vi", "En") if lang in locales])
    # Existing Studio struct catalogs and direct boolean adapters remain valid
    # owners. Check branch field sets/parameters without migrating their API.
    if path.endswith(".rs"):
        for match in re.finditer(r"\bif\s+(?:english|en)\s*\{", masked):
            brace = next(index for index, token in enumerate(tokens) if token.start == match.end() - 1)
            last = closing(tokens, brace)
            if last + 2 >= len(tokens) or [token.value for token in tokens[last + 1:last + 3]] != ["else", "{"]:
                continue
            other = last + 2
            end = closing(tokens, other)
            branches = [tokens[brace + 1:last], tokens[other + 1:end]]
            fields = []
            for branch in branches:
                values = {}
                for index, token in enumerate(branch):
                    if token.kind == "string" and index >= 2 and branch[index - 1].value == ":":
                        values[branch[index - 2].value] = token
                fields.append(values)
            if any(fields):
                for key in sorted(set(fields[0]) | set(fields[1])):
                    present = [field[key] for field in fields if key in field]
                    errors += pair_failures(path, present[0].line, key, [token.value for token in present])
            elif all(branch and branch[0].kind == "string" for branch in branches):
                errors += pair_failures(path, branches[0][0].line, "boolean locale adapter",
                                        [branch[0].value for branch in branches])
    if path.endswith(".kt"):
        for index, token in enumerate(tokens[:-1]):
            if token.value == "ShellText" and tokens[index + 1].value == "(":
                end = closing(tokens, index + 1, "(", ")")
                values = [item.value for item in tokens[index + 2:end] if item.kind == "string"]
                if values:
                    errors += pair_failures(path, token.line, "ShellText", values)
    if path.endswith(".kt"):
        for match in re.finditer(r"enum\s+class\s+\w+\s*\([^)]*\bvi\s*:\s*String[^)]*\ben\s*:\s*String[^)]*\)\s*\{", masked):
            start = next(index for index, token in enumerate(tokens) if token.start == match.end() - 1)
            end = closing(tokens, start)
            cursor = start + 1
            while cursor < end:
                if tokens[cursor].value == ";":
                    break
                if tokens[cursor].kind == "word" and cursor + 1 < end and tokens[cursor + 1].value == "(":
                    last = closing(tokens, cursor + 1, "(", ")")
                    errors += pair_failures(path, tokens[cursor].line, tokens[cursor].value,
                                            [token.value for token in tokens[cursor + 2:last] if token.kind == "string"])
                    cursor = last + 1
                else:
                    cursor += 1
    return errors


def parameter_name(tokens, index):
    """Named interpolation tuple keys are protocol, not displayed copy."""
    if index == 0 or index + 1 >= len(tokens):
        return False
    if tokens[index - 1].value != "(" or tokens[index + 1].value != ",":
        return False
    return bool(re.fullmatch(r"[A-Za-z_][A-Za-z_0-9]*", tokens[index].value)
                and re.search(r"\bformat(?:_specimen|_named|_message)?\b",
                              source_fragment(tokens, index)))


def source_fragment(tokens, index):
    return " ".join(token.value for token in tokens[max(0, index - 50):index])


def display_tokens(path, source, tokens, masked):
    found = set()
    if path.endswith(".rs"):
        ranges = view_ranges(tokens)
        for start, end in ranges:
            tag = False
            tag_name = None
            attribute = None
            braces = 0
            parentheses = 0
            nested = [(left, right) for left, right in ranges if start < left < right < end]
            for index, token in enumerate(tokens):
                if not start <= token.start < end:
                    continue
                if token.kind == "string":
                    if any(left <= token.start < right for left, right in nested):
                        continue
                    visible_attribute = attribute in VISIBLE_ATTRIBUTES and (attribute != "name" or (tag_name and tag_name[0].isupper()))
                    if not parameter_name(tokens, index) and (not tag or visible_attribute):
                        found.add(index)
                    continue
                if token.value == "<" and not tag and braces == 0:
                    tag = True
                    tag_name = None
                    attribute = None
                elif tag and tag_name is None and token.kind == "word":
                    tag_name = token.value
                elif token.value == ">" and tag and braces == 0 and parentheses == 0:
                    tag = False
                elif tag and token.kind == "word" and index + 1 < len(tokens) and tokens[index + 1].value == "=":
                    # `==` inside a prop expression is not a new attribute.
                    if index + 2 == len(tokens) or tokens[index + 2].value != "=":
                        attribute = token.value
                if token.value == "{":
                    braces += 1
                elif token.value == "}":
                    braces -= 1
                elif tag and token.value == "(":
                    parentheses += 1
                elif tag and token.value == ")":
                    parentheses -= 1
    elif path.endswith(".kt"):
        stack = []
        braces, brackets = 0, 0
        for index, token in enumerate(tokens):
            if token.value == "(":
                name = tokens[index - 1].value if index else ""
                if name == "of" and index >= 3 and tokens[index - 2].value == ".":
                    name = tokens[index - 3].value
                stack.append({"name": name, "argument": 0, "named": None,
                              "braces": braces, "brackets": brackets})
            elif token.value == ")":
                if stack:
                    stack.pop()
            elif token.value == "{":
                braces += 1
            elif token.value == "}":
                braces -= 1
            elif token.value == "[":
                brackets += 1
            elif token.value == "]":
                brackets -= 1
            elif token.value == "," and stack and stack[-1]["braces"] == braces and stack[-1]["brackets"] == brackets:
                stack[-1]["argument"] += 1
                stack[-1]["named"] = None
            elif token.kind == "word" and index + 1 < len(tokens) and tokens[index + 1].value == "=" and stack:
                if stack[-1]["braces"] == braces and stack[-1]["brackets"] == brackets:
                    stack[-1]["named"] = token.value
            elif token.kind == "string":
                before = masked[max(0, token.start - 120):token.start]
                named = re.search(r"\b(\w+)\s*=\s*$", before)
                semantics = named and named.group(1) in {"contentDescription", "stateDescription", "paneTitle"}
                if semantics:
                    found.add(index)
                elif stack and stack[-1]["name"] in UI_CALLS:
                    call = stack[-1]
                    if call["named"] in VISIBLE_ATTRIBUTES or (call["named"] is None and call["argument"] in UI_POSITIONS[call["name"]]):
                        found.add(index)
    return found


class HtmlCopy(HTMLParser):
    def __init__(self):
        super().__init__()
        self.candidates = []
        self.ignored = []

    def handle_starttag(self, tag, attrs):
        if tag in {"script", "style"}:
            self.ignored.append(tag)
        for name, value in attrs:
            if name in VISIBLE_ATTRIBUTES - {"name"} and value:
                raw = self.get_starttag_text()
                attribute = re.search(r"\b" + re.escape(name) + r"\s*=\s*(?:\"[^\"]*\"|'[^']*'|[^\s>]+)", raw, re.IGNORECASE)
                line = self.getpos()[0] + (raw.count("\n", 0, attribute.start()) if attribute else 0)
                last = self.getpos()[0] + (raw.count("\n", 0, attribute.end()) if attribute else 0)
                self.candidates.append(Token("string", value, 0, 0, line, last))

    def handle_endtag(self, tag):
        if self.ignored and self.ignored[-1] == tag:
            self.ignored.pop()

    def handle_data(self, data):
        if data.strip() and not self.ignored:
            leading = len(data) - len(data.lstrip())
            line = self.getpos()[0] + data.count("\n", 0, leading)
            last = self.getpos()[0] + data.rstrip().count("\n")
            self.candidates.append(Token("string", data.strip(), 0, 0, line, last))



def without_rust_tests(source):
    tokens, masked = tokenize(source)
    ranges = []
    for match in re.finditer(r"#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+\w+\s*\{", masked):
        brace = next(index for index, token in enumerate(tokens) if token.start == match.end() - 1)
        ranges.append((match.start(), tokens[closing(tokens, brace)].end))
    chars = list(source)
    for start, end in ranges:
        chars[start:end] = ["\n" if char == "\n" else " " for char in source[start:end]]
    return "".join(chars)

def check_source(path, source, changed_lines=None, policy=None):
    policy = policy or empty_policy()
    if not in_scope(path):
        return []
    if re.search(r"/src/(?:common|desktop|android|ios|jvm|test)[^/]*Test/", path) or "/src/test/" in path or "/tests/" in path:
        return []
    changed = lambda line: changed_lines is None or line in changed_lines
    changed_token = lambda token: any(changed(line) for line in range(token.line, (token.last_line or token.line) + 1))
    owners = {item["path"]: item for item in policy["owners"]}
    catalog_errors = (check_catalog(path, source) if path in owners and owners[path]["mode"] == "bilingual" else [])
    if path in owners and owners[path]["mode"] in {"fixed-english-tool", "generated"}:
        return []
    if path.endswith(".rs"):
        source = without_rust_tests(source)
    tokens, masked = tokenize(source)
    if path.endswith(".html"):
        parser = HtmlCopy()
        parser.feed(source)
        candidates = parser.candidates
    else:
        candidates = [tokens[index] for index in sorted(display_tokens(path, source, tokens, masked))]
    errors = catalog_errors
    if path in owners:
        candidates = []
    for token in candidates:
        if changed_token(token) and token.value.strip() and not exempt(path, token, source, policy):
            errors.append(f"{path}:{token.line}: displayed literal {token.value!r}; use its reviewed resource owner")
    # Existing owner inventory is exact-path; a screen cannot silently become a new catalog.
    locale_map = re.compile(r"\b(?:match|when)\s*\([^{}]*(?:\blocale\b|\benglish\b|\ben\b)[^{}]*\)\s*\{|\bmatch\s+locale\s*\{|\bif\s*\(?\s*(?:(?:english|en)\b|locale\s*==)")
    for match in locale_map.finditer(masked) if path not in owners else []:
        line = source.count("\n", 0, match.start()) + 1
        brace = next((index for index, token in enumerate(tokens) if token.start >= match.start() and token.value == "{"), None)
        end = tokens[closing(tokens, brace)].end if brace is not None else match.end()
        strings = [token for token in tokens if token.kind == "string" and match.end() <= token.start < end]
        if strings and any(changed(number) for number in range(line, source.count("\n", 0, end) + 2)):
            errors.append(f"{path}:{line}: feature-local translation owner is not reviewed; extend an existing owner")
    for match in re.finditer(r"\b(?:macro_rules\s*!\s*resources|resources\s*!\s*\{)", masked) if path not in owners else []:
        line = source.count("\n", 0, match.start()) + 1
        if changed(line):
            errors.append(f"{path}:{line}: translation owner/macro is not reviewed")
    regions = view_ranges(tokens) if path.endswith(".rs") else []
    if path.endswith(".kt"):
        for index, token in enumerate(tokens[:-1]):
            if token.value in UI_CALLS and tokens[index + 1].value == "(":
                end = closing(tokens, index + 1, "(", ")")
                regions.append((tokens[index + 1].end, tokens[end].start))
    for match in RAW_ERROR.finditer(masked):
        line = source.count("\n", 0, match.start()) + 1
        if changed(line) and any(start <= match.start() < end for start, end in regions):
            errors.append(f"{path}:{line}: raw error/API prose in UI sink; use an exhaustive typed message mapping")
    # Only the direct typed subject match is covered; nested HTTP integer
    # fallbacks are intentional and do not hide new closed error variants.
    mapping = re.compile(r"\b(?:fn|fun)\s+\w+\s*\([^)]*\b(error|reason|failure|status)\s*:[^)]*\)\s*(?:->|:)\s*(?:\w*Key|\w*Text)\b[^{}]*\{")
    for match in mapping.finditer(masked):
        start = next((index for index, token in enumerate(tokens) if token.start == match.end() - 1), None)
        if start is None:
            continue
        end = closing(tokens, start)
        subject = re.compile(r"\b(?:match\s+" + match.group(1) + r"|when\s*\(\s*" + match.group(1) + r"\s*\))\s*\{")
        direct = subject.search(masked, match.start(), tokens[end].start)
        if direct is None:
            continue
        brace = next(index for index, token in enumerate(tokens) if token.start == direct.end() - 1)
        depth = 1
        for index in range(brace + 1, closing(tokens, brace)):
            token = tokens[index]
            if token.value == "{":
                depth += 1
            elif token.value == "}":
                depth -= 1
            if depth == 1 and token.value in {"_", "else"} and index + 2 < len(tokens):
                arrow = [item.value for item in tokens[index + 1:index + 3]]
                if arrow in (["=", ">"], ["-", ">"] ) and changed(token.line):
                    errors.append(f"{path}:{token.line}: localized status/error mapping must be exhaustive; remove catch-all")
    return errors


def git(root, *arguments):
    return subprocess.run(["git", *arguments], cwd=root, check=True, capture_output=True,
                          text=True).stdout


def default_base(root):
    head = git(root, "rev-parse", "HEAD").strip()
    event = os.environ.get("GITHUB_EVENT_NAME", "")
    ref = os.environ.get("GITHUB_REF", "")
    ci_merge = event == "pull_request" and re.fullmatch(r"refs/pull/\d+/merge", ref)
    ci_develop = event in {"push", "workflow_dispatch"} and ref == "refs/heads/develop"
    known_ci_parent = os.environ.get("GITHUB_ACTIONS") == "true" and (ci_merge or ci_develop)
    for ref in ("origin/develop", "develop"):
        result = subprocess.run(["git", "merge-base", "HEAD", ref], cwd=root,
                                capture_output=True, text=True)
        if result.returncode == 0:
            base = result.stdout.strip()
            if base != head or not known_ci_parent:
                return base
    # CI checks out PR merge refs at depth 2: parent 1 is the target develop
    # snapshot. On a develop push this checks the immediately preceding commit.
    # Unknown local branch ancestry cannot safely use only its last commit.
    if known_ci_parent:
        parents = git(root, "rev-list", "--parents", "-n", "1", "HEAD").split()
        if ci_merge and len(parents) != 3:
            raise ValueError("localization PR checkout is not a two-parent merge ref; fetch develop history")
        parent = subprocess.run(["git", "rev-parse", "--verify", "HEAD^1"], cwd=root,
                                capture_output=True, text=True)
        if parent.returncode == 0:
            return parent.stdout.strip()
    raise ValueError("localization diff base unavailable; fetch develop history (CI merge/develop checkout needs fetch-depth: 2; no diff was skipped)")


def changed_sources(root, base=None):
    base = default_base(root) if base is None else git(root, "merge-base", "HEAD", base).strip()
    names = set(git(root, "diff", "--name-only", "--diff-filter=ACMR", "-z", base, "--").split("\0"))
    names.update(git(root, "ls-files", "--others", "--exclude-standard", "-z").split("\0"))
    changed = {}
    for name in sorted(filter(in_scope, names)):
        path = root / name
        if not path.is_file():
            continue
        patch = git(root, "diff", "--no-ext-diff", "--unified=0", base, "--", name)
        lines = set()
        for match in re.finditer(r"^@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@", patch, re.MULTILINE):
            start, count = int(match.group(1)), int(match.group(2) or "1")
            lines.update(range(start, start + count))
        if not patch:
            # New, untracked files have no ordinary git diff yet.
            tracked = subprocess.run(["git", "ls-files", "--error-unmatch", "--", name], cwd=root,
                                     capture_output=True).returncode == 0
            if not tracked:
                lines = set(range(1, len(path.read_text(encoding="utf-8").splitlines()) + 1))
        changed[name] = lines
    return changed


def repository_failures(root=ROOT, base=None):
    from localization_catalog import repository_failures as catalog_failures
    from localization_resources import repository_failures as resource_failures

    policy = json.loads((root / "scripts/localization_allowlist.json").read_text(encoding="utf-8"))
    errors = validate_policy(policy)
    if errors:
        return errors
    for name, lines in changed_sources(root, base).items():
        errors += check_source(name, (root / name).read_text(encoding="utf-8"), lines, policy)
    return errors + catalog_failures(root) + resource_failures(root)
