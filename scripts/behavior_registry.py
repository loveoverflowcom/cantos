"""Federated behavior registry: one root registry, optional module-owned manifests.

`specs/behavior/catalog.json` stays the canonical registry. A capability is either the
current inline record or a root pointer (`id`, `owner`, `lifecycle`, `manifestPath`) whose
verification payload lives in a module-owned JSON manifest. Both resolve to the same
normalized capability object, so the verifier and the registered conformance adapters read
one representation. This module resolves and checks registered sources; it never runs Quint,
Node or any observer.
"""

import hashlib
import json
import os
import posixpath
import re
from pathlib import Path

CATALOG = "specs/behavior/catalog.json"
MAX_BYTES = 4 * 1024 * 1024
NAME = re.compile(r"^[A-Za-z][A-Za-z0-9_]*$")
STABLE_ID = re.compile(r"^[a-z][a-z0-9]*(?:[.-][a-z0-9]+)*$")
LIFECYCLES = {"draft", "accepted", "deprecated", "needs-decision"}

# The root registry owns identity, ownership and lifecycle; a manifest owns everything else.
AUTHORITY_KEYS = frozenset({"id", "owner", "lifecycle", "manifestPath"})
PAYLOAD_KEYS = frozenset({
    "model", "module", "scenarios", "scenarioModule", "simulation", "properties", "runs",
    "witnesses", "controls", "sourceRefs", "inventory", "aliases", "reason",
    "observationSchema", "implementationConformance", "ownedSources", "implementationTests"})
MANIFEST_KEYS = PAYLOAD_KEYS | {"schemaVersion", "capabilityId"}
HELPER_KEYS = frozenset({"path", "reason"})
MAX_OWNED = 64
# Orphan detection is bounded to the trees that may own behavior; build output is never scanned.
SCAN_ROOTS = ("specs", "services", "apps", "xtask")
PRUNED = {"target", "node_modules", ".git", "build", ".gradle"}
MAX_SCANNED = 200000
SHARED = "#shared"

QUINT_TOKEN = re.compile(r'"(?:\\.|[^"\\\n])*"|//[^\n]*|/\*.*?\*/', re.S)
QUINT_IMPORT = re.compile(r'^\s*(?:import|export)\b[^"\n;]*?\bfrom\s+"([^"\n]*)"', re.M)
QUINT_STATE = re.compile(r"^\s*(?:var|action)\b", re.M)


class BehaviorError(Exception):
    pass


def require(condition, message):
    if not condition:
        raise BehaviorError(message)


def regular(path, limit=MAX_BYTES):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > limit:
        raise BehaviorError(f"missing, linked, special or oversized input: {path.name}")
    return path.read_bytes()


def digest(path, limit=MAX_BYTES):
    return hashlib.sha256(regular(path, limit)).hexdigest()


def read_json(path):
    try:
        return json.loads(regular(path))
    except (ValueError, UnicodeError) as error:
        raise BehaviorError(f"invalid JSON: {path.name}") from error


def _unlinked(root, result):
    require(result.resolve().is_relative_to(root.resolve()), "input path escapes the repository")
    for ancestor in (result, *result.parents):
        if ancestor == root:
            break
        require(not ancestor.is_symlink(), "linked input paths are not supported")


def normalized(relative):
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise BehaviorError("invalid repository-relative input path")
    if any(part in ("", ".", "..") for part in relative.split("/")) or relative.startswith("/"):
        raise BehaviorError("input path must be normalized and repository-relative")
    return relative


def local_path(root, relative):
    if not isinstance(relative, str) or not relative or "\\" in relative:
        raise BehaviorError("invalid repository-relative input path")
    plain = relative.split("#", 1)[0]
    normalized(plain)
    result = root / Path(plain)
    _unlinked(root, result)
    regular(result)
    return result


def local_directory(root, prefix):
    result = root / Path(normalized(prefix))
    _unlinked(root, result)
    require(result.is_dir(), "owned directory is missing")
    return result


def boundary(path):
    """The owning service/app/tool tree of a repository path; behavior may not cross it."""
    parts = path.split("/")
    return "/".join(parts[:2]) if parts[0] in ("services", "apps") and len(parts) > 2 else parts[0]


# --- registry entries -------------------------------------------------------------------------

def resolve_entry(root, entry, manifests):
    """Return the one normalized capability for an inline record or a root manifest pointer."""
    require(isinstance(entry, dict), "capability must be an object")
    if "manifestPath" not in entry:
        return entry
    extra = sorted(set(entry) - AUTHORITY_KEYS)
    require(not extra, "root pointer duplicates verification fields: " + ", ".join(extra))
    pointer = entry["manifestPath"]
    require(isinstance(pointer, str) and pointer.endswith(".json") and "#" not in pointer,
            "manifestPath must name a JSON file")
    require(pointer != CATALOG, "manifest cannot be the root registry")
    require(pointer not in manifests, "manifest is referenced by more than one capability")
    manifests.add(pointer)
    manifest = read_json(local_path(root, pointer))
    require(isinstance(manifest, dict), "manifest must be an object")
    require(manifest.get("schemaVersion") == 1, "unsupported behavior manifest schema")
    require(manifest.get("capabilityId") == entry.get("id"),
            "manifest capabilityId differs from the registry ID")
    authority = sorted(set(manifest) & AUTHORITY_KEYS)
    require(not authority, "manifest duplicates root-owned fields: " + ", ".join(authority))
    unknown = sorted(set(manifest) - MANIFEST_KEYS)
    require(not unknown, "manifest has unknown fields: " + ", ".join(unknown))
    payload = {key: value for key, value in manifest.items() if key in PAYLOAD_KEYS}
    return {**payload, **entry}


def resolve_capabilities(root, catalog):
    capabilities = catalog.get("capabilities")
    require(isinstance(capabilities, list) and capabilities, "empty capability catalog")
    manifests = set()
    return [resolve_entry(root, entry, manifests) for entry in capabilities]


def shared_helpers(root, catalog):
    """Reviewed, explicitly registered stateless helper modules shared across capabilities."""
    rows = catalog.get("sharedHelpers", [])
    require(isinstance(rows, list), "sharedHelpers must be a list")
    helpers = []
    for row in rows:
        require(isinstance(row, dict) and set(row) == HELPER_KEYS, "invalid shared helper record")
        path, reason = row["path"], row["reason"]
        require(isinstance(path, str) and path.endswith(".qnt") and path not in helpers,
                "invalid or duplicate shared helper path")
        require(isinstance(reason, str) and bool(reason), "shared helper needs a reason")
        local_path(root, path)
        helpers.append(path)
    return sorted(helpers)


def owned_sources(root, cap):
    """Validate the optional bounded implementation metadata of one capability."""
    for key in ("ownedSources", "implementationTests"):
        rows = cap.get(key)
        if rows is None:
            continue
        require(isinstance(rows, list) and 0 < len(rows) <= MAX_OWNED and len(set(map(str, rows))) == len(rows),
                f"{key} must be a nonempty, unique, bounded list")
        for row in rows:
            require(isinstance(row, str) and "#" not in row, f"invalid {key} path")
            if row.endswith("/"):
                prefix = row[:-1]
                require(len(normalized(prefix).split("/")) >= 3, f"{key} directory is too broad")
                local_directory(root, prefix)
            else:
                local_path(root, row)


# --- Quint sources ----------------------------------------------------------------------------

def strip_comments(text):
    return QUINT_TOKEN.sub(lambda m: m.group(0) if m.group(0).startswith('"') else " ", text)


def local_imports(root, path):
    """Repository-relative files `path` imports; only `./` and `../` specifiers are local."""
    text = strip_comments(regular(root / path).decode("utf-8"))
    targets = []
    for specifier in QUINT_IMPORT.findall(text):
        require(specifier.startswith(("./", "../")), "behavior import is not a local relative path")
        target = posixpath.normpath(posixpath.join(posixpath.dirname(path), specifier))
        target = target if target.endswith(".qnt") else target + ".qnt"
        require(not target.startswith("../"), "behavior import escapes the repository")
        local_path(root, target)
        targets.append(target)
    return targets


def discover_qnt(root):
    found, scanned = set(), 0
    for tree in SCAN_ROOTS:
        base = root / tree
        if not base.is_dir() or base.is_symlink():
            continue
        for directory, names, files in os.walk(base):
            names[:] = sorted(name for name in names if name not in PRUNED)
            scanned += len(files) + len(names)
            require(scanned <= MAX_SCANNED, "behavior source discovery exceeded its bound")
            for name in files:
                if name.endswith(".qnt"):
                    found.add(str((Path(directory) / name).relative_to(root).as_posix()))
    return found


def check_sources(root, capabilities, helpers):
    """Register every Quint file, resolve each capability's import closure and refuse orphans.

    A file is registered when a capability names it (`model`/`scenarios`), a root helper record
    names it, or a registered file of the same boundary imports it. Cross-boundary imports are
    allowed only into registered stateless helpers. Mutates `importClosure` on each capability.
    """
    explicit = {}
    for cap in capabilities:
        for key in ("model", "scenarios"):
            path = cap.get(key)
            if path is None:
                continue
            require(isinstance(path, str) and path.endswith(".qnt"), f"{key} must be a Quint file")
            require(explicit.setdefault(path, cap["id"]) == cap["id"], "Quint file registered twice")
    for path in helpers:
        require(path not in explicit, "shared helper is also a capability model")
        explicit[path] = SHARED
        text = strip_comments(regular(root / path).decode("utf-8"))
        require(QUINT_STATE.search(text) is None, "shared helper must be stateless")
    graph, registered = {}, set(explicit)

    def edges(path):
        if path not in graph:
            graph[path] = local_imports(root, path)
        return graph[path]

    def visit(path, stack, closure, helper_only):
        require(path not in stack, "cyclic behavior import")
        if path in closure:
            return
        stack.append(path)
        for target in edges(path):
            into_helper = explicit.get(target) == SHARED
            require(into_helper or not helper_only, "shared helper imports a non-helper")
            require(into_helper or boundary(target) == boundary(path),
                    "cross-boundary behavioral import: " + path + " -> " + target)
            registered.add(target)
            visit(target, stack, closure, into_helper)
        stack.pop()
        closure.add(path)

    for cap in capabilities:
        closure = set()
        for key in ("model", "scenarios"):
            if cap.get(key):
                visit(cap[key], [], closure, False)
        cap["importClosure"] = sorted(closure - {cap.get("model"), cap.get("scenarios")})
    for path in helpers:
        visit(path, [], set(), True)
    orphans = sorted(discover_qnt(root) - registered)
    require(not orphans, "unregistered executable Quint file: " + ", ".join(orphans[:5]))


# --- reverse dependency index -----------------------------------------------------------------

def impact_index(capabilities, helpers):
    """changed path -> affected runnable capability IDs ('*' means every model).

    `inputs` are executable behavior files (Quint closure, helpers, manifests). `sources` are
    non-document production references that select the model run. `conformance` paths select the
    registered observer. Documents never select a run; the repository gate owns them.
    """
    runnable = sorted(c["id"] for c in capabilities if c.get("model") is not None)
    observed = sorted(c["id"] for c in capabilities
                      if c.get("model") is not None and c.get("implementationConformance") is not None)
    index = {"schemaVersion": 1, "capabilities": runnable, "observed": observed,
             "inputs": {}, "sources": {}, "conformance": {},
             "sourcePrefixes": {}, "conformancePrefixes": {}}

    def add(table, path, identity):
        table.setdefault(path, set()).add(identity)

    for cap in capabilities:
        identity = cap["id"] if cap["id"] in runnable else "*"
        files = [p for p in (cap.get("model"), cap.get("scenarios"), cap.get("manifestPath")) if p]
        for path in [*files, *cap.get("importClosure", [])]:
            add(index["inputs"], path, identity)
        if identity == "*":
            continue
        refs = [cap["owner"], *(r.split("#", 1)[0] for r in cap["sourceRefs"])]
        owned = [*cap.get("ownedSources", []), *cap.get("implementationTests", [])]
        for path in [*refs, *owned]:
            if path.endswith(".md"):
                continue
            table = index["sourcePrefixes"] if path.endswith("/") else index["sources"]
            add(table, path, identity)
        if cap["id"] in observed:
            for path in [cap["owner"], *owned]:
                if not path.endswith(".md"):
                    table = index["conformancePrefixes"] if path.endswith("/") else index["conformance"]
                    add(table, path, identity)
    for path in helpers:
        index["inputs"].setdefault(path, {"*"})
    for key in ("inputs", "sources", "conformance", "sourcePrefixes", "conformancePrefixes"):
        index[key] = {path: sorted(ids) for path, ids in sorted(index[key].items())}
    return index
