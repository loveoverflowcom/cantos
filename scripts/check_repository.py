#!/usr/bin/env python3
"""Check documents, links, fixtures, hygiene, skills and Server/Identity/Theatre boundaries.

This does not validate application behavior, YAML schemas or audio quality.
Skill checks cover structure only: portable frontmatter, Codex metadata, the
README index, reachable references, Claude Code symlinks and shared-instruction bootstrap.
Architecture checks inspect source dependencies (Rust paths, Kotlin imports), workspace Cargo
manifests and named SQL tables, not runtime behavior: they do not prove OS/file isolation,
one-owner locking or the absence of cross-database reads at runtime.
It uses only Python's standard library (3.11+ for `tomllib`) and Git.
"""

import argparse
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
    "services/cantos-server/contracts/studio/script-ir/examples/episode-draft.json",
    "docs/architecture/repository-layout.md",
    "docs/decisions/0009-monorepo-layout.md",
    "apps/README.md",
    "services/cantos-forge-functions/README.md",
    "apps/cantos-theatre/web/README.md",
    "apps/cantos-theatre/mobile/README.md",
    "apps/cantos-studio/web/README.md",
    "services/cantos-server/README.md",
    "services/cantos-identity/README.md",
    "packages/store-turso/README.md",
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

API_SOURCE = Path("services/cantos-server/src")
# The pre-#332 crate path. A manifest here would be a second canonical server implementation.
LEGACY_API_CRATE = Path("services/cantos-api")
CONTEXT_UPSTREAM = {
    "editorial": set(),
    "ingestion": {"editorial"},
    "adaptation": {"editorial", "ingestion"},
    "production": {"editorial", "catalog", "ingestion"},
    "catalog": {"editorial", "ingestion"},
    "media": {"editorial", "catalog", "production"},
    "listening": {"catalog", "media"},
}
# Database drivers by backend. PostgreSQL is the legacy authority; Turso is the #331 target,
# reached through the shared `cantos-store-turso` owner (SQLite-family engines would compete
# with it). Neither backend is a pure dependency.
POSTGRES_DRIVERS = {"deadpool_postgres", "tokio_postgres", "postgres", "sqlx", "diesel"}
TURSO_DRIVERS = {"turso", "turso_core", "cantos_store_turso", "libsql", "rusqlite"}
PERSISTENCE_DRIVERS = POSTGRES_DRIVERS | TURSO_DRIVERS
# The private Identity RPC transport runtime. Identity owns `cantos.identity.v1`; the Server
# generates its own client binding into the platform Identity adapter (`IDENTITY_RPC_CLIENT`),
# and only that adapter names the runtime or the binding.
RPC_RUNTIME = {"tonic", "tonic_prost"}
IDENTITY_RPC_ADAPTER = ("platform", "identity_auth")
IDENTITY_RPC_CLIENT = ("platform", "identity_auth", "rpc")
# prost encodes the Identity client binding and the Server's Studio Protobuf pilot edge codec.
PROST = "prost"
# The Server's own representation of the HTTP contracts it publishes. It is a leaf: canonical
# schemas live in services/cantos-server/contracts and no client imports this module.
SERVER_CONTRACT = "contract"
STUDIO_PROTOBUF_CODEC = (SERVER_CONTRACT, "studio", "protobuf")
SERVER_CONTRACT_FORBIDDEN = {"contexts", "adapters", "application", "platform", "script_ir"}
# The retired central contract crate; canonical contracts belong to their defining service.
RETIRED_CONTRACT_PACKAGE = Path("packages/contracts")
DOMAIN_EXTERNAL = PERSISTENCE_DRIVERS | {
    "axum", "reqwest", "hyper", "tokio", "tower", "tower_http", "leptos", "rand", "getrandom",
}
# Crates the source checks recognize by name; a manifest or `extern crate` rename would hide them.
GUARDED_CRATES = DOMAIN_EXTERNAL | RPC_RUNTIME | {PROST, "cantos_identity", "cantos_server"}
DOMAIN_STD_EFFECTS = {
    ("std", "fs"), ("std", "net"), ("std", "process"), ("std", "env"),
    ("std", "thread"), ("std", "time"), ("std", "os"),
}
# Context-owned persistence adapters sit directly under their context, one module per backend:
# contexts/<owner>/<backend>.rs or contexts/<owner>/<backend>/**. A backend name anywhere else
# in a path (application/turso/, domain/postgres.rs, an HTTP or top-level turso/ module) is not
# an adapter, so the name alone never admits SQL.
PERSISTENCE_BACKENDS = ("postgres", "turso")
# Platform plumbing owns connections/transactions. HTTP keeps only the reviewed Store/StoreError
# facade (current platform::store and historical platform::postgres), never statement/row/engine types.
PERSISTENCE_PLUMBING = {("platform", backend) for backend in (*PERSISTENCE_BACKENDS, "store")}
HTTP_PLUMBING_FACADE = {
    ("platform", module, name)
    for module in ("postgres", "store") for name in ("Store", "StoreError")
}
# The legacy trusted operator seed writes Editorial evidence and Ingestion source rows from the
# platform tool. These exact (file, verb, table) pairs are the only platform access to a context
# table; every statement matching a pair is admitted.
PLATFORM_CONTEXT_SQL = {
    ("platform/operator.rs", "INTO", "script_evidence"),
    ("platform/operator.rs", "INTO", "source_records"),
}
# Cantos Identity owns `identity.db`. Its tables are those its persistence adapter creates; the
# Server never names them and Identity never names Server tables. Only the RPC adapter and the
# process composition hold the generated contract and Tonic.
IDENTITY_SOURCE = Path("services/cantos-identity/src")
IDENTITY_PERSISTENCE = "persistence"
IDENTITY_RPC_MODULES = {"rpc", "server"}
# Workspace members whose manifests are checked. Only the shared store owner depends on the
# Turso engine; a service depends on another service only for tests, never at runtime.
WORKSPACE_MANIFEST = Path("Cargo.toml")
TURSO_OWNER = Path("packages/store-turso")
TURSO_ENGINE_CRATES = {"turso", "turso_core", "libsql", "rusqlite"}
LEGACY_API_MODULES = {
    "adaptation", "audio", "clip_probe", "clip_reuse", "http", "imports", "incremental", "jobs",
    "operator", "postgres", "production", "regeneration", "revisions", "speech_fingerprint",
    "speech_reuse", "tts", "tts_receipt", "wav_inspection", "worker",
}
HTTP_PURE_BOUNDARIES = {
    ("contexts", "editorial", "domain", "diagnostics", "validation_issues"),
    # Editorial's facts/shape-only projections are not revision decisions or SQL access.
    ("contexts", "editorial", "RevisionOperation"),
    ("contexts", "editorial", "RevisionFacts"),
    ("contexts", "editorial", "RevisionHistory"),
    ("contexts", "editorial", "ReviewFacts"),
    ("contexts", "editorial", "document"),
    ("contexts", "editorial", "document", "revision"),
    ("contexts", "editorial", "document", "issues"),
    ("contexts", "production", "production_catalog"),
    ("script_ir", "read_script"),
}
# Media's concrete private-read facade admits HTTP ranges and exposes only application scope,
# safe errors and a bounded byte stream. This never permits direct domain/adapter dependencies.
MEDIA_READ_HTTP_TYPES = {
    "AudioReadScope", "ByteRange", "NormalizedByteRange", "RangeError", "StorageCapability", "StorageError",
    "StorageRead", "StorageStream",
}
TABLE_OWNER = {
    "scripts": "editorial", "script_members": "editorial",
    "script_revisions": "editorial", "script_reviews": "editorial",
    "script_review_operations": "editorial", "script_evidence": "editorial",
    "revision_evidence": "editorial", "source_records": "ingestion",
    "actors": "platform", "sessions": "platform", "cantos_migrations": "platform", "cantos_schema": "platform",
    "actor_external_identities": "platform",
    "sample_seed_principals": "platform",
    "fictional_seed_principals": "platform",
    "bearer_flows": "platform", "bearer_families": "platform", "bearer_tokens": "platform",
    "identity_sessions": "platform", "identity_session_tokens": "platform",
    "admin_audit": "platform", "admin_operations": "platform", "admin_access_revocations": "platform",
    "studio_oidc_flows": "platform", "studio_oidc_sessions": "platform",
    "theatre_identity_flows": "platform", "theatre_identity_sessions": "platform",
    "theatre_native_flows": "platform", "theatre_native_sessions": "platform",
    **{table: "platform" for table in ("theatre_registrations", "theatre_registration_mail_starts", "theatre_firebase_enrollments", "theatre_firebase_enrollment_receipts", "theatre_firebase_send_starts")},
    "audio_upload_intents": "media", "audio_intake_receipts": "media",
    "media_image_intents": "media", "media_image_assets": "media",
    "media_work_artwork_changes": "media", "media_artwork_operations": "media",
    "media_cover_disclosures": "media",
    **{table: "listening" for table in ("listener_state", "listener_saved", "listener_progress", "listener_history", "listener_playback_sessions", "listener_device_sequences", "listener_operations", "listener_bookmarks")},
}
RUST_PATH = re.compile(r"\b[a-zA-Z_][a-zA-Z_0-9]*(?:::\s*[a-zA-Z_][a-zA-Z_0-9]*)+")
USE_TOKEN = re.compile(r"[a-zA-Z_][a-zA-Z_0-9]*|::|[{},*]")
# Schema qualifiers (`public.`, `main.`, an attached alias), `IF [NOT] EXISTS`, `ONLY`, quoted or
# bracketed names (escaped inside ordinary Rust strings), `USING`/`TRUNCATE` and DDL targets
# (`REFERENCES`, `CREATE INDEX/TRIGGER ... ON`) must not hide a table from the ownership checks.
# Comma-separated FROM lists and dynamic SQL remain review obligations.
SQL_NAME = r"(?:\\?[\"`\[])?([a-z_][a-z_0-9]*)(?:\\?[\"`\]])?"
SQL_QUALIFIER = r"(?:(?:\\?[\"`\[])?[a-z_][a-z_0-9]*(?:\\?[\"`\]])?\.)?"
SQL_TABLE = re.compile(
    rf"\b(FROM|JOIN|INTO|UPDATE|TABLE|REFERENCES|USING|TRUNCATE)\s+(?:TABLE\s+)?(?:ONLY\s+)?"
    rf"(?:IF\s+(?:NOT\s+)?EXISTS\s+)?{SQL_QUALIFIER}{SQL_NAME}",
    re.IGNORECASE,
)
SQL_DDL_TARGET = re.compile(rf"\b(?:INDEX|TRIGGER)\b[^;]*?\b(ON)\s+(?:ONLY\s+)?{SQL_QUALIFIER}{SQL_NAME}", re.IGNORECASE)
# `TABLE` preceded by one of these defines a table rather than naming one in prose.
SQL_TABLE_DEFINITION = re.compile(r"\b(?:CREATE|ALTER|DROP)\s+(?:(?:TEMP|TEMPORARY|VIRTUAL|UNLOGGED)\s+)?\Z", re.IGNORECASE)
# Each service owns one database file; attaching another would read across owners.
SQL_ATTACH = re.compile(r"\bATTACH\s+(?:DATABASE\b|\\?['\"]|[?:$@])", re.IGNORECASE)
# The Turso schema registry registers one DDL file per version straight under this directory.
# `server.db` is one database, so its DDL declares the tables of several contexts and the foreign
# keys between them, as the PostgreSQL schema did; queries over those tables still belong to
# their owner's persistence adapter and are checked there. DDL may not ATTACH another database.
TURSO_SCHEMA_DDL = Path("platform/turso")
# Fixture modules compiled only for tests (`cfg(test)` or the `test-support` feature): they seed
# rows across contexts to build a world and are never reachable from a runtime build.
TEST_FIXTURES = Path("test_support")


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


def check_claude_bootstrap(root):
    """Keep Claude's root memory connected to the canonical repository instructions."""
    memory = root / "CLAUDE.md"
    if not memory.is_file():
        return ["CLAUDE.md: missing shared-instruction bootstrap"]
    lines = prose_only(memory.read_text(encoding="utf-8")).splitlines()
    errors = []
    if "@AGENTS.md" not in lines:
        errors.append("CLAUDE.md: must import @AGENTS.md on its own prose line")
    if not (root / "AGENTS.md").is_file():
        errors.append("CLAUDE.md: imported AGENTS.md is missing")
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

    errors += check_claude_bootstrap(root)
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


def source_paths(code, module, legacy=LEGACY_API_MODULES):
    """Yield source offsets and dependency paths, resolving crate/self/super prefixes.

    A bare path whose head is a module declared in the same module scope (`mod turso;`) names
    that child module, not an extern crate of the same name; a leading `::` keeps the extern
    crate. `extern crate X [as Y]` is a dependency on X at that line.
    """
    uses = list(re.finditer(r"\buse\s+([^;]+);", code))
    paths = [(match.start(), tuple(re.split(r"::\s*", match.group())),
              code[max(0, match.start() - 2):match.start()] == "::")
             for match in RUST_PATH.finditer(code)
             if not any(use.start() <= match.start() < use.end() for use in uses)]
    aliases = {}
    for match in uses:
        absolute = match.group(1).lstrip().startswith("::")
        for path, alias in use_paths(USE_TOKEN.findall(match.group(1))):
            paths.append((match.start(), path, absolute))
            if alias and alias != "*":
                aliases[alias] = path
    for match in re.finditer(r"\bextern\s+crate\s+(?!self\b)(\w+)", code):
        paths.append((match.start(), (match.group(1),), True))
    modules, braces = [], []
    for match in re.finditer(r"[{}]", code):
        if match.group() == "{":
            declaration = re.search(r"\bmod\s+(\w+)\s*$", code[:match.start()])
            braces.append((match.start(), declaration.group(1) if declaration else None))
        elif braces:
            start, name = braces.pop()
            if name:
                modules.append((start, match.start(), name))

    def scope(offset):
        return tuple(name for start, end, name in sorted(modules) if start < offset < end)

    declared = {(scope(match.start()), match.group(1))
                for match in re.finditer(r"\bmod\s+(\w+)\s*[;{]", code)}
    for offset, path, absolute in sorted(set(paths)):
        if not path:
            continue
        for _ in range(0 if absolute else len(aliases)):
            replacement = aliases.get(path[0])
            if not replacement or replacement == (path[0],) or path[0] in {"crate", "super", "self"}:
                break
            path = replacement + path[1:]
        if path[0] == "crate":
            path = path[1:]
            if path and path[0] in legacy:
                path = ("<legacy>",) + path
        elif (scope(offset), path[0]) in declared and not absolute and path[0] not in aliases:
            path = tuple(module) + scope(offset) + path
        elif path[0] in {"self", "super"}:
            base = list(module) + list(scope(offset))
            while path and path[0] in {"self", "super"}:
                if path[0] == "super" and base:
                    base.pop()
                path = path[1:]
            path = tuple(base) + path
        if path:
            yield offset, path


def renamed_crates(code):
    """Yield (offset, crate) for `extern crate X as Y` renames of a guarded crate."""
    for match in re.finditer(r"\bextern\s+crate\s+(\w+)\s+as\s+(\w+)", code):
        if match.group(1) in GUARDED_CRATES and match.group(2) != match.group(1):
            yield match.start(), match.group(1)


def without_sql_comments(text):
    return re.sub(r"--[^\n]*", lambda match: " " * len(match.group()), text)


def source_texts(file, content):
    """Return the Rust code and the (offset, text, inline) texts searched for SQL tables.

    Rust sources contribute their string literals and the files they `include_str!` (or
    `include_bytes!` for `.sql`), judged as the including file at the macro's offset; `inline`
    is False for those, whose offsets do not index `content`. A `.sql` file is also searched
    where it sits. Paths built by macros (`concat!`, `env!`) are not resolved.
    """
    if file.suffix == ".sql":
        return "", [(0, without_sql_comments(content), True)]
    code, strings = rust_code(content)
    texts = [(offset, text, True) for offset, text in strings]
    literals = dict(strings)
    for match in re.finditer(r"\binclude_(str|bytes)!\s*\(", code):
        # The literal itself is masked in `code`; find it in the original text.
        start = match.end() + len(content[match.end():]) - len(content[match.end():].lstrip())
        target = literals.get(start)
        included = file.parent / target if target else None
        if not included or not included.is_file() or (
            match.group(1) == "bytes" and included.suffix != ".sql"
        ):
            continue
        try:
            text = included.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError):
            continue
        texts.append((match.start(), without_sql_comments(text) if included.suffix == ".sql" else text, False))
    return code, texts


def sql_tables(text):
    """Yield (offset, verb, table, defines) per statically named table in one SQL text.

    `defines` marks `CREATE/ALTER/DROP ... TABLE`, as opposed to a name used in a statement.
    """
    # Rust escapes and line continuations become equal-length blanks, keeping offsets.
    text = re.sub(r"\\(?:(?=\n)|[nrt])", lambda match: " " * len(match.group()), text)
    for pattern in (SQL_TABLE, SQL_DDL_TARGET):
        for match in pattern.finditer(text):
            verb = match.group(1).upper()
            defines = verb == "TABLE" and bool(SQL_TABLE_DEFINITION.search(text, 0, match.start()))
            yield match.start(), verb, match.group(2).lower(), defines


def sql_line(content, offset, start, inline):
    """Return the 1-based line of a table name, or of the include macro for included text."""
    return content.count("\n", 0, offset + (start if inline else 0)) + 1


def sources(directory):
    """Return the Rust and SQL files under a source directory, in a stable order."""
    return sorted(directory.rglob("*.rs")) + sorted(directory.rglob("*.sql"))


def rust_module(relative):
    """Return the module path of a source file relative to its crate's `src`."""
    parts = relative.parts
    if parts in {("lib.rs",), ("main.rs",)}:
        return ()
    return parts[:-1] + (() if relative.stem == "mod" else (relative.stem,))


def table_owner(table):
    for prefix, owner in (("adaptation_", "adaptation"), ("production_", "production"),
                          ("catalog_", "catalog"), ("audio_", "media"),
                          ("durable_", "synthesis")):
        if table.startswith(prefix):
            return owner
    return TABLE_OWNER.get(table)


HTTP_STORE_METHODS = {"load", "save", "history", "review"}
STORE_PATHS = {("application", "Store"), ("platform", "postgres", "Store"),
               ("platform", "store", "Store")}


def braced_body(code, opening):
    depth = 1
    for match in re.finditer(r"[{}]", code[opening + 1:]):
        depth += 1 if match.group() == "{" else -1
        if depth == 0:
            return code[opening + 1:opening + 1 + match.start()]
    return ""


def http_store_method_calls(code):
    """Find direct typed receivers/impls; this lexical check is not Rust type inference.

    Inherent HTTP compatibility methods have no HTTP import at the callsite. Recognize
    imported Store aliases, typed parameters and self inside their impl blocks, while leaving
    context-owned service.history and atomic.load alone. Macros and inferred receiver aliases
    remain review obligations, as with the other source-path checks.
    """
    names = {"Store"}
    for match in re.finditer(r"\buse\s+([^;]+);", code):
        for path, alias in use_paths(USE_TOKEN.findall(match.group(1))):
            if path[:1] == ("crate",) and path[1:] in STORE_PATHS and alias != "*":
                names.add(alias)
    types = "(?:" + "|".join(sorted(names)) + ")"
    methods = "(?:" + "|".join(sorted(HTTP_STORE_METHODS)) + ")"
    scopes = []
    for match in re.finditer(r"\bimpl\s+" + types + r"\s*\{", code):
        scopes.append((match.end(), {"self"}))
    for match in re.finditer(r"\bfn\s+\w+\s*(?:<[^{};]*>)?\s*\(([^{};]*)\)[^{;]*\{", code):
        receivers = set(re.findall(
            r"\b(\w+)\s*:\s*(?:&\s*(?:'\w+\s+)?(?:mut\s+)?)?"
            r"(?:\w+\s*::\s*)*" + types + r"\b", match.group(1)))
        scopes.append((match.end(), receivers))
    for start, receivers in scopes:
        body = braced_body(code, start - 1)
        for receiver in receivers:
            for match in re.finditer(r"\b" + receiver + r"\s*\.\s*" + methods + r"\s*\(", body):
                yield start + match.start()


def persistence_adapter(parts):
    """Return the backend whose context-owned adapter holds `parts`, else None.

    `parts` is relative to API_SOURCE. Only the owning context's top-level backend module or
    directory qualifies; this is a path rule, not evidence that the adapter is complete.
    """
    if len(parts) < 3 or parts[0] != "contexts" or parts[1] not in CONTEXT_UPSTREAM:
        return None
    name = Path(parts[2]).stem if len(parts) == 3 else parts[2]
    return name if name in PERSISTENCE_BACKENDS else None


def persistence_backend(path):
    """Return the backend a dependency path opens directly (driver or platform plumbing)."""
    if path[0] in POSTGRES_DRIVERS or path[:2] == ("platform", "postgres"):
        return "postgres"
    if path[0] in TURSO_DRIVERS or path[:2] == ("platform", "turso"):
        return "turso"
    return None


def adapter_backend(path):
    """Return the backend of a context adapter module a dependency path names, else None."""
    if path[0] == "contexts" and len(path) > 2 and path[2] in PERSISTENCE_BACKENDS:
        return path[2]
    return None


def pure_dependency_error(path):
    """Return why a pure domain must not name `path`; shared by the Server and Identity checks."""
    if path[0] in RPC_RUNTIME or path[0] == PROST or path[:3] == IDENTITY_RPC_CLIENT:
        return "pure domain must not depend on generated RPC DTOs or their runtime; map them in the RPC adapter"
    if path[0] in DOMAIN_EXTERNAL or path[:2] in DOMAIN_STD_EFFECTS or path[-1] in {
        "new_v4", "new_v7", "now_v7", "thread_rng", "random",
    }:
        return "pure domain must not depend on I/O, a runtime, clock or randomness"
    if path[:2] == ("std", "io") and len(path) > 2 and path[2] in {"stdin", "stdout", "stderr"}:
        return "pure domain must not depend on console I/O"
    return None


def dependency_error(path, owner, domain, http, listener_pg_test=False, application_glue=False,
                     adapter=None, rpc_adapter=False, contract=False, studio_protobuf=False):
    """Return the boundary violated by one path, or None for an admitted dependency.

    `adapter` is the backend of the context-owned persistence adapter holding the file, if any;
    `rpc_adapter` marks the platform Identity adapter that owns the generated RPC client;
    `contract` marks the Server's own contract representation and `studio_protobuf` its
    Studio Protobuf edge codec.
    """
    if not path:
        return None
    if path[0] == "cantos_identity":
        return ("Server must reach Identity through its own generated client of Identity's "
                "contract (platform::identity_auth::rpc), never the cantos-identity service crate")
    if contract and path[0] in SERVER_CONTRACT_FORBIDDEN:
        return ("the Server's contract representation is a wire leaf: it must not depend on "
                "domain, application, adapters or platform code")
    if (owner or domain or http or application_glue) and path[0] == "<legacy>":
        return "context uses a legacy crate alias; name the owning context explicitly"
    if (owner or application_glue) and path[0] in {"interfaces", "adapters"} and not (listener_pg_test and path in {
        ("adapters", "inbound", "http", "router_with_listener_identity"),
        ("adapters", "inbound", "http", "AppState"),
    }):
        return "context must not depend on an interface"
    if domain:
        if path[0] == "application":
            return "pure domain must not depend on application glue"
        if error := pure_dependency_error(path):
            return error
        if path[0] == "platform" and path[:2] != ("platform", "ids") and path[:3] != ("platform", "identity", "domain"):
            return "pure domain may use only pure platform::ids or identity::domain values"
        if path[0] in {"interfaces", "adapters"}:
            return "pure domain must not depend on an interface"
        # Persisted canonical documents may live in contracts; listener read projections may not.
        if path[:3] == (SERVER_CONTRACT, "theatre", "discovery"):
            return "pure domain must not build Theatre wire DTOs; project them in the application layer"
        if owner in {"editorial", "adaptation"} and path[:3] in {
            (SERVER_CONTRACT, "studio", "FieldIssue"), (SERVER_CONTRACT, "studio", "AdaptationProblem"),
        }:
            return "pure diagnostics must use safe domain facts; project wire errors at the boundary"
    if (path[0] in RPC_RUNTIME or path[:3] == IDENTITY_RPC_CLIENT) and not rpc_adapter:
        return "generated Identity RPC DTOs and their runtime stay in the platform Identity adapter"
    if path[0] == PROST and not (rpc_adapter or studio_protobuf):
        return ("prost stays in the generated Identity RPC client and the Studio Protobuf edge "
                "codec (contract::studio::protobuf)")
    backend = persistence_backend(path)
    # Legacy PostgreSQL transactions still cross context application code through
    # platform::postgres; the Turso port must not repeat that by naming the engine directly.
    if owner and not domain and adapter is None and backend == "turso":
        return "context code must reach the shared Turso owner only through its persistence adapter"
    # One adapter module per backend: a Turso adapter is not a second database beside PostgreSQL,
    # nor a caller of the other backend's adapter.
    opened = backend or adapter_backend(path)
    if adapter and opened and opened != adapter:
        return f"the {adapter} persistence adapter must not open the {opened} backend"
    if owner and path[:-1] in STORE_PATHS and path[-1] in HTTP_STORE_METHODS:
        return "context must not call the HTTP-owned Store facade; use internal Editorial operations"
    if path[0] == "contexts" and len(path) >= 2:
        target = path[1]
        credential_inquiry = owner == "production" and not domain and path == ("contexts", "media", "upload_token_registered")
        if owner and target != owner and target not in CONTEXT_UPSTREAM[owner] and not credential_inquiry:
            return f"{owner} must not depend on {target}; upstream contexts are {sorted(CONTEXT_UPSTREAM[owner])}"
        if domain and (len(path) < 3 or path[2] != "domain"):
            return "pure domain must name a context's domain directly, without an adapter or facade"
        if owner and target != owner and len(path) >= 3 and path[2] in {
            *PERSISTENCE_BACKENDS, "application", "providers", "media", "worker",
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
        media_read = (path[:3] == ("contexts", "media", "read") and len(path) >= 4
                      and path[3] in MEDIA_READ_HTTP_TYPES)
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
            # Trusted origin-delivery composition value (proposed ADR 0032); no decision API.
            ("contexts", "media", "OriginPlayback"),
        }
        listener_composition = path in {
            ("contexts", "listening", "LibraryService"),
            ("contexts", "listening", "LibraryService", "new"),
            ("contexts", "listening", "auth", "AuthenticatedListener"),
            ("contexts", "listening", "auth", "ListenerAuthenticator"),
            ("contexts", "listening", "auth", "DenyListenerAuthentication"),
            ("contexts", "listening", "auth", "TheatreListenerAuthenticator"),
            ("contexts", "listening", "auth", "TheatreListenerAuthenticator", "new"),
            ("contexts", "listening", "auth", "BearerListenerAuthenticator"),
            ("contexts", "listening", "auth", "BearerListenerAuthenticator", "new"),
            ("contexts", "listening", "auth", "NativeListenerAuthenticator"),
            ("contexts", "listening", "auth", "NativeListenerAuthenticator", "new"),
            ("contexts", "listening", "auth", "PasswordListenerAuthenticator"),
            ("contexts", "listening", "auth", "PasswordListenerAuthenticator", "new"),
            ("contexts", "listening", "auth", "ListenerCredentials"),
            ("contexts", "listening", "auth", "ListenerCredentials", "bearer"),
        } or path[:3] == ("contexts", "listening", "LibraryError") or path[:4] == (
            "contexts", "listening", "auth", "AuthenticationError")
        artwork_composition = path in {
            ("contexts", "media", "ArtworkService"),
            ("contexts", "media", "ArtworkService", "new"),
            ("contexts", "media", "ArtworkReceipt"),
            # ADR 0033 owner cover disclosure: one application use case, mounted with artwork.
            ("contexts", "media", "CoverDisclosureService"),
            ("contexts", "media", "CoverDisclosureService", "new"),
        }
        artwork_error = path[:3] in {
            ("contexts", "media", "ArtworkApplicationError"),
            ("contexts", "media", "ImageObjectError"),
            ("contexts", "media", "CoverDisclosureApplicationError"),
        } or path[:5] in {
            ("contexts", "media", "domain", "artwork", "ArtworkError"),
            ("contexts", "media", "domain", "image", "ImageError"),
            ("contexts", "media", "domain", "cover_disclosure", "CoverDisclosureError"),
        } or path[:4] == ("contexts", "media", "image_intake", "ImageIntakeError")
        if http and path not in HTTP_PURE_BOUNDARIES and not (
            catalog_wire or catalog_error or catalog_input_type or media_read
            or publication_error or publication_composition or listener_composition
            or artwork_composition or artwork_error
        ):
            return "HTTP must delegate business decisions through application use cases"
    if http and path[0] in DOMAIN_EXTERNAL - {"axum", "tokio", "tower", "tower_http"}:
        return "HTTP must not call a database or external provider directly"
    if http and path[:2] in PERSISTENCE_PLUMBING and path != ("platform", "postgres") and (
        path[:3] not in HTTP_PLUMBING_FACADE
    ):
        return "HTTP may import only the reviewed Store and StoreError facade from persistence plumbing"
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
    if (root / LEGACY_API_CRATE / "Cargo.toml").exists():
        errors.add(f"{LEGACY_API_CRATE}: second server implementation; "
                   f"{API_SOURCE.parent} is the only canonical crate")
    files = sorted((root / API_SOURCE).rglob("*.rs"))
    if not files:
        # An empty walk would report no violations and pass: fail when the guarded tree moves.
        errors.add(f"{API_SOURCE}: no Rust sources to check, so the architecture guard "
                   "would pass vacuously; update API_SOURCE when the crate moves")
        return sorted(errors)
    identity_owned = identity_tables(root)
    private_modules = "|".join((*PERSISTENCE_BACKENDS, "application"))
    for file in sources(root / API_SOURCE):
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
        domain = domain or relative in {Path("platform/identity/domain.rs"), Path("platform/oidc/core.rs"), Path("platform/admin/domain.rs")}
        http = parts[:3] == ("adapters", "inbound", "http")
        platform = parts[0] == "platform" and not domain
        adapter = persistence_adapter(parts)
        module = rust_module(relative)
        content = file.read_text(encoding="utf-8")
        code, strings = source_texts(file, content)
        for offset, path in source_paths(code, module):
            error = dependency_error(path, owner, domain, http,
                listener_pg_test=relative in {
                    Path("contexts/listening/postgres/tests.rs"),
                    Path("contexts/listening/turso/tests.rs"),
                },
                application_glue=relative == Path("application.rs"),
                adapter=adapter,
                rpc_adapter=len(parts) > 1 and (parts[0], Path(parts[1]).stem) == IDENTITY_RPC_ADAPTER,
                contract=module[:1] == (SERVER_CONTRACT,),
                studio_protobuf=module[:3] == STUDIO_PROTOBUF_CODEC)
            if parts[0] == "platform" and path[0] == "contexts":
                error = "platform must not depend on a business context"
            if error:
                line = code.count("\n", 0, offset) + 1
                errors.add(f"{API_SOURCE / relative}:{line}: {'::'.join(path)}: {error}")
        for offset, crate in renamed_crates(code):
            line = code.count("\n", 0, offset) + 1
            errors.add(f"{API_SOURCE / relative}:{line}: extern crate {crate}: renaming it hides it "
                       "from the source guards; use its crate name")
        if domain and re.search(r"\b(?:print|println|eprint|eprintln|dbg)\s*!", code):
            errors.add(f"{API_SOURCE / relative}: pure domain must not write to the console")
        if owner and relative == Path(f"contexts/{owner}/mod.rs"):
            for match in re.finditer(rf"\bpub(?:\s*\([^)]*\))?\s+mod\s+({private_modules})\b", code):
                errors.add(f"{API_SOURCE / relative}: {match.group(1)} must remain private")
        if owner:
            for offset in http_store_method_calls(code):
                line = code.count("\n", 0, offset) + 1
                errors.add(f"{API_SOURCE / relative}:{line}: context must not call the HTTP-owned Store facade; use internal Editorial operations")
        if http:
            for match in re.finditer(
                r"\.\s*(connection|transaction|query|query_one|query_opt|execute|batch_execute)\s*\(", code,
            ):
                errors.add(f"{API_SOURCE / relative}: HTTP must not execute SQL or open transactions")
        for offset, text, inline in strings:
            if not inline and relative == TURSO_SCHEMA_DDL / "mod.rs":
                continue
            if attach := SQL_ATTACH.search(text):
                errors.add(f"{API_SOURCE / relative}:{sql_line(content, offset, attach.start(), inline)}: "
                           "SQL must not ATTACH another database; each service owns one database file")
            if (relative.suffix == ".sql" and relative.parent == TURSO_SCHEMA_DDL) or (
                relative == TEST_FIXTURES.with_suffix(".rs") or parts[0] == TEST_FIXTURES.name
            ):
                continue
            for start, verb, table, _ in sql_tables(text):
                target = table_owner(table)
                line = sql_line(content, offset, start, inline)
                if not target:
                    if table in identity_owned:
                        errors.add(f"{API_SOURCE / relative}:{line}: SQL table {table} belongs to "
                                   "the Identity database; reach Identity only through its RPC contract")
                    continue
                # This legacy read proves the source link and bytes together under editorial's
                # already-held script lock. No other cross-context table access is admitted; a
                # Turso adapter needs its own reviewed exception.
                legacy_link = (relative.as_posix(), verb, table) in {
                    ("contexts/ingestion/postgres.rs", "FROM", "revision_evidence"),
                    ("contexts/editorial/postgres.rs", "JOIN", "actors"),
                }
                if domain or not (adapter or platform):
                    errors.add(f"{API_SOURCE / relative}:{line}: SQL table access belongs in "
                               "its owner's persistence adapter")
                elif platform and target != "platform" and (
                    relative.as_posix(), verb, table,
                ) not in PLATFORM_CONTEXT_SQL:
                    errors.add(f"{API_SOURCE / relative}:{line}: SQL table {table} belongs to {target}")
                elif adapter and target != owner and not legacy_link:
                    errors.add(f"{API_SOURCE / relative}:{line}: SQL table {table} belongs to {target}")
    return sorted(errors)


def identity_tables(root):
    """Return the tables Identity's persistence adapter defines, excluding its test fixtures."""
    tables = set()
    for file in sources(root / IDENTITY_SOURCE / IDENTITY_PERSISTENCE):
        if file.stem == "tests" or file.stem.endswith("_tests"):
            continue
        for _, text, _ in source_texts(file, file.read_text(encoding="utf-8"))[1]:
            tables.update(table for _, _, table, defines in sql_tables(text) if defines)
    return tables


def check_identity_architecture(root=ROOT):
    """Check Identity's pure domain, RPC and persistence placement and database ownership.

    Like the Server check, this reads explicit Rust paths and named SQL tables. It does not prove
    process or file isolation, one-owner locking or that no runtime path opens another service's
    database; those obligations belong to the native isolation and file-backed tests.
    """
    source = root / IDENTITY_SOURCE
    files = sorted(source.rglob("*.rs"))
    if not files:
        return [f"{IDENTITY_SOURCE}: no Rust sources to check, so the Identity guard would pass "
                "vacuously; update IDENTITY_SOURCE when the crate moves"]
    errors = set()
    modules = {file.relative_to(source).parts[0].removesuffix(".rs") for file in files}
    if "domain" not in modules:
        errors.add(f"{IDENTITY_SOURCE}: no domain module, so the purity guard would pass vacuously")
    tables = identity_tables(root)
    if not tables:
        errors.add(f"{IDENTITY_SOURCE / IDENTITY_PERSISTENCE}: defines no tables, so the database "
                   "ownership guard would pass vacuously")
    outer = modules - {"domain", "lib", "main", "bin"}
    for file in sources(source):
        relative = file.relative_to(source)
        module = Path(relative.parts[0]).stem
        domain = module == "domain"
        content = file.read_text(encoding="utf-8")
        code, strings = source_texts(file, content)
        for offset, path in source_paths(code, rust_module(relative), legacy=()):
            if path[0] == "cantos_server":
                error = "Identity must not depend on the Server"
            elif domain:
                error = pure_dependency_error(path) or (
                    "Identity domain must not depend on its adapters or application"
                    if path[0] in outer else None)
            elif (path[0] in RPC_RUNTIME or path[0] == PROST or path[:2] == ("rpc", "generated")) \
                    and module not in IDENTITY_RPC_MODULES:
                error = ("generated RPC DTOs and their runtime stay in Identity's RPC adapter "
                         "and process composition")
            else:
                error = None
            if error:
                line = code.count("\n", 0, offset) + 1
                errors.add(f"{IDENTITY_SOURCE / relative}:{line}: {'::'.join(path)}: {error}")
        for offset, crate in renamed_crates(code):
            line = code.count("\n", 0, offset) + 1
            errors.add(f"{IDENTITY_SOURCE / relative}:{line}: extern crate {crate}: renaming it hides it "
                       "from the source guards; use its crate name")
        if domain and re.search(r"\b(?:print|println|eprint|eprintln|dbg)\s*!", code):
            errors.add(f"{IDENTITY_SOURCE / relative}: pure domain must not write to the console")
        for offset, text, inline in strings:
            if attach := SQL_ATTACH.search(text):
                errors.add(f"{IDENTITY_SOURCE / relative}:{sql_line(content, offset, attach.start(), inline)}: "
                           "SQL must not ATTACH another database; each service owns one database file")
            for start, verb, table, defines in sql_tables(text):
                line = sql_line(content, offset, start, inline)
                if table_owner(table):
                    errors.add(f"{IDENTITY_SOURCE / relative}:{line}: SQL table {table} belongs "
                               "to the Server database")
                elif module != IDENTITY_PERSISTENCE and (table in tables or defines):
                    errors.add(f"{IDENTITY_SOURCE / relative}:{line}: SQL table access and DDL "
                               "belong in Identity's persistence adapter")
    return sorted(errors)


def manifest_dependencies(manifest, member, inherited):
    """Yield (section, key, crate, root-relative path) per dependency, target tables included.

    A `workspace = true` entry takes its crate and path from `[workspace.dependencies]`
    (`inherited`), whose paths are relative to the workspace root.
    """
    kinds = ("dependencies", "dev-dependencies", "build-dependencies")
    tables = [(kind, manifest.get(kind, {})) for kind in kinds]
    for cfg, target in manifest.get("target", {}).items():
        tables += [(f"target.{cfg}.{kind}", target.get(kind, {})) for kind in kinds]
    for section, table in tables:
        for key, spec in table.items():
            detail = spec if isinstance(spec, dict) else {}
            origin, base = detail, member
            if detail.get("workspace") is True:
                shared = inherited.get(key, {})
                origin, base = (shared if isinstance(shared, dict) else {}), Path(".")
            crate = detail.get("package") or origin.get("package") or key
            path = origin.get("path")
            yield section, key, crate, Path(os.path.normpath(base / path)) if path else None


def crate_name(name):
    return name.replace("-", "_")


def check_manifest_boundaries(root=ROOT):
    """Check workspace dependency direction from the members' Cargo manifests.

    Shared packages depend on no service or app, apps reach services over HTTP, a service uses
    another service only from tests, only the shared store owner depends on the Turso engine,
    no crate the source checks look for is renamed out of their sight, and the retired central
    contract package does not return (each service owns its canonical contracts). Path dependencies
    (including inherited workspace ones) are resolved; the compiler enforces that undeclared
    crates are unreachable. This is static dependency evidence, not process/database isolation.
    """
    try:
        import tomllib  # Python 3.11+; on an older interpreter this check fails closed.
        workspace = tomllib.loads((root / WORKSPACE_MANIFEST).read_text(encoding="utf-8"))
    except (ImportError, OSError, ValueError) as error:
        return [f"{WORKSPACE_MANIFEST}: cannot read the workspace manifest: {error}"]
    members = [Path(member) for member in workspace.get("workspace", {}).get("members", [])]
    inherited = workspace.get("workspace", {}).get("dependencies", {})
    errors = []
    if (root / RETIRED_CONTRACT_PACKAGE).exists():
        errors.append(f"{RETIRED_CONTRACT_PACKAGE}: retired central contract package; canonical "
                      "contracts belong to their defining service (ADR 0046)")
    if not any(member.parts[:1] == ("packages",) for member in members):
        errors.append(f"{WORKSPACE_MANIFEST}: no packages/ members, so the dependency-direction "
                      "guard would pass vacuously")
    for member in members:
        manifest_path = member / "Cargo.toml"
        try:
            manifest = tomllib.loads((root / manifest_path).read_text(encoding="utf-8"))
        except (OSError, ValueError) as error:
            errors.append(f"{manifest_path}: cannot read the workspace member manifest: {error}")
            continue
        layer = member.parts[0]
        for section, key, crate, target in manifest_dependencies(manifest, member, inherited):
            where = f"{manifest_path}: [{section}] {key}"
            if crate_name(crate) in TURSO_ENGINE_CRATES and member != TURSO_OWNER:
                errors.append(f"{where}: depend on the shared cantos-store-turso owner, not the engine")
            if crate_name(crate) in GUARDED_CRATES and crate_name(key) != crate_name(crate):
                errors.append(f"{where}: renaming {crate} hides it from the source guards; "
                              "depend on it by its crate name")
            if target is None or target == member:
                continue
            reached = target.parts[0]
            if layer == "packages" and reached in {"services", "apps"}:
                errors.append(f"{where}: shared packages must not depend on a service or app")
            elif layer == "apps" and reached in {"services", "apps"}:
                errors.append(f"{where}: apps reach services over HTTP and never import service "
                              "or another app's code")
            elif layer == "services" and (reached == "apps" or (
                reached == "services" and not section.endswith("dev-dependencies"))):
                errors.append(f"{where}: a service reaches another service through its own "
                              "generated client of the owner's contract; implementation crates "
                              "are test-only dev-dependencies")
    return errors


THEATRE_MOBILE = Path("apps/cantos-theatre/mobile")
THEATRE_SOURCE = THEATRE_MOBILE / "shared/src"
THEATRE_SANDBOX_SOURCE = THEATRE_MOBILE / "preview-sandbox/src"
# Production application hosts compose :shared only; their main sources obey the shared boundary.
THEATRE_PRODUCTION_HOSTS = (":androidApp",)
THEATRE_PACKAGE = ("com", "cantos", "theatre")
THEATRE_LAYERS = {"domain", "ports", "application", "features", "adapters", "app", "demo", "presentation"}
# Allowlisted layers may import only these namespaces. Features use the denylist in
# theatre_dependency_error; adapters and app have only the production/sandbox boundary.
THEATRE_INTERNAL_ALLOWED = {
    "domain": ("domain",),
    "ports": ("domain", "ports"),
    "application": ("domain", "ports", "application"),
    "demo": ("demo", "presentation.model"),
    "presentation": ("presentation",),
}
THEATRE_EXTERNAL_ALLOWED = {
    "domain": ("kotlin",),
    "ports": ("kotlin", "kotlinx.coroutines"),
    "application": ("kotlin", "kotlinx.coroutines"),
    "demo": ("kotlin",),
    "presentation": ("kotlin",),
}
THEATRE_FEATURE_FORBIDDEN = ("app", "adapters", "ports")
THEATRE_COMMON_FORBIDDEN_ROOTS = {"android", "platform", "java", "javax"}
THEATRE_QUALIFIED_ROOTS = {"androidx", "android", "java", "javax", "platform", "kotlinx"}
THEATRE_IDENT = r"(?:[A-Za-z_]\w*|`[^`\n]+`)"
THEATRE_QUALIFIED = rf"{THEATRE_IDENT}(?:\s*\.\s*{THEATRE_IDENT})*"
THEATRE_PACKAGE_STATEMENT = re.compile(rf"(?m)^[ \t]*package\s+({THEATRE_QUALIFIED})")
THEATRE_IMPORT_STATEMENT = re.compile(
    rf"(?m)(?:^|;)[ \t]*import\s+({THEATRE_QUALIFIED}(?:\s*\.\s*\*)?)(?:\s+as\s+{THEATRE_IDENT})?"
)
THEATRE_QUALIFIED_USE = re.compile(rf"(?<![\w.`])({THEATRE_QUALIFIED})")
# Types and top-level factories alike, such as MockStorageDatabase and mockAppContainer.
THEATRE_SANDBOX_SYMBOL = re.compile(
    r"(?:[Mm]ock[A-Z]\w*|[Ff]ake[A-Z]\w*|[Dd]emo[A-Z]\w*|CantosDemo\w*)\Z"
)
THEATRE_SANDBOX_PART = re.compile(r"(?:mock|demo|sandbox|preview)(?:[-_]|$)", re.I)
THEATRE_TEST_NAMESPACES = (
    ("kotlin", "test"), ("kotlinx", "coroutines", "test"), ("org", "junit"),
    ("junit",), ("io", "mockk"), ("io", "kotest"),
    ("io", "ktor", "client", "engine", "mock"), ("okio", "fakefilesystem"),
    ("androidx", "compose", "ui", "test"),
)


def theatre_sandbox_dependency_error(name):
    """The shared module's main sources have no preview/test escape through app/adapters."""
    if (any(THEATRE_SANDBOX_SYMBOL.fullmatch(part) for part in name)
            or under(name, THEATRE_PACKAGE) and any(
                part.lower() in {"mock", "demo", "sandbox", "preview", "workbench"}
                for part in name[len(THEATRE_PACKAGE):]
            ) or under(name, ("com", "cantos", "ui", "preview"))
            or name[:2] == ("AppContainer", "mock")
            or any(under(name, prefix) for prefix in THEATRE_TEST_NAMESPACES)):
        return "shared main must not depend on preview, mock, demo or test code"
    return None


def theatre_names(text):
    return tuple(part.strip("`") for part in re.sub(r"\s+", "", text).split("."))


def under(name, prefix):
    return name[:len(prefix)] == prefix


def theatre_dependency_error(layer, feature, source_set, name):
    """Return why this layer may not use `name` (a dotted tuple, wildcard stripped)."""
    if layer in THEATRE_INTERNAL_ALLOWED:
        allowed = [THEATRE_PACKAGE + tuple(part.split(".")) for part in THEATRE_INTERNAL_ALLOWED[layer]]
        allowed += [tuple(part.split(".")) for part in THEATRE_EXTERNAL_ALLOWED[layer]]
        if any(under(name, prefix) for prefix in allowed):
            return None
        return f"{layer} may import only {', '.join('.'.join(prefix) for prefix in allowed)}"
    if layer == "features":
        for target in THEATRE_FEATURE_FORBIDDEN:
            if under(name, THEATRE_PACKAGE + (target,)):
                return f"features must not depend on {target}"
        own = THEATRE_PACKAGE + ("features", feature)
        if under(name, THEATRE_PACKAGE + ("features",)) and not under(name, own):
            return f"features must not import another feature (own: {feature})"
        if source_set == "commonMain" and name[0] in THEATRE_COMMON_FORBIDDEN_ROOTS:
            return f"commonMain features must not use platform namespace {name[0]}"
    return None


def blank(text):
    return re.sub(r"[^\n]", " ", text)


def theatre_gradle_main_text(text):
    """Blank recognized Test source-set blocks; all unknown scopes stay production-visible."""
    test_scope = re.compile(
        r'(?:\b\w+Test(?:\s+by\s+(?:getting|creating))?'
        r'|\b(?:getByName|named|maybeCreate)\s*\(\s*"\w+Test"\s*\))'
        r'(?:\s*\.\s*\w+(?:\s*\([^{}]*\))?)*\s*$'
    )
    tokens = re.finditer(r'"""[\s\S]*?"""|"(?:\\.|[^"\\])*"|[{}]', text)
    stack, ranges, boundary = [], [], 0
    for token in tokens:
        if token.group() == "{":
            test = bool(stack and stack[-1][1]) or bool(test_scope.search(text[boundary:token.start()]))
            stack.append((token.start(), test))
            boundary = token.end()
        elif token.group() == "}" and stack:
            start, test = stack.pop()
            if test and not (stack and stack[-1][1]):
                ranges.append((start, token.end()))
            boundary = token.end()
    for start, end in reversed(ranges):
        text = text[:start] + blank(text[start:end]) + text[end:]
    return text


def theatre_gradle_projects(text, label):
    """Conservatively count every project reference, including aliases and Test edges."""
    edges, errors = set(), []
    for _, arguments in theatre_gradle_calls(text, lambda name: name == "project"):
        target = re.fullmatch(r'\s*(?:path\s*=\s*)?"(:[\w:-]+)"\s*,?\s*', arguments)
        if target:
            edges.add(target.group(1))
        else:
            errors.append(f"{label}: project dependency requires a literal path for architecture inspection")
    return edges, errors


def theatre_gradle_calls(text, select):
    for name, arguments, _ in theatre_gradle_call_regions(text, select):
        yield name, arguments


def theatre_gradle_call_regions(text, select):
    """Read balanced calls without interpreting call-shaped text inside policy strings."""
    tokens = list(re.finditer(r'"""[\s\S]*?"""|"(?:\\.|[^"\\])*"|\w+|[^\s]', text))
    for index, token in enumerate(tokens[:-1]):
        if not select(token.group()) or tokens[index + 1].group() != "(":
            continue
        depth = 1
        for end in tokens[index + 2:]:
            depth += (end.group() == "(") - (end.group() == ")")
            if not depth:
                yield token.group(), text[tokens[index + 1].end():end.start()], token.start()
                break


def theatre_gradle_dependencies(text):
    """Inspect dependency declarations and their simple val aliases, not assertion messages."""
    declaration = re.compile(r"(?:implementation|api|compileOnly|runtimeOnly|add|"
                             r"\w+Main(?:Implementation|Api|CompileOnly|RuntimeOnly))\Z")
    expressions = [arguments for _, arguments in theatre_gradle_calls(text, declaration.fullmatch)]
    aliases = dict(re.findall(r"\bval\s+(\w+)(?:\s*:[^=\n]+)?\s*=\s*([^\n;]+)", text))
    included = set()
    for _ in range(len(aliases)):
        used = set(re.findall(r"\b\w+\b", " ".join(expressions)))
        selected = used.intersection(aliases).difference(included)
        if not selected:
            break
        expressions.extend(aliases[name] for name in sorted(selected))
        included.update(selected)
    return "\n".join(expressions)


def theatre_gradle_main_blocks(text):
    """Locate literal Main source-set blocks so exclusions cannot leak across siblings."""
    source_scope = re.compile(
        r'(?:\b(?P<direct>\w+Main)(?:\s+by\s+(?:getting|creating))?'
        r'|\b(?:getByName|named|maybeCreate)\s*\(\s*"(?P<named>\w+Main)"\s*\))'
        r'(?:\s*\.\s*\w+(?:\s*\([^{}]*\))?)*\s*$'
    )
    stack, blocks, boundary = [], [], 0
    for token in re.finditer(r'"""[\s\S]*?"""|"(?:\\.|[^"\\])*"|[{}]', text):
        if token.group() == "{":
            heading = source_scope.search(text[boundary:token.start()])
            name = (heading.group("direct") or heading.group("named")) if heading else None
            stack.append((token.start(), name))
            boundary = token.end()
        elif token.group() == "}" and stack:
            start, name = stack.pop()
            if name:
                blocks.append((start, token.end(), name))
            boundary = token.end()
    return blocks


def theatre_production_modules(root):
    """Inspect the local, literal Gradle graph, independently of Android variant resolution.

    No Gradle is evaluated. Conditional edges count as present; project aliases are caught at
    their project() declaration. Test project edges also count, preventing a shared Test ->
    sandbox -> shared cycle. Version catalogs, convention plugins and dynamic source routing
    require a new explicit policy rather than claiming resolved dependency/artifact evidence.
    """
    from ui_preview_catalog import source_text

    mobile = root / THEATRE_MOBILE
    settings = mobile / "settings.gradle.kts"
    if not settings.is_file():
        return {THEATRE_SOURCE}, []
    settings_text = source_text(settings.read_text(encoding="utf-8"))
    included = {name for _, call in theatre_gradle_calls(settings_text, lambda name: name == "include")
                for name in re.findall(r'"(:[\w:-]+)"', call)}
    build_files = {mobile / "build.gradle.kts"} | set(mobile.glob("*/build.gradle.kts"))
    build_files.update(mobile / Path(*name.strip(":").split(":")) / "build.gradle.kts" for name in included)
    errors, builds, graph = [], {}, {}
    for file in sorted(file for file in build_files if file.is_file()):
        relative = file.relative_to(mobile)
        if any(part in {"build", ".gradle", ".kotlin"} for part in relative.parts):
            continue
        module = ":" + ":".join(relative.parts[:-1])
        text = source_text(file.read_text(encoding="utf-8"))
        builds[module] = (file, text)
        graph[module], project_errors = theatre_gradle_projects(text, file.relative_to(root))
        errors.extend(project_errors)
    accessors = {".".join(re.sub(r"-([a-z])", lambda m: m.group(1).upper(), part)
                           for part in name.strip(":").split(":")): name for name in builds if name != ":"}
    for module, (file, text) in builds.items():
        for accessor in re.findall(r"\bprojects\.([\w.]+)", source_text(text, keep_strings=False)):
            target = next((name for key, name in accessors.items()
                           if accessor == key or accessor.startswith(key + ".")), None)
            if target:
                graph[module].add(target)
            else:
                errors.append(f"{file.relative_to(root)}: unknown project accessor {accessor}")

    for module in (":shared", ":preview-sandbox"):
        if module not in included or module not in builds:
            errors.append(f"{THEATRE_MOBILE}/settings.gradle.kts: {module} must be a separate included Gradle module")
    if ":preview-sandbox" in builds:
        file, text = builds[":preview-sandbox"]
        if not re.search(r'\bkotlin\s*\(\s*"multiplatform"\s*\)', text):
            errors.append(f"{file.relative_to(root)}: preview-sandbox must be a Kotlin Multiplatform module")
        if ":shared" not in graph[":preview-sandbox"]:
            errors.append(f"{file.relative_to(root)}: preview-sandbox must depend explicitly on :shared")
    for host in THEATRE_PRODUCTION_HOSTS:
        if host not in included or host not in builds:
            errors.append(f"{THEATRE_MOBILE}/settings.gradle.kts: production host {host} must be a separate included Gradle module")
        elif graph[host] != {":shared"}:
            errors.append(f"{builds[host][0].relative_to(root)}: production host must depend on :shared and no other local project")
    if re.search(r"\bproject\s*\([^)]*\)\s*\.\s*projectDir\b", settings_text):
        errors.append(f"{THEATRE_MOBILE}/settings.gradle.kts: projectDir remapping needs an explicit architecture policy")

    # A stack: :shared is visited first, so its own edges are reported from :shared.
    production = set()
    pending = [(host,) for host in THEATRE_PRODUCTION_HOSTS if host in builds] + [(":shared",)]
    while pending:
        chain = pending.pop()
        module = chain[-1]
        if module in production:
            continue
        production.add(module)
        if module != ":shared" and any(
                THEATRE_SANDBOX_PART.match(part) for part in re.split(r"[:/]+", module) if part):
            owner = "shared" if chain[0] == ":shared" else "production host"
            errors.append(f"{THEATRE_MOBILE}: {owner} main dependency reaches sandbox/demo module: {' -> '.join(chain)}")
            continue
        if module not in builds:
            errors.append(f"{THEATRE_MOBILE}: unresolved local project dependency: {' -> '.join(chain)}")
            continue
        file, text = builds[module]
        main = theatre_gradle_main_text(text)
        dependencies = theatre_gradle_dependencies(main)
        if any(True for _ in theatre_gradle_calls(dependencies, lambda name: name in {"files", "fileTree"})):
            errors.append(f"{file.relative_to(root)}: local binary dependencies need an explicit production policy")
        for coordinate in re.findall(r'"([^"\n]+:[^"\n]+)"', dependencies):
            artifact = coordinate.split(":")[1]
            if re.search(r"(?:^|[-_.])(?:test\w*|mock\w*|fakefilesystem|junit\w*|kotest)(?:[-_.]|$)", artifact, re.I):
                errors.append(f"{file.relative_to(root)}: shared main graph must not include test dependency {coordinate}")
        if re.search(r'\bkotlin\s*\(\s*"test(?:-[^"]*)?"|\bcompose\.\w+(?:\.\w+)*[Tt]est\w*'
                     r'|\blibs\.(?:\w+\.)*(?:test\w*|mock\w*|junit\w*|kotest)\b', dependencies):
            errors.append(f"{file.relative_to(root)}: shared main graph must not include test dependency aliases")
        main_blocks = theatre_gradle_main_blocks(main)
        for _, arguments, offset in theatre_gradle_call_regions(main, lambda name: name in {"srcDir", "srcDirs", "setSrcDirs"}):
            paths = re.findall(r'"([^"\n]+)"', arguments)
            for path in paths:
                own_main = re.fullmatch(r"src/\w+Main/(?:kotlin|resources|composeResources)", path)
                canonical_ui = module == ":shared" and path == "../../../packages/ui-system/cmp/src/commonMain/kotlin"
                containing = [(start, end, name) for start, end, name in main_blocks if start < offset < end]
                source_set = min(containing, key=lambda span: span[1] - span[0])[2] if containing else None
                # Reviewed production codegen routes: Library and Identity v1 schema types. Fixtures are
                # generated into a separate commonTest directory, never admitted by this exception.
                # The literal output and compile dependency remain inspectable without Gradle.
                generated_routes = {
                    "generated/libraryWire/commonMain/kotlin": "generateLibraryWire",
                    "generated/identityV1Wire/commonMain/kotlin": "generateIdentityV1Wire",
                }
                generator = generated_routes.get(path)
                schema_wire = (
                    module == ":shared" and source_set == "commonMain"
                    and generator is not None
                    and re.fullmatch(r'layout\s*\.\s*buildDirectory\s*\.\s*dir\s*\(\s*"' + re.escape(path) + r'"\s*\)', arguments.strip())
                    and re.search(r'\bkotlin\s*\.\s*$', main[:offset])
                    and re.search(r'\bdependsOn\s*\(\s*' + re.escape(generator) + r'\s*\)', source_text(main, keep_strings=False))
                )
                if not own_main and not canonical_ui and not schema_wire:
                    errors.append(f"{file.relative_to(root)}: main source routing needs an explicit production policy: {path}")
                if canonical_ui:
                    containing = [(start, end) for start, end, _ in main_blocks if start < offset < end]
                    start, end = min(containing, key=lambda span: span[1] - span[0]) if containing else (0, 0)
                    scope = main[start:end]
                    exclusions = {arguments.strip() for _, arguments in theatre_gradle_calls(scope, lambda name: name == "exclude")}
                    if '"com/cantos/ui/preview/**"' not in exclusions:
                        errors.append(f"{file.relative_to(root)}: shared UI source inclusion must exclude its preview package in the same Main source set")
            if not paths:
                errors.append(f"{file.relative_to(root)}: main source routing requires literal inspected paths")
        for target in sorted(graph[module]):
            pending.append(chain + (target,))

    def visit(module, chain):
        if module in chain:
            errors.append(f"{THEATRE_MOBILE}: Gradle project dependency cycle: {' -> '.join(chain + (module,))}")
            return
        if module in visited:
            return
        visited.add(module)
        for target in sorted(graph.get(module, ())):
            visit(target, chain + (module,))

    visited = set()
    for module in sorted(graph):
        visit(module, ())
    sources = {THEATRE_MOBILE / Path(*module.strip(":").split(":")) / "src"
               for module in production if module in builds}
    return sources | {THEATRE_SOURCE}, errors


def check_theatre_architecture(root=ROOT):
    """Check Theatre mobile import layers, package/directory agreement and qualified names.

    This is lexical, like the API check: it reads package and import statements plus dotted
    names in code and executable string templates. It does not resolve overloads or prove
    behavior (a clock or random source in domain, reducer imports from features). The
    production-safe shared graph additionally excludes sandbox symbols, resources and local
    project edges in every main layer, including adapters/app. This does not inspect an APK,
    resolve a Gradle variant, dynamic DSL, reflection or external dependency metadata.
    """
    from ui_preview_catalog import kotlin_source_regions

    production_sources, graph_errors = theatre_production_modules(root)
    errors = set(graph_errors)
    for source_path in sorted(production_sources | {THEATRE_SANDBOX_SOURCE}):
        source = root / source_path
        production = source_path in production_sources
        # Android application modules use the plain `main` source set.
        for file in sorted({*source.glob("*Main/**/*.kt"), *source.glob("main/**/*.kt")}):
            label = file.relative_to(root).as_posix()
            parts = file.relative_to(source).parts
            source_set = parts[0]
            if parts[1] != "kotlin" or parts[2:5] != THEATRE_PACKAGE:
                errors.add(f"{label}: Kotlin source must live under {source_set}/kotlin/com/cantos/theatre")
                continue
            if production and any(THEATRE_SANDBOX_PART.match(part) for part in parts[2:]):
                errors.add(f"{label}: shared main must not contain preview, mock, demo or sandbox sources")
                continue
            layer = parts[5] if len(parts) > 6 else None
            if layer not in THEATRE_LAYERS:
                errors.add(f"{label}: package needs an explicit layer policy; place it directly under "
                           f"com/cantos/theatre/{{{','.join(sorted(THEATRE_LAYERS))}}}")
                continue
            feature = parts[6] if layer == "features" and len(parts) > 7 else None
            if layer == "features" and not feature:
                errors.add(f"{label}: features code belongs in features/<feature>/")
                continue

            content = file.read_text(encoding="utf-8")
            code, templates = kotlin_source_regions(content, keep_lines=True)

            def report(offset, name, reason):
                errors.add(f"{label}:{code.count(chr(10), 0, offset) + 1}: {'.'.join(name)}: {reason}")

            directory = parts[2:-1]
            statement = THEATRE_PACKAGE_STATEMENT.search(code)
            declared = theatre_names(statement.group(1)) if statement else None
            if declared != directory:
                errors.add(f"{label}:{code.count(chr(10), 0, statement.start()) + 1 if statement else 1}: "
                           f"package {'.'.join(declared) if declared else '(none)'} "
                           f"does not match directory {'.'.join(directory)}")

            remainder = code
            for match in THEATRE_IMPORT_STATEMENT.finditer(code):
                name = theatre_names(match.group(1))
                name = name[:-1] if name[-1] == "*" else name
                reason = theatre_dependency_error(layer, feature, source_set, name)
                if production:
                    reason = reason or theatre_sandbox_dependency_error(name)
                if reason:
                    report(match.start(1), name, reason)
                remainder = remainder[:match.start()] + blank(match.group()) + remainder[match.end():]
            if statement:
                remainder = remainder[:statement.start()] + blank(statement.group()) + remainder[statement.end():]

            # Fully qualified use needs no import in Kotlin, so inspect dotted names directly.
            for text, in_template in ((remainder, False), (templates, True)):
                for match in THEATRE_QUALIFIED_USE.finditer(text):
                    name = theatre_names(match.group(1))
                    project = name[:2] == THEATRE_PACKAGE[:2] and len(name) > 2
                    platform = (len(name) > 2 and name[0] in THEATRE_QUALIFIED_ROOTS
                                and any(part[:1].isupper() for part in name))
                    reason = (project or platform) and theatre_dependency_error(layer, feature, source_set, name)
                    if production:
                        reason = reason or theatre_sandbox_dependency_error(name)
                    if not reason:
                        continue
                    if in_template:
                        # Template regions have no line map; find the first literal occurrence.
                        found = re.search(r"\s*\.\s*".join(re.escape(part) for part in name), content)
                        line = content.count("\n", 0, found.start()) + 1 if found else 1
                        errors.add(f"{label}:{line}: {'.'.join(name)}: {reason}")
                    else:
                        report(match.start(1), name, reason)
        if production:
            for file in sorted({*source.glob("*Main/**/*"), *source.glob("main/**/*")}):
                if not file.is_file() or file.suffix == ".kt":
                    continue
                label = file.relative_to(root).as_posix()
                if file.suffix in {".java", ".swift", ".class", ".jar", ".aar"}:
                    errors.add(f"{label}: non-Kotlin main sources or binary payloads need an explicit production policy")
                    continue
                if any(THEATRE_SANDBOX_PART.match(part) for part in file.relative_to(source).parts[1:]):
                    errors.add(f"{label}: shared main must not contain mock/demo/sandbox resources")
                    continue
                if file.suffix in {".xml", ".json", ".properties", ".txt", ".html", ".svg"}:
                    text = file.read_text(encoding="utf-8")
                    if re.search(r"\b(?:mock_|demo_|sandbox_|Mock[A-Z]\w*|FakeStorage\w*|DemoFixtures)"
                                 r"|com\.cantos\.theatre\.(?:demo|sandbox|adapters\.mock)(?:\.|\b)", text):
                        errors.add(f"{label}: shared main resource references mock/demo/sandbox content")
    return sorted(errors)


def check(base=None):
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
    errors += check_identity_architecture(ROOT)
    errors += check_manifest_boundaries(ROOT)
    errors += check_theatre_architecture(ROOT)

    from native_table_ownership import repository_failures as native_ownership_failures
    errors += native_ownership_failures(ROOT)

    # Added UI copy is checked against exact reviewed owners and exceptions.
    # Full catalog parity is independent from the displayed-literal diff.
    from check_localization import repository_failures as localization_failures
    try:
        errors += localization_failures(ROOT, base=base)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        errors.append(f"localization: {error}")

    # The two diagnostics share one declaration source. Registration never means
    # runtime/design approval; new app imports need fresh evidence on both renderers.
    from ui_preview_catalog import CATALOG, load_design_inventory, outputs, repository_failures, validate
    from ui_tokens import SOURCE, check_outputs, load_source, validate as validate_tokens
    try:
        catalog = validate(load_source(CATALOG.read_bytes()))
        tokens = validate_tokens(load_source(SOURCE.read_bytes()))
        inventory = load_design_inventory(catalog, tokens)
        errors += check_outputs(outputs(catalog, tokens, inventory=inventory))
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
        "local links, JSON syntax, skill structure, Server/Identity/Theatre architecture, workspace dependency direction "
        "and changed UI localization valid."
    )
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", metavar="REF", help="localization comparison ref (uses its merge base with HEAD)")
    arguments = parser.parse_args()
    try:
        sys.exit(check(base=arguments.base))
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"Unable to check repository: {exc}", file=sys.stderr)
        sys.exit(1)
