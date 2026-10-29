#!/usr/bin/env python3
"""Generate both preview registries and guard new application component adoption.

Structural registration is not design approval. Qualification records must name actual
runtime, accessibility and inspected-reference evidence for both renderers and match
the current source/token digests. Existing exact import sites are a compatibility baseline.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import sys

from ui_tokens import (
    ROOT, SOURCE, check_outputs, css_value, kotlin_string, load_source,
    validate as validate_tokens, variable, write_outputs,
)

CATALOG = ROOT / "packages/ui-system/shared/preview-catalog.json"
DESIGN_INVENTORY = ROOT / "packages/ui-system/shared/design-inventory-v3.json"
RUST_OUTPUT = ROOT / "packages/ui-system/leptos/src/preview_catalog.rs"
KOTLIN_OUTPUT = ROOT / "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/generated/PreviewCatalog.kt"
KOTLIN_INVENTORY_OUTPUT = ROOT / "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/generated/DesignInventory.kt"
FAMILIES = {"foundation", "actions", "navigation", "fields", "feedback", "layout", "icons", "dialog"}
RENDERERS = {"leptos", "cmp"}
STATUSES = {"registered", "implemented-unverified", "unsupported"}
NAME = re.compile(r"[A-Z][A-Za-z0-9]*")
ID = re.compile(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*")
SHA = re.compile(r"[0-9a-f]{64}")
DESIGN_SCOPES = {"shared-primitive", "shared-composition", "app-owned", "platform-specific"}
DESIGN_FAMILIES = FAMILIES | {"choices", "content", "listening", "studio", "access"}
DESIGN_TEXT_FIELDS = ("id", "name", "family", "design_status", "scope", "owner", "consumer", "reason", "description")
DESIGN_LIST_FIELDS = ("states", "component_ids", "anatomy", "token_roles", "platforms")


def source_text(content, keep_strings=True):
    """Remove comments (including nested blocks), optionally opaque string literals.

    This bounded lexer supports ordinary, Kotlin triple and Rust raw strings plus character
    literals. Rust lifetimes remain identifiers. It is not a compiler; renderer compilation/
    tests remain an independent requirement.
    """
    result, index = [], 0
    strings = re.compile(r'"""[\s\S]*?"{3,}|r(?P<hashes>\#*)"[\s\S]*?"(?P=hashes)|"(?:\\.|[^"\\])*"')
    characters = re.compile(r"'(?:\\(?:u\{[0-9a-fA-F_]+\}|u[0-9a-fA-F]{4}|x[0-9a-fA-F]{2}|[^\r\n])|[^'\\\r\n])'")
    while index < len(content):
        literal = characters.match(content, index) or strings.match(content, index)
        if literal:
            result.append(literal.group() if keep_strings else " ")
            index = literal.end()
        elif content.startswith("//", index):
            end = content.find("\n", index)
            index = len(content) if end < 0 else end
        elif content.startswith("/*", index):
            depth = 1
            index += 2
            while index < len(content) and depth:
                if content.startswith("/*", index):
                    depth += 1
                    index += 2
                elif content.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    if content[index] == "\n":
                        result.append("\n")
                    index += 1
            result.append(" ")
        else:
            result.append(content[index])
            index += 1
    return "".join(result)


def object_keys(value, keys, label):
    if not isinstance(value, dict) or set(value) != set(keys):
        raise ValueError(f"{label}: expected keys {', '.join(sorted(keys))}")


def text(value, label):
    if not isinstance(value, str) or not value.strip() or any(ord(c) < 32 for c in value):
        raise ValueError(f"{label}: nonempty single-line text required")
    return value


def relative_path(value, label):
    value = text(value, label)
    path = Path(value)
    if path.is_absolute() or ".." in path.parts or path.as_posix() != value:
        raise ValueError(f"{label}: repository-relative path required")
    return path


def safe_location(root, value, label):
    root = Path(root).resolve()
    path = root / relative_path(value, label)
    if not path.resolve().is_relative_to(root):
        raise ValueError(f"{label}: symlink escapes repository")
    return path


def validate(source):
    object_keys(source, {"version", "components", "baseline_consumers", "qualifications"}, "catalog")
    if type(source["version"]) is not int or source["version"] != 1:
        raise ValueError("catalog: supported version is 1")
    if not isinstance(source["components"], list) or not source["components"]:
        raise ValueError("catalog: components must be a nonempty list")
    ids, names = set(), set()
    for entry in source["components"]:
        object_keys(entry, {"id", "name", "family", "states", "leptos", "cmp"}, "component")
        if not isinstance(entry["id"], str) or not ID.fullmatch(entry["id"]) or entry["id"] in ids:
            raise ValueError("component: invalid or duplicate id")
        if not isinstance(entry["name"], str) or not NAME.fullmatch(entry["name"]) or entry["name"] in names:
            raise ValueError("component: invalid or duplicate name")
        ids.add(entry["id"])
        names.add(entry["name"])
        if entry["family"] not in FAMILIES:
            raise ValueError(f"{entry['id']}: unknown family")
        states = entry["states"]
        if not isinstance(states, list) or not states or any(not isinstance(s, str) or not ID.fullmatch(s) for s in states):
            raise ValueError(f"{entry['id']}: named states required")
        if len(set(states)) != len(states):
            raise ValueError(f"{entry['id']}: duplicate states")
        for renderer in sorted(RENDERERS):
            registration = entry[renderer]
            object_keys(registration, {"status", "reason"}, f"{entry['id']}/{renderer}")
            if registration["status"] not in STATUSES:
                raise ValueError(f"{entry['id']}/{renderer}: registration is not qualification")
            text(registration["reason"], f"{entry['id']}/{renderer}/reason")
    if not isinstance(source["baseline_consumers"], list):
        raise ValueError("baseline_consumers: list required")
    seen = set()
    for site in source["baseline_consumers"]:
        object_keys(site, {"renderer", "path", "components"}, "baseline consumer")
        renderer = site["renderer"]
        path = relative_path(site["path"], "baseline consumer path")
        if renderer not in RENDERERS or not path.parts or path.parts[0] != "apps":
            raise ValueError("baseline consumer: application renderer/path required")
        key = renderer, path.as_posix()
        if key in seen:
            raise ValueError("baseline consumer: duplicate site")
        seen.add(key)
        entries = site["components"]
        if not isinstance(entries, list) or not entries or len(set(entries)) != len(entries) or not set(entries) <= names:
            raise ValueError("baseline consumer: registered unique components required")
    if not isinstance(source["qualifications"], list):
        raise ValueError("qualifications: list required")
    qualified = set()
    for item in source["qualifications"]:
        object_keys(item, {"component_id", "evidence"}, "qualification")
        ident = item["component_id"]
        if ident not in ids or ident in qualified:
            raise ValueError("qualification: unknown or duplicate component")
        qualified.add(ident)
        object_keys(item["evidence"], RENDERERS, "qualification evidence")
        for renderer, path in item["evidence"].items():
            relative_path(path, f"qualification/{renderer}")
            if not path.startswith("docs/evidence/") or not path.endswith(".json"):
                raise ValueError("qualification: evidence must be a repository docs/evidence JSON record")
    return source


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":")).encode()).hexdigest()


def named_design_values(values, label, maximum=24):
    if not isinstance(values, list) or not 1 <= len(values) <= maximum:
        raise ValueError(f"{label}: bounded nonempty list required")
    for value in values:
        text(value, label)
    if len(set(values)) != len(values):
        raise ValueError(f"{label}: duplicate value")


def validate_design_inventory(inventory, source, tokens):
    """Keep offline proposals outside the runtime registry and preserve the exact baseline.

    Design states describe a bounded requested matrix, not implemented props. Registration,
    qualification and guarded application adoption continue to use CATALOG alone.
    """
    object_keys(inventory, {"version", "source", "baseline", "specimens"}, "design inventory")
    if type(inventory["version"]) is not int or inventory["version"] != 1:
        raise ValueError("design inventory: supported version is 1")
    provenance = inventory["source"]
    object_keys(provenance, {"version", "url", "sha256", "handoff_url", "kind", "retrieved_on"}, "design source")
    for key, value in provenance.items():
        text(value, f"design source/{key}")
    if not SHA.fullmatch(provenance["sha256"]):
        raise ValueError("design source: raw manifest SHA-256 required")
    if provenance["kind"] != "offline-static-design-proposal":
        raise ValueError("design source: offline proposal provenance required")
    for key in ("url", "handoff_url"):
        if not re.fullmatch(r"https://drive\.google\.com/file/d/[A-Za-z0-9_-]+/view", provenance[key]):
            raise ValueError(f"design source/{key}: canonical Drive file URL required")
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", provenance["retrieved_on"]):
        raise ValueError("design source: retrieval date required")
    baseline = inventory["baseline"]
    object_keys(baseline, {"source_revision", "catalog_sha256", "catalog_digest", "catalog_snapshot"}, "design baseline")
    if not isinstance(baseline["source_revision"], str) or not re.fullmatch(r"[0-9a-f]{40}", baseline["source_revision"]):
        raise ValueError("design baseline: source revision required")
    for key in ("catalog_sha256", "catalog_digest"):
        if not isinstance(baseline[key], str) or not SHA.fullmatch(baseline[key]):
            raise ValueError(f"design baseline/{key}: SHA-256 required")
    snapshot = validate(baseline["catalog_snapshot"])
    if digest(snapshot) != baseline["catalog_digest"]:
        raise ValueError("design baseline: snapshot digest mismatch")
    entries = {entry["id"]: entry for entry in source["components"]}
    original = {entry["id"]: entry for entry in snapshot["components"]}
    if len(original) != 32 or sum(len(entry["states"]) for entry in original.values()) != 198:
        raise ValueError("design baseline: exact 32 API / 198 state snapshot required")
    for ident, entry in original.items():
        current = entries.get(ident)
        if current is None:
            raise ValueError(f"baseline API missing: {ident}")
        if (current["name"], current["family"]) != (entry["name"], entry["family"]):
            raise ValueError(f"baseline API changed: {ident}")
        if not set(entry["states"]) <= set(current["states"]):
            raise ValueError(f"baseline states lost: {ident}")
    if source["baseline_consumers"] != snapshot["baseline_consumers"]:
        raise ValueError("design baseline: compatibility sites changed; explicit migration review required")
    specimens = inventory["specimens"]
    if not isinstance(specimens, list) or len(specimens) != 102:
        raise ValueError("design inventory: exact 102 design specimens required")
    seen, existing, proposed = set(), set(), set()
    roles = set(tokens["tokens"]) | set(tokens["themes"]["light"]) | set(tokens["aliases"])
    for entry in specimens:
        object_keys(entry, set(DESIGN_TEXT_FIELDS + DESIGN_LIST_FIELDS), "design specimen")
        for key in DESIGN_TEXT_FIELDS:
            text(entry[key], f"design/{entry['id']}/{key}")
        ident = entry["id"]
        if not ID.fullmatch(ident) or ident in seen:
            raise ValueError(f"design inventory: invalid or duplicate design id: {ident}")
        seen.add(ident)
        if entry["family"] not in DESIGN_FAMILIES:
            raise ValueError(f"design/{ident}: unknown design family")
        if entry["design_status"] not in {"existing", "proposed"}:
            raise ValueError(f"design/{ident}: design status is not runtime qualification")
        if entry["scope"] not in DESIGN_SCOPES:
            raise ValueError(f"design/{ident}: unknown ownership scope")
        for key in DESIGN_LIST_FIELDS:
            named_design_values(entry[key], f"design/{ident}/{key}")
        if not set(entry["component_ids"]) <= set(entries):
            raise ValueError(f"design/{ident}: unknown runtime component")
        if not set(entry["token_roles"]) <= roles:
            raise ValueError(f"design/{ident}: unknown canonical token")
        if not set(entry["platforms"]) <= {"web", "cmp"}:
            raise ValueError(f"design/{ident}: unknown platform")
        if entry["family"] in {"studio", "access"} and entry["platforms"] != ["web"]:
            raise ValueError(f"design/{ident}: Studio and Admin proposals are Web only")
        if entry["design_status"] == "existing":
            existing.add(ident)
            if ident not in original or entry["component_ids"] != [ident]:
                raise ValueError(f"design/{ident}: existing design must map to its baseline API")
        else:
            proposed.add(ident)
            if ident in original:
                raise ValueError(f"design/{ident}: baseline API cannot become a proposal")
    if existing != set(original) or len(proposed) != 70:
        raise ValueError("design inventory: exact 32 existing / 70 proposed mapping required")
    return inventory


def load_design_inventory(source, tokens):
    return validate_design_inventory(load_source(DESIGN_INVENTORY.read_bytes()), source, tokens)


def renderer_digest(renderer, root=ROOT, declarations=None):
    """Invalidate approval for runtime/source edits and component/state contract changes.

    Exclude evidence and baseline metadata from the declaration digest: evidence contains this
    hash, so hashing the whole catalogue would make qualification recursively impossible.
    """
    directory = Path(root) / "packages/ui-system" / renderer
    suffixes = {".rs", ".css", ".in", ".html", ".toml", ".lock", ".json", ".svg", ".avif", ".png", ".ttf", ".otf"} if renderer == "leptos" else {".kt", ".kts", ".properties", ".toml", ".lockfile", ".json", ".xml", ".svg", ".avif", ".png", ".ttf", ".otf"}
    excluded = {RUST_OUTPUT.relative_to(ROOT), KOTLIN_OUTPUT.relative_to(ROOT)}
    if renderer == "cmp":
        # The SDK location is machine-local configuration, not a rendering input.
        excluded.add(Path("packages/ui-system/cmp/android-preview/local.properties"))
    records = {}
    if declarations is None:
        declarations = load_source((Path(root) / CATALOG.relative_to(ROOT)).read_bytes())["components"]
    records["shared-component-declarations"] = digest(declarations)
    build_inputs = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml") if renderer == "leptos" else ()
    for relative in build_inputs:
        path = Path(root) / relative
        records[relative] = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else "absent"
    if renderer == "leptos":
        # Preview typography and discovery metadata live in shared inputs outside the
        # renderer directory. Their generated outputs alone must not hide stale review.
        shared_inputs = [Path(root) / DESIGN_INVENTORY.relative_to(ROOT)]
        fonts = Path(root) / "packages/ui-system/shared/assets/noto-fonts"
        if fonts.is_dir():
            shared_inputs.extend(path for path in sorted(fonts.rglob("*")) if path.is_file())
        for path in shared_inputs:
            relative = path.relative_to(root).as_posix()
            records[relative] = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else "absent"
    for path in sorted(directory.rglob("*")):
        relative = path.relative_to(directory)
        generated_output = relative.parts[0] in {"build", ".gradle"}
        if renderer == "cmp":
            # Only this isolated project's build/IDE output roots are excluded; source
            # directories with those names elsewhere still affect qualification.
            generated_output |= relative.parts[:2] in {
                ("android-preview", "build"), ("android-preview", ".gradle"),
                ("android-preview", ".idea"), ("android-preview", ".kotlin"),
            }
        if path.is_file() and path.suffix in suffixes and path.relative_to(root) not in excluded and not generated_output:
            records[path.relative_to(root).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return digest(records)


def validate_evidence(entry, renderer, path, tokens_sha256, root=ROOT, declarations=None):
    """Verify record freshness and honest named evidence; no pixel comparison is fabricated."""
    location = safe_location(root, path, "evidence path")
    try:
        record = load_source(location.read_bytes())
    except OSError as error:
        raise ValueError(f"{entry['id']}/{renderer}: missing qualification evidence {path}") from error
    object_keys(record, {"component_id", "renderer", "status", "reviewer", "reference", "tokens_sha256", "source_sha256", "checks"}, "evidence")
    if record["component_id"] != entry["id"] or record["renderer"] != renderer or record["status"] != "PASS":
        raise ValueError(f"{entry['id']}/{renderer}: evidence identity/status mismatch")
    text(record["reviewer"], "evidence reviewer")
    text(record["reference"], "evidence inspected reference")
    if record["tokens_sha256"] != tokens_sha256 or record["source_sha256"] != renderer_digest(renderer, root, declarations):
        raise ValueError(f"{entry['id']}/{renderer}: stale source or token qualification")
    object_keys(record["checks"], {"structural", "runtime", "accessibility", "visual"}, "evidence checks")
    for check, result in record["checks"].items():
        object_keys(result, {"status", "command", "artifact"}, f"evidence/{check}")
        if result["status"] != "PASS":
            raise ValueError(f"{entry['id']}/{renderer}: {check} is not PASS")
        text(result["command"], f"evidence/{check}/command")
        artifact = safe_location(root, result["artifact"], f"evidence/{check}/artifact")
        if not artifact.is_file():
            raise ValueError(f"{entry['id']}/{renderer}: missing {check} artifact")
    if entry[renderer]["status"] == "unsupported":
        raise ValueError(f"{entry['id']}/{renderer}: unsupported cannot be qualified")


def kotlin_source_regions(content, keep_lines=False):
    """Separate ordinary code from executable template expressions, ignoring inert text.

    This is a bounded lexical walk, not a Kotlin parser. String/comment delimiters and
    balanced expression braces are enough to keep static namespaces out of admission.
    With keep_lines, each inert region keeps its newlines so offsets in the returned
    code map back to source line numbers.
    """
    code, expressions, index = [], [], 0
    stack = [["code", 0]]
    characters = re.compile(r"'(?:\\(?:u[0-9a-fA-F]{4}|[^\r\n])|[^'\\\r\n])'")
    while index < len(content):
        mode = stack[-1][0]
        if mode in {"string", "triple"}:
            delimiter = '"""' if mode == "triple" else '"'
            if content.startswith(delimiter, index):
                lines = stack.pop()[2]
                index += len(delimiter)
                if mode == "triple":
                    # Kotlin's TRIPLE_QUOTE_CLOSE consumes the complete closing run;
                    # extra quotes are literal content, never a following string opener.
                    while index < len(content) and content[index] == '"':
                        index += 1
                if keep_lines:
                    (expressions if stack[-1][0] == "expression" else code).append("\n" * lines)
            elif mode == "string" and content[index] == "\\":
                index += 2
            elif content.startswith("${", index):
                expressions.append(" ")
                stack.append(["expression", 1])
                index += 2
            else:
                stack[-1][2] += content[index] == "\n"
                index += 1
            continue

        output = expressions if mode == "expression" else code
        if content.startswith("//", index):
            end = content.find("\n", index)
            index = len(content) if end < 0 else end
            output.append(" ")
        elif content.startswith("/*", index):
            start, depth, index = index, 1, index + 2
            while index < len(content) and depth:
                if content.startswith("/*", index):
                    depth, index = depth + 1, index + 2
                elif content.startswith("*/", index):
                    depth, index = depth - 1, index + 2
                else:
                    index += 1
            output.append(" " + "\n" * content.count("\n", start, index) * keep_lines)
        elif literal := characters.match(content, index):
            index = literal.end()
            output.append(" ")
        elif content.startswith('"', index):
            triple = content.startswith('"""', index)
            stack.append(["triple" if triple else "string", 0, 0])
            index += 3 if triple else 1
            output.append(" ")
        elif mode == "expression" and content[index] == "}":
            stack[-1][1] -= 1
            if stack[-1][1] == 0:
                stack.pop()
                output.append(" ")
            else:
                output.append("}")
            index += 1
        else:
            if mode == "expression" and content[index] == "{":
                stack[-1][1] += 1
            if mode == "expression" and content[index] == "\n":
                stack[-2][2] += 1
            output.append(content[index])
            index += 1
    return "".join(code), "".join(expressions)


def imported_components(content, renderer, names):
    """Inspect explicit shared-library imports, including aliases; reject wildcard bypasses."""
    original = content
    content = source_text(content, False)
    if renderer == "leptos":
        content = re.sub(r"\br#([A-Za-z_]\w*)", r"\1", content)
        content = re.sub(r"\s*::\s*", "::", content)
        imports = []
        for match in re.finditer(r"\buse\b\s*(?:::)?cantos_ui::\{([^}]+)\}", content, re.S):
            if "{" in match.group(1):
                raise ValueError("explicit cantos_ui component imports required for adoption checks")
            imports += [re.split(r"\s+as\s+", part.strip())[0] for part in match.group(1).split(",")]
        imports += re.findall(r"\bcantos_ui::([A-Z]\w*)", content)
        opaque_namespace = (
            r"(?:\buse\b|extern\s+crate)\s*(?:::)?cantos_ui\s*(?:as\b|;)"
            r"|\buse\b\s*(?:::)?cantos_ui::\{[^}]*\bself\b"
            r"|\buse\b\s*\{[^}]*\bcantos_ui\b"
        )
        if "*" in imports or re.search(r"\buse\b\s*(?:::)?cantos_ui::\*|" + opaque_namespace, content, re.S):
            raise ValueError("explicit cantos_ui component imports required for adoption checks")
    else:
        content, expressions = kotlin_source_regions(original)
        content = re.sub(r"`([A-Za-z_]\w*)`", r"\1", content)
        content = re.sub(r"\s*\.\s*", ".", content)
        # Kotlin resolves public top-level declarations in the same package without
        # imports. Product sources may not join the shared package to hide adoption.
        if re.search(r"\bpackage\s+com\.cantos\.ui(?:\s|;|$)", content):
            raise ValueError("explicit CMP component imports required for shared-package adoption checks")
        imports = re.findall(r"\bcom\.cantos\.ui\.([A-Z]\w*)", content)
        if re.search(r"\bimport\s+com\.cantos\.ui\.\*", content):
            raise ValueError("explicit CMP component imports required for adoption checks")
        # Calls inside executable string templates need explicit imports so admission
        # cannot be hidden inside a literal. Inert strings and comments are absent here.
        expressions = re.sub(r"`([A-Za-z_]\w*)`", r"\1", expressions)
        expressions = re.sub(r"\s*\.\s*", ".", expressions)
        hidden = set(re.findall(r"\bcom\.cantos\.ui\.([A-Z]\w*)", expressions)) - set(imports)
        if hidden.intersection(names):
            raise ValueError("explicit CMP component imports required for template adoption checks")
    return set(imports).intersection(names)


def dependency_alias_failures(root):
    """Keep the shared Rust crate visible under its canonical source namespace.

    TOML is standard-library parsed where available. Older Python conservatively refuses
    package aliases/escaped keys rather than trusting a lexical interpretation of them.
    """
    try:
        import tomllib
    except ImportError:
        tomllib = None

    def renamed_dependencies(value):
        if not isinstance(value, dict):
            return False
        for key, item in value.items():
            if key in {"dependencies", "dev-dependencies", "build-dependencies"} and isinstance(item, dict):
                if any(isinstance(dependency, dict) and dependency.get("package") == "cantos-ui"
                       and name.replace("-", "_") != "cantos_ui" for name, dependency in item.items()):
                    return True
            if renamed_dependencies(item):
                return True
        return False

    failures = []
    for path in sorted({root / "Cargo.toml", *(root / "apps").rglob("Cargo.toml")}):
        if not path.is_file():
            continue
        relative, content = path.relative_to(root).as_posix(), path.read_text()
        if tomllib is None:
            if re.search(r'''(?:["']?\bpackage\b["']?\s*=|\\[uU])''', content):
                failures.append(f"{relative}: Python 3.11+ required to inspect dependency package aliases")
            continue
        try:
            manifest = tomllib.loads(content)
        except ValueError:
            failures.append(f"{relative}: cannot inspect dependency aliases in invalid TOML")
            continue
        if renamed_dependencies(manifest):
            failures.append(f"{relative}: cantos-ui dependency rename hides adoption; use the canonical cantos_ui namespace")
    return failures


def repository_failures(source, token_source, root=ROOT):
    """Fail absent declarations/samples, drift and new use without both fresh qualifications."""
    root = Path(root)
    failures = dependency_alias_failures(root)
    entries = {entry["name"]: entry for entry in source["components"]}
    names = set(entries)
    leptos = root / "packages/ui-system/leptos"
    exported = set()
    for path in (leptos / "src").rglob("*.rs"):
        exported.update(re.findall(r"#\[component(?:\([^]]*\))?\]\s*(?:#\[[^]]*\]\s*)*pub\s+fn\s+([A-Za-z_]\w*)", source_text(path.read_text(), False), re.S))
    if exported != names:
        failures.append(f"Leptos component inventory mismatch: missing {sorted(exported - names)}, stale {sorted(names - exported)}")
    gallery = "\n".join(source_text(path.read_text(), False) for path in sorted((leptos / "preview").glob("*.rs")))
    for name in sorted(names):
        if not re.search(r"<" + name + r"(?:\s|/|>)", gallery):
            failures.append(f"Leptos preview omits actual {name} sample")
    cmp = root / "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui"
    if not (cmp / "Components.kt").is_file():
        failures.append("CMP reusable Components.kt is missing")
    else:
        declared = set()
        for path in sorted(cmp.rglob("*.kt")):
            if {"generated", "preview"}.intersection(path.relative_to(cmp).parts):
                continue
            code = source_text(path.read_text(), False)
            code = re.sub(r"`([A-Za-z_]\w*)`", r"\1", code)
            code = re.sub(r"\s*\.\s*", ".", code)
            annotation_names = {"Composable", "androidx.compose.runtime.Composable"}
            annotation_names.update(re.findall(
                r"\bimport\s+androidx\.compose\.runtime\.Composable\s+as\s+([A-Za-z_]\w*)", code))
            # Direct annotation type aliases can be chained or declared out of order.
            aliases = re.findall(r"\btypealias\s+([A-Za-z_]\w*)\s*=\s*([A-Za-z_][\w.]*)", code)
            while True:
                resolved = {alias for alias, target in aliases if target in annotation_names}
                if resolved <= annotation_names:
                    break
                annotation_names.update(resolved)
            annotation = "@(?:" + "|".join(re.escape(name) for name in sorted(annotation_names)) + ")\\b"
            # Default-public and explicitly public composables remain API declarations
            # with ordinary Kotlin modifiers and type parameters. Private/internal helpers
            # do not become shared candidates merely because they are composable.
            declarations = re.finditer(
                annotation + r"(?:\(\s*\))?\s+(?:@[\w.]+(?:\([^)]*\))?\s+)*"
                r"(?P<modifiers>(?:(?:public|private|internal|protected|inline|suspend|tailrec|"
                r"operator|infix|external|expect|actual|final|open|abstract|override)\s+)*)"
                r"fun\b\s*(?:<[^()]*>\s*)?(?P<name>[A-Z]\w*)",
                code, re.S,
            )
            declared.update(match["name"] for match in declarations
                            if not {"private", "internal"}.intersection(match["modifiers"].split()))
        # UiRoot owns the theme adapter; it is not a second standalone component contract.
        declared.discard("CantosTheme")
        supported = {name for name in names if entries[name]["cmp"]["status"] != "unsupported"}
        if declared != supported:
            failures.append(f"CMP component inventory mismatch: missing {sorted(supported - declared)}, undeclared {sorted(declared - supported)}")
    cmp_gallery = cmp / "preview/PreviewApp.kt"
    if not cmp_gallery.is_file():
        failures.append("CMP preview app is missing")
    else:
        content = source_text(cmp_gallery.read_text(), False)
        model = source_text((cmp / "PreviewModel.kt").read_text()) if (cmp / "PreviewModel.kt").is_file() else ""
        for entry in source["components"]:
            branch = re.search(r"PreviewExample\." + entry["name"] + r"\s*->(.*?)(?=\n\s*PreviewExample\.|\Z)", content, re.S)
            mapping = re.search(r"\b" + entry["name"] + r'\s*\(\s*"' + re.escape(entry["id"]) + r'"\s*\)', model)
            invoked = branch and re.search(r"\b" + entry["name"] + r"\s*\(", branch.group(1))
            # Dialog needs a retained-state/focus helper. Its dispatch and real wrapper call
            # are both mandatory; future indirect sample helpers need explicit gate review.
            if entry["name"] == "Dialog" and branch:
                invoked = re.search(r"\bDialogExample\s*\(", branch.group(1)) and re.search(r"fun\s+DialogExample\b[\s\S]*?\bDialog\s*\(", content)
            if not (mapping and branch and (invoked or entry["cmp"]["status"] == "unsupported")):
                failures.append(f"CMP preview omits {entry['id']} dispatch")
    baseline = {(site["renderer"], site["path"]): set(site["components"]) for site in source["baseline_consumers"]}
    qualifications = {item["component_id"]: item["evidence"] for item in source["qualifications"]}
    tokens_sha256 = hashlib.sha256(json.dumps(token_source, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    fresh = set()
    for ident, paths in qualifications.items():
        entry = next(item for item in source["components"] if item["id"] == ident)
        try:
            for renderer, path in paths.items():
                validate_evidence(entry, renderer, path, tokens_sha256, root, source["components"])
            fresh.add(entry["name"])
        except ValueError as error:
            failures.append(str(error))
    for path in sorted((root / "apps").rglob("*")):
        if path.suffix not in {".rs", ".kt"} or not path.is_file():
            continue
        renderer = "leptos" if path.suffix == ".rs" else "cmp"
        relative = path.relative_to(root).as_posix()
        try:
            used = imported_components(path.read_text(), renderer, names)
        except ValueError as error:
            failures.append(f"{relative}: {error}")
            continue
        pending = used - baseline.get((renderer, relative), set()) - fresh
        if pending:
            failures.append(f"{relative}: new use requires both preview qualifications: {', '.join(sorted(pending))}")
    return failures


def rust_render(source, tokens, inventory=None):
    header = f"// Generated by python3 scripts/ui_preview_catalog.py; catalog sha256:{digest(source)}. Do not edit."
    lines = [header, "", "pub struct RendererRegistration {", "    pub status: &'static str,", "    pub reason: &'static str,", "}", "",
             "pub struct ComponentRegistration {", "    pub id: &'static str,", "    pub name: &'static str,", "    pub family: &'static str,", "    pub states: &'static [&'static str],", "    pub leptos: RendererRegistration,", "    pub cmp: RendererRegistration,", "}", "",
             "pub const COMPONENTS: &[ComponentRegistration] = &["]
    for entry in source["components"]:
        lines += ["    ComponentRegistration {"]
        lines += [f'        {key}: {json.dumps(entry[key], ensure_ascii=False)},' for key in ("id", "name", "family")]
        lines += ["        states: &[" + ", ".join(json.dumps(s) for s in entry["states"]) + "],"]
        for renderer in sorted(RENDERERS):
            registration = entry[renderer]
            lines += [f"        {renderer}: RendererRegistration {{", f'            status: {json.dumps(registration["status"], ensure_ascii=False)},', f'            reason: {json.dumps(registration["reason"], ensure_ascii=False)},', "        },"]
        lines += ["    },"]
    lines += ["];", "", "pub struct TokenRegistration {", "    pub name: &'static str,", "    pub css_variable: &'static str,", "    pub kind: &'static str,", "    pub light: &'static str,", "    pub dark: &'static str,", "    pub compact: Option<&'static str>,", "    pub comfortable: Option<&'static str>,", "    pub alias_of: Option<&'static str>,", "}", "", f'pub const TOKEN_VERSION: &str = "{tokens["version"]}";', "pub const TOKENS: &[TokenRegistration] = &["]
    canonical = tokens["tokens"] | tokens["themes"]["light"]
    for role in sorted(set(canonical) | set(tokens["aliases"])):
        target = tokens["aliases"].get(role, role)
        light = tokens["tokens"].get(target, tokens["themes"]["light"].get(target))
        dark = tokens["tokens"].get(target, tokens["themes"]["dark"].get(target))
        lines += ["    TokenRegistration {"]
        for key, value in (("name", role), ("css_variable", variable(role)), ("kind", light["type"]), ("light", css_value(light)), ("dark", css_value(dark))):
            lines.append(f"        {key}: {json.dumps(value, ensure_ascii=False)},")
        for density in ("compact", "comfortable"):
            value = tokens["density"][density].get(target)
            rendered = "None" if value is None else "Some(" + json.dumps(css_value({"type": "dimension", "value": value})) + ")"
            lines.append(f"        {density}: {rendered},")
        lines += [f'        alias_of: {"Some(" + json.dumps(target) + ")" if target != role else "None"},', "    },"]
    lines += ["];", ""]
    if inventory is not None:
        lines += rust_design_inventory(inventory)
    return "\n".join(lines)


def rust_design_inventory(inventory):
    """Generate preview-only discovery metadata, never public API registrations."""
    provenance = inventory["source"]
    values = (("DESIGN_VERSION", provenance["version"]), ("DESIGN_SOURCE_URL", provenance["url"]),
              ("DESIGN_SOURCE_SHA256", provenance["sha256"]),
              ("DESIGN_BASELINE_SHA256", inventory["baseline"]["catalog_sha256"]))
    lines = [f"// Offline design inventory sha256:{digest(inventory)}; registration and qualification remain separate."]
    for name, value in values:
        declaration = f"pub const {name}: &str = {json.dumps(value)};"
        if len(declaration) > 100:
            lines += [f"pub const {name}: &str =", f"    {json.dumps(value)};"]
        else:
            lines.append(declaration)
    lines += ["", "#[derive(Clone, Copy, Debug, PartialEq, Eq)]", "pub struct DesignSpecimen {"]
    lines += [f"    pub {key}: &'static str," for key in DESIGN_TEXT_FIELDS]
    lines += [f"    pub {key}: &'static [&'static str]," for key in DESIGN_LIST_FIELDS]
    lines += ["}", "", "pub const DESIGN_SPECIMENS: &[DesignSpecimen] = &["]
    for entry in inventory["specimens"]:
        lines += ["    DesignSpecimen {"]
        lines += [f"        {key}: {json.dumps(entry[key], ensure_ascii=False)}," for key in DESIGN_TEXT_FIELDS]
        lines += ["        " + key + ": &[" + ", ".join(json.dumps(value, ensure_ascii=False) for value in entry[key]) + "],"
                  for key in DESIGN_LIST_FIELDS]
        lines += ["    },"]
    lines += ["];", ""]
    return lines


def kotlin_render(source):
    lines = [f"// Generated by python3 scripts/ui_preview_catalog.py; catalog sha256:{digest(source)}. Do not edit.", "package com.cantos.ui.generated", "", "data class PreviewComponent(", "    val id: String, val name: String, val category: String, val states: List<String>,", "    val cmpStatus: String, val cmpReason: String?,", "    val leptosStatus: String, val leptosReason: String?", ")", "", "object PreviewCatalog {", "    val components: List<PreviewComponent> = listOf("]
    for entry in source["components"]:
        values = ", ".join(kotlin_string(entry[key]) for key in ("id", "name", "family"))
        states = ", ".join(kotlin_string(s) for s in entry["states"])
        renderer_values = ", ".join(kotlin_string(entry[r][key]) for r in ("cmp", "leptos") for key in ("status", "reason"))
        lines.append(f"        PreviewComponent({values}, listOf({states}), {renderer_values}),")
    lines += ["    )", "}", ""]
    return "\n".join(lines)


def kotlin_camel(key):
    head, *tail = key.split("_")
    return head + "".join(part.capitalize() for part in tail)


def kotlin_design_inventory(inventory):
    """Generate the CMP gallery's design discovery metadata; never a public registration.

    The Kotlin registry (`PreviewCatalog`) keeps its runtime-only role: this file carries design
    status and ownership for display and is neither registration nor qualification. It lives under
    `preview/`, which the Theatre mock's build already excludes, so product apps never ship it.
    """
    provenance = inventory["source"]
    fields = [(kotlin_camel(key), "String") for key in DESIGN_TEXT_FIELDS]
    fields += [(kotlin_camel(key), "List<String>") for key in DESIGN_LIST_FIELDS]
    lines = [f"// Generated by python3 scripts/ui_preview_catalog.py; design inventory sha256:{digest(inventory)}. Do not edit.",
             "package com.cantos.ui.preview.generated", "",
             "/** One offline design specimen. Design status and ownership are neither registration nor qualification. */",
             "data class DesignSpecimen("]
    lines += [f"    val {name}: {kind}," for name, kind in fields]
    lines += [")", "", "object DesignInventory {",
              f"    const val version: String = {kotlin_string(provenance['version'])}",
              f"    const val sourceSha256: String = {kotlin_string(provenance['sha256'])}",
              f"    const val baselineSha256: String = {kotlin_string(inventory['baseline']['catalog_sha256'])}",
              "    val specimens: List<DesignSpecimen> = listOf("]
    for entry in inventory["specimens"]:
        values = [kotlin_string(entry[key]) for key in DESIGN_TEXT_FIELDS]
        values += ["listOf(" + ", ".join(kotlin_string(value) for value in entry[key]) + ")" for key in DESIGN_LIST_FIELDS]
        lines.append("        DesignSpecimen(" + ", ".join(values) + "),")
    lines += ["    )", "}", ""]
    return "\n".join(lines)


def outputs(source, tokens, output_root=ROOT, inventory=None):
    source, tokens = validate(source), validate_tokens(tokens)
    if inventory is not None:
        validate_design_inventory(inventory, source, tokens)
    generated = {Path(output_root) / RUST_OUTPUT.relative_to(ROOT): rust_render(source, tokens, inventory),
                 Path(output_root) / KOTLIN_OUTPUT.relative_to(ROOT): kotlin_render(source)}
    if inventory is not None:
        generated[Path(output_root) / KOTLIN_INVENTORY_OUTPUT.relative_to(ROOT)] = kotlin_design_inventory(inventory)
    return generated


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output-root", type=Path, default=ROOT)
    parser.add_argument("--digests", action="store_true", help="Print current qualification digests, without approving anything")
    args = parser.parse_args(argv)
    try:
        source = validate(load_source(CATALOG.read_bytes()))
        tokens = validate_tokens(load_source(SOURCE.read_bytes()))
        inventory = load_design_inventory(source, tokens)
        if args.digests:
            print(json.dumps({"tokens_sha256": digest(tokens),
                              "source_sha256": {renderer: renderer_digest(renderer, args.output_root, source["components"])
                                                for renderer in sorted(RENDERERS)}}, indent=2))
            return 0
        expected = outputs(source, tokens, args.output_root, inventory)
        if args.check:
            failures = check_outputs(expected) + repository_failures(source, tokens, args.output_root)
            if failures:
                print("FAIL: preview catalog\n" + "\n".join(failures), file=sys.stderr)
                return 1
            print("PASS: two preview registries, named tokens and guarded adoption agree")
        else:
            write_outputs(expected)
            print(f"PASS: generated {len(expected)} preview catalog artifacts; qualification unchanged")
        return 0
    except (OSError, ValueError) as error:
        print(f"FAIL: preview catalog: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
