"""Source-only #332 ownership coverage, called by the existing repository gate.

This recognizes the repository's literal CREATE TABLE public.name (...) declarations.
It does not parse general SQL, inspect a database or authorize migration/retirement.
An unrecognized CREATE TABLE form fails closed for explicit review.

The two PostgreSQL migrator metadata tables were declared only by the retired Rust registry
(`platform/postgres/migrations.rs`, last present at b14acf83^). No current source declares them,
so they are named here and must stay classified `replace-registry`; this check never reads that
retired path.
"""

import json
import re
from collections import Counter
from pathlib import Path


MANIFEST = Path("services/cantos-server/schema/native-table-ownership.json")
SCHEMA = Path("services/cantos-server/schema")
RETIRED_REGISTRY_TABLES = frozenset({"cantos_schema", "cantos_migrations"})
DISPOSITIONS = {"server", "replace-provider-auth", "development-only", "replace-registry"}
PROVIDER_PREFIXES = (
    "bearer_", "studio_oidc_", "theatre_identity_", "theatre_native_",
    "theatre_registration", "theatre_firebase_",
)
DECLARATION = re.compile(
    r"\bCREATE\s+TABLE\s+public\.([a-z_][a-z_0-9]*)\s*\(", re.IGNORECASE | re.ASCII,
)
CREATE_TABLE = re.compile(r"\bCREATE\s+(?:[a-z_][a-z_0-9]*\s+)*TABLE\b", re.IGNORECASE | re.ASCII)


def without_sql_comments(text):
    """Preserve quoted text; remove SQL comments without hiding later declarations.

    Kept string contents can conservatively flag DDL examples as unsupported. This is
    source coverage, not a claim to resolve dynamic SQL or execute PostgreSQL grammar.
    """
    result = []
    index = 0
    while index < len(text):
        if text.startswith("--", index):
            end = text.find("\n", index)
            index = len(text) if end < 0 else end
            result.append(" ")
        elif text.startswith("/*", index):
            depth = 1
            index += 2
            while index < len(text) and depth:
                if text.startswith("/*", index):
                    depth += 1
                    index += 2
                elif text.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            if depth:
                raise ValueError("unterminated SQL block comment")
            result.append(" ")
        elif text[index] in "\"'":
            start, quote = index, text[index]
            index += 1
            while index < len(text):
                if text[index] == "\\" and index + 1 < len(text) and text[index + 1] == quote:
                    # E-strings and standard_conforming_strings can disagree here. Refuse
                    # the source shape rather than choosing a mode and masking later DDL.
                    raise ValueError("unsupported backslash-escaped SQL quote")
                if text[index] == quote:
                    index += 1
                    if index < len(text) and text[index] == quote:
                        index += 1
                    else:
                        break
                else:
                    index += 1
            else:
                raise ValueError("unterminated SQL quote")
            result.append(text[start:index])
        elif text[index] == "$" and (tag := re.match(r"\$(?:[a-zA-Z_][a-zA-Z_0-9]*)?\$", text[index:])):
            delimiter = tag.group()
            end = text.find(delimiter, index + len(delimiter))
            if end < 0:
                raise ValueError("unterminated SQL dollar quote")
            end += len(delimiter)
            result.append(text[index:end])
            index = end
        else:
            result.append(text[index])
            index += 1
    return "".join(result)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def source_tables(root):
    """Return exact table -> source path, refusing duplicate/unsupported declarations."""
    files = sorted((root / SCHEMA).glob("*.sql"))
    if not files:
        raise ValueError("no current schema SQL files")
    tables = {}
    for path in files:
        text = without_sql_comments(path.read_text(encoding="utf-8"))
        declarations = list(DECLARATION.finditer(text))
        if len(declarations) != len(CREATE_TABLE.findall(text)):
            raise ValueError(f"{path.relative_to(root)}: unsupported CREATE TABLE declaration")
        for declaration in declarations:
            # Only ASCII unquoted identifiers are supported; PostgreSQL folds them before
            # lookup, ownership classification and duplicate detection.
            table = declaration.group(1).lower()
            if table in tables:
                raise ValueError(f"duplicate source table: {table}")
            tables[table] = path.relative_to(root).as_posix()
    return tables


def validate(manifest, tables):
    if not isinstance(manifest, dict) or set(manifest) != {"version", "dispositions"}:
        raise ValueError("expected version and dispositions")
    if type(manifest["version"]) is not int or manifest["version"] != 1:
        raise ValueError("unsupported ownership manifest version")
    groups = manifest["dispositions"]
    if not isinstance(groups, dict) or set(groups) != DISPOSITIONS:
        raise ValueError("expected the four explicit native dispositions")
    classified = []
    errors = []
    for disposition, names in groups.items():
        if not isinstance(names, list) or any(not isinstance(name, str) for name in names):
            raise ValueError(f"{disposition}: expected a table-name array")
        if names != sorted(names):
            errors.append(f"{disposition}: table names must be sorted")
        for raw_table in names:
            table = raw_table.lower()
            if re.fullmatch(r"[a-z_][a-z_0-9]*", raw_table, flags=re.ASCII) is None:
                errors.append(f"{raw_table}: expected canonical lowercase unquoted table name")
            classified.append(table)
            required = (
                "replace-provider-auth" if table.startswith(PROVIDER_PREFIXES)
                else "development-only" if table == "sessions"
                else "replace-registry" if table in RETIRED_REGISTRY_TABLES
                else "server"
            )
            if disposition != required:
                errors.append(f"{table}: expected {required}, found {disposition}")
    for table, count in sorted(Counter(classified).items()):
        if count != 1:
            errors.append(f"{table}: classified {count} times")
    for table in sorted(set(tables) - set(classified)):
        errors.append(f"{table}: missing native disposition ({tables[table]})")
    for table in sorted(set(classified) - set(tables) - RETIRED_REGISTRY_TABLES):
        errors.append(f"{table}: no current source declaration")
    for table in sorted(RETIRED_REGISTRY_TABLES - set(classified)):
        errors.append(f"{table}: retired migrator table must stay classified replace-registry")
    return errors


def repository_failures(root):
    try:
        manifest = json.loads((root / MANIFEST).read_text(encoding="utf-8"),
                              object_pairs_hook=unique_object)
        errors = validate(manifest, source_tables(root))
    except (OSError, ValueError) as error:
        errors = [str(error)]
    return [f"native table ownership: {error}" for error in errors]
