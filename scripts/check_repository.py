#!/usr/bin/env python3
"""Check documents, links, fixtures, hygiene, skills and API architecture boundaries.

This does not validate application behavior, YAML schemas or audio quality.
Skill checks cover structure only: portable frontmatter, Codex metadata, the
README index, reachable references and Claude Code symlinks.
Architecture checks inspect source dependencies and named SQL tables, not runtime behavior.
It uses only Python's standard library and Git.
"""

import json
import os
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]
REQUIRED = (
    "README.md",
    "AGENTS.md",
    "CLAUDE.md",
    "CONTRIBUTING.md",
    "docs/README.md",
    "docs/product/brief.md",
    "docs/product/business-rules.md",
    "docs/product/production-pipeline.md",
    "docs/architecture/overview.md",
    "docs/architecture/script-ir.md",
    "docs/architecture/mobile.md",
    "docs/design/ui-system.md",
    "docs/work-plan/README.md",
    "templates/README.md",
    "packages/contracts/examples/episode-draft.json",
    "docs/architecture/repository-layout.md",
    "docs/decisions/0009-monorepo-layout.md",
    "apps/README.md",
    "apps/cantos-forge/README.md",
    "apps/cantos-theatre/web/README.md",
    "apps/cantos-theatre/mobile/README.md",
    "apps/cantos-studio/web/README.md",
    "services/cantos-api/README.md",
    "services/kanidm/README.md",
    "packages/README.md",
    "packages/ui-system/README.md",
    ".agents/skills/README.md",
    ".github/pull_request_template.md",
    ".github/ISSUE_TEMPLATE/config.yml",
    ".github/workflows/repository-checks.yml",
)
TEXT_SUFFIXES = {".md", ".py", ".json", ".yml", ".yaml", ".example"}
TEXT_NAMES = {".gitignore", ".editorconfig"}
LINK = re.compile(r"!?\[[^\]\n]*\]\(([^)\n]+)\)")
FENCE = re.compile(r"^\s*(`{3,}|~{3,})")

SKILLS_DIR = Path(".agents/skills")
CLAUDE_SKILLS_DIR = Path(".claude/skills")
SKILL_NAME = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
SKILL_NAME_MAX = 64
SKILL_DESCRIPTION_MAX = 1024
SKILL_FILE_MAX_LINES = 300
SKILL_FRONTMATTER_KEYS = ("name", "description")
SKILL_ENTRIES = {"SKILL.md", "agents", "references", "scenarios", "evaluations", "scripts"}
OPENAI_INTERFACE_FIELDS = ("display_name", "short_description", "default_prompt")
FRONTMATTER_KEY = re.compile(r"^([a-z][a-z0-9_-]*):(.*)$")
BLOCK_SCALARS = {">", ">-", "|", "|-"}
YAML_PAIR = re.compile(r'^  ([a-z_]+): "([^"\\]*)"$')

API_SOURCE = Path("services/cantos-api/src")
CONTEXT_UPSTREAM = {
    "editorial": set(),
    "ingestion": {"editorial"},
    "adaptation": {"editorial", "ingestion"},
    "production": {"editorial", "catalog", "ingestion"},
    "catalog": {"editorial", "ingestion"},
    "media": {"editorial", "catalog", "production"},
}
DOMAIN_EXTERNAL = {
    "axum", "deadpool_postgres", "tokio_postgres", "postgres", "sqlx", "diesel",
    "reqwest", "hyper", "tokio", "tower", "tower_http", "leptos", "rand", "getrandom",
}
LEGACY_API_MODULES = {
    "adaptation", "audio", "clip_probe", "clip_reuse", "http", "imports", "incremental", "jobs",
    "operator", "postgres", "production", "regeneration", "revisions", "speech_fingerprint",
    "speech_reuse", "tts", "tts_receipt", "wav_inspection", "worker",
}
HTTP_PURE_BOUNDARIES = {
    ("contexts", "editorial", "domain", "diagnostics", "validation_issues"),
    ("contexts", "production", "production_catalog"),
    ("script_ir", "read_script"),
}
TABLE_OWNER = {
    "scripts": "editorial", "script_members": "editorial",
    "script_revisions": "editorial", "script_reviews": "editorial",
    "script_review_operations": "editorial", "script_evidence": "editorial",
    "revision_evidence": "editorial", "source_records": "ingestion",
    "actors": "platform", "sessions": "platform", "cantos_migrations": "platform",
    "speech_clips": "synthesis",
    "audio_upload_intents": "media", "audio_intake_receipts": "media",
}
RUST_PATH = re.compile(r"\b[a-zA-Z_][a-zA-Z_0-9]*(?:::\s*[a-zA-Z_][a-zA-Z_0-9]*)+")
USE_TOKEN = re.compile(r"[a-zA-Z_][a-zA-Z_0-9]*|::|[{},*]")
SQL_TABLE = re.compile(
    r"\b(FROM|JOIN|INTO|UPDATE|TABLE)\s+(?:public\.)?([a-z_][a-z_0-9]*)", re.IGNORECASE,
)


def prose_only(content):
    """Skip fenced examples; paths shown as examples need not exist."""
    lines = []
    fence = None
    for line in content.splitlines():
        marker = FENCE.match(line)
        if marker:
            token = marker.group(1)
            if fence is None:
                fence = token
            elif token[0] == fence[0] and len(token) >= len(fence):
                fence = None
            continue
        if fence is None:
            lines.append(line)
    return "\n".join(lines)


def local_link_targets(path, content):
    """Yield (raw target, resolved path) for each local Markdown link outside code fences."""
    for match in LINK.finditer(prose_only(content)):
        target = match.group(1).strip().split(' "', 1)[0].strip("<>")
        parsed = urlsplit(target)
        if parsed.scheme or parsed.netloc or not parsed.path:
            continue
        relative = unquote(parsed.path)
        if relative.startswith("/"):
            yield target, ROOT / relative.lstrip("/")
        else:
            yield target, path.parent / relative


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def parse_frontmatter(content):
    """Parse the portable YAML subset used by SKILL.md frontmatter.

    Supports `key: value` pairs and block scalars (`>`, `>-`, `|`, `|-`).
    Returns (fields, errors); anything outside the subset is an error.
    """
    lines = content.split("\n")
    if not lines or lines[0] != "---":
        return {}, ["frontmatter must start on line 1 with ---"]
    try:
        end = lines.index("---", 1)
    except ValueError:
        return {}, ["frontmatter is not closed with ---"]

    fields, errors = {}, []
    index = 1
    while index < end:
        line = lines[index]
        match = FRONTMATTER_KEY.match(line)
        if not match:
            errors.append(f"unsupported frontmatter line {index + 1}: {line!r}")
            index += 1
            continue
        key, rest = match.group(1), match.group(2).strip()
        index += 1
        if rest in BLOCK_SCALARS:
            block = []
            while index < end and (lines[index].startswith(" ") or not lines[index].strip()):
                block.append(lines[index].strip())
                index += 1
            separator = " " if rest.startswith(">") else "\n"
            value = separator.join(part for part in block if part)
        else:
            value = unquote_scalar(rest)
        if key in fields:
            errors.append(f"duplicate frontmatter key: {key}")
        fields[key] = value
    return fields, errors


def unquote_scalar(text):
    quoted = len(text) >= 2 and text[0] == text[-1] and text[0] in "'\""
    return text[1:-1] if quoted else text


def parse_openai_yaml(content):
    """Parse `agents/openai.yaml` as top-level sections of double-quoted string pairs."""
    sections, errors = {}, []
    current = None
    for number, line in enumerate(content.splitlines(), 1):
        if not line.strip():
            continue
        if not line.startswith(" ") and line.endswith(":"):
            current = sections.setdefault(line[:-1], {})
            continue
        pair = YAML_PAIR.match(line)
        if current is None or not pair:
            errors.append(f"unsupported line {number}: {line!r}")
            continue
        current[pair.group(1)] = pair.group(2)
    return sections, errors


def check_skill_frontmatter(name, content):
    fields, errors = parse_frontmatter(content)
    unknown = sorted(set(fields) - set(SKILL_FRONTMATTER_KEYS))
    if unknown:
        errors.append(f"non-portable frontmatter keys: {', '.join(unknown)}")
    declared = fields.get("name", "")
    if declared != name:
        errors.append(f"frontmatter name {declared!r} must equal directory name {name!r}")
    if not SKILL_NAME.match(name) or len(name) > SKILL_NAME_MAX:
        errors.append(
            f"skill name must be lowercase words joined by single hyphens, "
            f"at most {SKILL_NAME_MAX} chars"
        )
    description = fields.get("description", "")
    if not description:
        errors.append("frontmatter description is required")
    elif len(description) > SKILL_DESCRIPTION_MAX:
        errors.append(
            f"description has {len(description)} chars; limit is {SKILL_DESCRIPTION_MAX}"
        )
    return errors


def check_openai_metadata(name, path):
    if not path.is_file():
        return ["missing agents/openai.yaml"]
    sections, errors = parse_openai_yaml(path.read_text(encoding="utf-8"))
    interface = sections.get("interface", {})
    errors += [
        f"interface.{field} is required"
        for field in OPENAI_INTERFACE_FIELDS
        if not interface.get(field)
    ]
    if f"${name}" not in interface.get("default_prompt", ""):
        errors.append(f"interface.default_prompt must invoke ${name}")
    return [f"agents/openai.yaml: {error}" for error in errors]


def unreachable_documents(skill_dir):
    """Return Markdown files in a skill that SKILL.md does not reach through local links."""
    documents = {path.resolve() for path in skill_dir.rglob("*.md")}
    entry = (skill_dir / "SKILL.md").resolve()
    reached, pending = {entry}, [entry]
    while pending:
        current = pending.pop()
        for _, target in local_link_targets(current, current.read_text(encoding="utf-8")):
            resolved = target.resolve()
            if resolved in documents and resolved not in reached:
                reached.add(resolved)
                pending.append(resolved)
    return sorted(documents - reached)


def check_skill(skill_dir):
    name = skill_dir.name
    skill_file = skill_dir / "SKILL.md"
    if not skill_file.is_file():
        return ["missing SKILL.md"]

    content = skill_file.read_text(encoding="utf-8")
    errors = check_skill_frontmatter(name, content)
    line_count = len(content.splitlines())
    if line_count > SKILL_FILE_MAX_LINES:
        errors.append(
            f"SKILL.md has {line_count} lines; move depth into references/ "
            f"(limit {SKILL_FILE_MAX_LINES})"
        )
    errors += check_openai_metadata(name, skill_dir / "agents" / "openai.yaml")
    for entry in sorted(child.name for child in skill_dir.iterdir()):
        if entry not in SKILL_ENTRIES:
            allowed = ", ".join(sorted(SKILL_ENTRIES))
            errors.append(f"unexpected entry {entry!r}; allowed: {allowed}")
    for document in unreachable_documents(skill_dir):
        relative = document.relative_to(skill_dir.resolve())
        errors.append(f"{relative} is not linked from SKILL.md or its references")
    return errors


def check_claude_links(root, names):
    """Each skill needs `.claude/skills/<name>` -> `../../.agents/skills/<name>`, nothing else."""
    claude_dir = root / CLAUDE_SKILLS_DIR
    if claude_dir.is_symlink() or not claude_dir.is_dir():
        return [f"{CLAUDE_SKILLS_DIR} must be a real directory of per-skill symlinks"]
    errors = []
    for name in names:
        link = claude_dir / name
        expected = f"../../{SKILLS_DIR.as_posix()}/{name}"
        if not link.is_symlink():
            errors.append(f"{CLAUDE_SKILLS_DIR / name}: missing symlink to {expected}")
        elif os.readlink(link) != expected:
            actual = os.readlink(link)
            errors.append(f"{CLAUDE_SKILLS_DIR / name}: points to {actual!r}, expected {expected!r}")
    for entry in sorted(claude_dir.iterdir()):
        if entry.name not in names:
            errors.append(f"{CLAUDE_SKILLS_DIR / entry.name}: no matching skill in {SKILLS_DIR}")
    return errors


def skill_directories(root):
    skills_dir = root / SKILLS_DIR
    return sorted(
        path for path in skills_dir.iterdir()
        if path.is_dir() and not path.name.startswith(".")
    )


def indexed_skill_files(index):
    if not index.is_file():
        return set()
    content = index.read_text(encoding="utf-8")
    return {target.resolve() for _, target in local_link_targets(index, content)}


def check_skills(root=ROOT):
    """Validate every skill under .agents/skills plus its index entry and Claude symlink."""
    if not (root / SKILLS_DIR).is_dir():
        return [f"missing skills directory: {SKILLS_DIR}"]
    skill_dirs = skill_directories(root)

    errors = []
    for skill_dir in skill_dirs:
        errors += [f"{SKILLS_DIR / skill_dir.name}: {error}" for error in check_skill(skill_dir)]

    indexed = indexed_skill_files(root / SKILLS_DIR / "README.md")
    for skill_dir in skill_dirs:
        if (skill_dir / "SKILL.md").resolve() not in indexed:
            errors.append(f"{SKILLS_DIR / 'README.md'}: does not link {skill_dir.name}/SKILL.md")

    return errors + check_claude_links(root, [path.name for path in skill_dirs])


def rust_code(content):
    """Mask comments/literals without shifting lines; retain strings for SQL checks.

    Rust block comments nest and raw strings may contain quotes. Character literals are
    removed, while lifetimes stay intact. This is a lexical check, not a Rust parser.
    """
    code, strings, offset = list(content), [], 0
    while offset < len(content):
        start = offset
        raw = re.match(r'(?:br|r)(#*)"', content[offset:])
        if content.startswith("//", offset):
            end = content.find("\n", offset)
            offset = len(content) if end < 0 else end
        elif content.startswith("/*", offset):
            offset, depth = offset + 2, 1
            while offset < len(content) and depth:
                if content.startswith("/*", offset):
                    depth, offset = depth + 1, offset + 2
                elif content.startswith("*/", offset):
                    depth, offset = depth - 1, offset + 2
                else:
                    offset += 1
        elif raw:
            body = offset + len(raw.group())
            close = '"' + raw.group(1)
            end = content.find(close, body)
            offset = len(content) if end < 0 else end + len(close)
            strings.append((start, content[body:end if end >= 0 else len(content)]))
        elif content[offset] == '"':
            offset += 1
            while offset < len(content):
                if content[offset] == "\\":
                    offset += 2
                elif content[offset] == '"':
                    offset += 1
                    break
                else:
                    offset += 1
            strings.append((start, content[start + 1:offset - 1]))
        elif content[offset] == "'" and (char := re.match(r"'(?:\\.|[^'\\\n])'", content[offset:])):
            offset += len(char.group())
        else:
            offset += 1
            continue
        for index in range(start, min(offset, len(content))):
            if code[index] != "\n":
                code[index] = " "
    return "".join(code), strings


def use_paths(tokens, prefix=()):
    """Expand grouped `use` trees into paths, including nested groups and aliases."""
    paths, index = [], 0
    while index < len(tokens):
        path = list(prefix)
        while index < len(tokens) and tokens[index] not in {"{", "}", ",", "as"}:
            if tokens[index] != "::":
                path.append(tokens[index])
            index += 1
        if index < len(tokens) and tokens[index] == "{":
            start, depth = index + 1, 1
            index += 1
            while index < len(tokens) and depth:
                depth += (tokens[index] == "{") - (tokens[index] == "}")
                index += 1
            paths.extend(use_paths(tokens[start:index - 1], tuple(path)))
        else:
            path = tuple(path[:-1] if path[-1:] == ["self"] else path)
            alias = path[-1] if path else None
            if index < len(tokens) and tokens[index] == "as":
                alias = tokens[index + 1]
                index += 2
            paths.append((path, alias))
        if index < len(tokens) and tokens[index] in {",", "}"}:
            index += 1
    return paths


def source_paths(code, module):
    """Yield source offsets and dependency paths, resolving crate/self/super prefixes."""
    uses = list(re.finditer(r"\buse\s+([^;]+);", code))
    paths = [(match.start(), tuple(re.split(r"::\s*", match.group())))
             for match in RUST_PATH.finditer(code)
             if not any(use.start() <= match.start() < use.end() for use in uses)]
    aliases = {}
    for match in uses:
        for path, alias in use_paths(USE_TOKEN.findall(match.group(1))):
            paths.append((match.start(), path))
            if alias and alias != "*":
                aliases[alias] = path
    modules, braces = [], []
    for match in re.finditer(r"[{}]", code):
        if match.group() == "{":
            declaration = re.search(r"\bmod\s+(\w+)\s*$", code[:match.start()])
            braces.append((match.start(), declaration.group(1) if declaration else None))
        elif braces:
            start, name = braces.pop()
            if name:
                modules.append((start, match.start(), name))
    for offset, path in sorted(set(paths)):
        if not path:
            continue
        for _ in range(len(aliases)):
            replacement = aliases.get(path[0])
            if not replacement or replacement == (path[0],) or path[0] in {"crate", "super", "self"}:
                break
            path = replacement + path[1:]
        if path[0] == "crate":
            path = path[1:]
            if path and path[0] in LEGACY_API_MODULES:
                path = ("<legacy>",) + path
        elif path[0] in {"self", "super"}:
            base = list(module) + [name for start, end, name in sorted(modules)
                                   if start < offset < end]
            while path and path[0] in {"self", "super"}:
                if path[0] == "super" and base:
                    base.pop()
                path = path[1:]
            path = tuple(base) + path
        yield offset, path


def table_owner(table):
    for prefix, owner in (("adaptation_", "adaptation"), ("production_", "production"),
                          ("catalog_", "catalog"), ("audio_", "media"),
                          ("durable_", "synthesis")):
        if table.startswith(prefix):
            return owner
    return TABLE_OWNER.get(table)


def dependency_error(path, owner, domain, http):
    """Return the boundary violated by one path, or None for an admitted dependency."""
    if not path:
        return None
    if (owner or domain or http) and path[0] == "<legacy>":
        return "context uses a legacy crate alias; name the owning context explicitly"
    if owner and path[0] == "interfaces":
        return "context must not depend on an interface"
    if domain:
        if path[0] in DOMAIN_EXTERNAL or path[:2] in {
            ("std", "fs"), ("std", "net"), ("std", "process"), ("std", "env"),
            ("std", "thread"), ("std", "time"), ("std", "os"),
        } or path[-1] in {"new_v4", "new_v7", "now_v7", "thread_rng", "random"}:
            return "pure domain must not depend on I/O, a runtime, clock or randomness"
        if path[:2] == ("std", "io") and len(path) > 2 and path[2] in {"stdin", "stdout", "stderr"}:
            return "pure domain must not depend on console I/O"
        if path[0] == "platform" and path[:2] != ("platform", "ids"):
            return "pure domain may use only the pure platform::ids boundary"
        if path[0] == "interfaces":
            return "pure domain must not depend on an interface"
    if path[0] == "contexts" and len(path) >= 2:
        target = path[1]
        credential_inquiry = owner == "production" and not domain and path == ("contexts", "media", "upload_token_registered")
        if owner and target != owner and target not in CONTEXT_UPSTREAM[owner] and not credential_inquiry:
            return f"{owner} must not depend on {target}; upstream contexts are {sorted(CONTEXT_UPSTREAM[owner])}"
        if domain and (len(path) < 3 or path[2] != "domain"):
            return "pure domain must name a context's domain directly, without an adapter or facade"
        if owner and target != owner and len(path) >= 3 and path[2] in {
            "postgres", "application", "providers", "media", "worker",
        }:
            return "cross-context calls must use the owning context's entrypoint"
        # Catalog's pure wire facade admits raw DTOs and formats immutable outcomes.
        # The HTTP shell may map typed errors, but cannot invoke a domain transition or SQL.
        catalog_wire = path[:3] == ("contexts", "catalog", "wire")
        catalog_error = path[:3] == ("contexts", "catalog", "CatalogStoreError") or path[:4] == (
            "contexts", "catalog", "domain", "CatalogError")
        catalog_input_type = path in {
            ("contexts", "catalog", "CatalogCommand"),
            ("contexts", "catalog", "CatalogCursor"),
            ("contexts", "catalog", "CatalogCursor", "decode"),
        }
        publication_error = path[:3] in {
            ("contexts", "media", "PublicationStoreError"),
            ("contexts", "media", "PublicationDependencyError"),
            ("contexts", "media", "ListenerError"),
        } or path[:5] == ("contexts", "media", "domain", "publication", "PublicationError")
        publication_composition = path in {
            ("contexts", "media", "PublicationService"),
            ("contexts", "media", "PublicationService", "new"),
            ("contexts", "media", "PublicationService", "with_publication_delivery"),
            ("contexts", "media", "PublicationDelivery"),
            ("contexts", "media", "ListenerService"),
            ("contexts", "media", "ListenerService", "new"),
            ("contexts", "media", "ListenerError"),
            ("contexts", "media", "PlaybackDelivery"),
        }
        if http and path not in HTTP_PURE_BOUNDARIES and not (
            catalog_wire or catalog_error or catalog_input_type or publication_error or publication_composition
        ):
            return "HTTP must delegate business decisions through application use cases"
    if http and path[0] in DOMAIN_EXTERNAL - {"axum", "tokio", "tower", "tower_http"}:
        return "HTTP must not call a database or external provider directly"
    if http and path[:2] == ("platform", "postgres") and len(path) >= 3 and path[2] not in {
        "Store", "StoreError",
    }:
        return "HTTP may import only Store and StoreError from PostgreSQL plumbing"
    if http and path[0] == "script_ir" and path not in HTTP_PURE_BOUNDARIES:
        return "HTTP may admit Script IR bytes but must delegate business decisions"
    if domain and not owner and path[0] == "contexts":
        return "Script IR shared kernel must not depend on a business context"
    return None


def check_api_architecture(root=ROOT):
    """Check explicit Rust paths, context visibility and statically named SQL tables.

    Compiler visibility completes the check for imported aliases; this deliberately does not
    claim to resolve macros, dynamic SQL or prove the absence of arbitrary business logic.
    """
    errors = set()
    for file in sorted((root / API_SOURCE).rglob("*.rs")):
        relative = file.relative_to(root / API_SOURCE)
        parts = relative.parts
        owner = parts[1] if parts[0] == "contexts" and len(parts) > 2 else None
        if owner and owner not in CONTEXT_UPSTREAM:
            errors.add(f"{API_SOURCE / relative}: new context needs an explicit dependency policy")
            continue
        domain = parts[0] in {"script_ir", "script_ir.rs"} or (
            owner and parts[2] in {"domain", "domain.rs"}
        )
        domain = domain or relative == Path("platform/ids.rs")
        http = parts[:2] == ("interfaces", "http")
        module = parts[:-1] + (() if file.stem == "mod" else (file.stem,))
        content = file.read_text(encoding="utf-8")
        code, strings = rust_code(content)
        for offset, path in source_paths(code, module):
            error = dependency_error(path, owner, domain, http)
            if parts[0] == "platform" and path[0] == "contexts":
                error = "platform must not depend on a business context"
            if error:
                line = code.count("\n", 0, offset) + 1
                errors.add(f"{API_SOURCE / relative}:{line}: {'::'.join(path)}: {error}")
        if domain and re.search(r"\b(?:print|println|eprint|eprintln|dbg)\s*!", code):
            errors.add(f"{API_SOURCE / relative}: pure domain must not write to the console")
        if owner and relative == Path(f"contexts/{owner}/mod.rs"):
            for match in re.finditer(r"\bpub(?:\s*\([^)]*\))?\s+mod\s+(postgres|application)\b", code):
                errors.add(f"{API_SOURCE / relative}: {match.group(1)} must remain private")
        if http:
            for match in re.finditer(
                r"\.\s*(connection|transaction|query|query_one|query_opt|execute|batch_execute)\s*\(", code,
            ):
                errors.add(f"{API_SOURCE / relative}: HTTP must not execute SQL or open transactions")
        for offset, literal in strings:
            for match in SQL_TABLE.finditer(literal):
                table = match.group(2).lower()
                target = table_owner(table)
                if not target:
                    continue
                # This legacy read proves the source link and bytes together under editorial's
                # already-held script lock. No other cross-context table access is admitted.
                legacy_link = (relative.as_posix(), match.group(1).upper(), table) in {
                    ("contexts/ingestion/postgres.rs", "FROM", "revision_evidence"),
                    ("contexts/editorial/postgres.rs", "JOIN", "actors"),
                }
                in_adapter = owner and len(parts) > 2 and parts[2] in {"postgres", "postgres.rs"}
                if http or domain or (owner and not in_adapter):
                    line = content.count("\n", 0, offset) + 1
                    errors.add(f"{API_SOURCE / relative}:{line}: SQL table access belongs in the PostgreSQL adapter")
                elif owner and target != owner and not legacy_link:
                    line = content.count("\n", 0, offset) + 1
                    errors.add(f"{API_SOURCE / relative}:{line}: SQL table {table} belongs to {target}")
    return sorted(errors)


def check():
    errors = []
    for name in REQUIRED:
        if not (ROOT / name).is_file():
            errors.append(f"missing required file: {name}")

    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"],
        cwd=ROOT, capture_output=True, check=True,
    )
    names = sorted(set(result.stdout.decode("utf-8").rstrip("\0").split("\0")))
    checked = 0
    for name in filter(None, names):
        path = ROOT / name
        if path.suffix not in TEXT_SUFFIXES and path.name not in TEXT_NAMES:
            continue
        if not path.is_file():
            errors.append(f"missing tracked file: {name}")
            continue
        try:
            content = path.read_bytes().decode("utf-8")
        except UnicodeDecodeError:
            errors.append(f"{name}: expected UTF-8 text")
            continue
        checked += 1
        if not content.endswith("\n"):
            errors.append(f"{name}: missing final newline")
        if "\r" in content:
            errors.append(f"{name}: expected LF line endings")
        for number, line in enumerate(content.splitlines(), 1):
            # Markdown two-space hard breaks are intentional.
            trailing = line[len(line.rstrip()):]
            if trailing and not (path.suffix == ".md" and trailing == "  "):
                errors.append(f"{name}:{number}: trailing whitespace")

        if path.suffix == ".json":
            try:
                json.loads(content, object_pairs_hook=unique_object)
            except (ValueError, json.JSONDecodeError) as exc:
                errors.append(f"{name}: invalid JSON: {exc}")

        if path.suffix != ".md":
            continue
        for target, destination in local_link_targets(path, content):
            if not destination.exists():
                errors.append(f"{name}: broken local link: {target}")

    errors += check_skills(ROOT)
    errors += check_api_architecture(ROOT)

    # The two diagnostics share one declaration source. Registration never means
    # runtime/design approval; new app imports need fresh evidence on both renderers.
    from ui_preview_catalog import CATALOG, outputs, repository_failures, validate
    from ui_tokens import SOURCE, check_outputs, load_source, validate as validate_tokens
    try:
        catalog = validate(load_source(CATALOG.read_bytes()))
        tokens = validate_tokens(load_source(SOURCE.read_bytes()))
        errors += check_outputs(outputs(catalog, tokens))
        errors += repository_failures(catalog, tokens)
    except (OSError, ValueError) as error:
        errors.append(f"preview catalog: {error}")

    if errors:
        print("Repository checks failed:", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1
    skills = len(skill_directories(ROOT))
    print(
        f"Repository checks passed: {checked} text files and {skills} skills; "
        "local links, JSON syntax, skill structure and API architecture valid."
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(check())
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"Unable to check repository: {exc}", file=sys.stderr)
        sys.exit(1)
