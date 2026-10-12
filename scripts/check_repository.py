#!/usr/bin/env python3
"""Check bootstrap documents, local links, JSON fixtures, text hygiene and agent skills.

This does not validate application behavior, YAML schemas or audio quality.
Skill checks cover structure only: portable frontmatter, Codex metadata, the
README index, reachable references and Claude Code symlinks.
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
    "apps/cantos/web/README.md",
    "apps/cantos/mobile/README.md",
    "apps/cantos-studio/web/README.md",
    "apps/cantos-studio/mobile/README.md",
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
        "local links, JSON syntax and skill structure valid."
    )
    return 0


if __name__ == "__main__":
    try:
        sys.exit(check())
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"Unable to check repository: {exc}", file=sys.stderr)
        sys.exit(1)
