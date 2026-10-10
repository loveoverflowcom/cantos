#!/usr/bin/env python3
"""Initialize private story folders outside Git; never fetch, sync, or update stages.

Exclusive file creation preserves existing files but is not a multi-file transaction.
If initialization crashes, malformed or missing story identity blocks the next run.
Restore identity from a trusted backup or Drive record; otherwise inspect and move
aside the incomplete directory before retrying. Never replace an existing story's
ID to hide damage. Inspect partially written indexes and name maps before resuming.
"""

import argparse
import json
import re
import sys
import unicodedata
import uuid
from pathlib import Path
from urllib.parse import unquote_plus, urlsplit, urlunsplit


DRIVE_ROOT_ID = "185z1tU88YvQkcS6mjCQsXhxiYwebkhqJ"
STAGES = {
    "raw": "Original source captures and extraction records",
    "score": "Reviewed dramatic adaptations and performance cues",
    "audio": "Final renders and their production and audit records",
}
TRACKING_PARAMETERS = {
    "utm_source", "utm_medium", "utm_campaign", "utm_term", "utm_content",
    "utm_id", "gclid", "fbclid", "msclkid",
}


class WorkspaceError(ValueError):
    """An input or existing workspace needs human identity or filesystem review."""


def clean_text(value, field):
    if not isinstance(value, str) or not value.strip():
        raise WorkspaceError(f"{field} must be nonempty text")
    if any(unicodedata.category(char) == "Cc" for char in value):
        raise WorkspaceError(f"{field} must not contain control characters")
    return unicodedata.normalize("NFC", value.strip())


def folder_name(title):
    folded = unicodedata.normalize("NFKD", title.replace("Đ", "D").replace("đ", "d"))
    ascii_title = folded.encode("ascii", "ignore").decode("ascii").lower()
    slug = re.sub(r"[^a-z0-9]+", "_", ascii_title).strip("_")[:96].rstrip("_")
    if not slug:
        raise WorkspaceError("title has no ASCII letters or digits for a folder name")
    return slug


def canonical_url(value):
    value = clean_text(value, "source URL")
    if any(char.isspace() for char in value):
        raise WorkspaceError("source URL must use percent encoding for whitespace")
    try:
        parts = urlsplit(value)
        if parts.scheme.lower() not in {"http", "https"} or not parts.hostname:
            raise ValueError("an absolute HTTP(S) URL is required")
        if parts.username is not None or parts.password is not None:
            raise ValueError("credentials are not allowed in a source URL")
        scheme = parts.scheme.lower()
        host = parts.hostname.lower()
        host = f"[{host}]" if ":" in host else host
        port = parts.port
        if port is not None and (scheme, port) not in {("http", 80), ("https", 443)}:
            host = f"{host}:{port}"
    except ValueError as error:
        raise WorkspaceError(f"invalid source URL: {error}") from error
    # Preserve content parameters, their order, and their original encoding.
    query = "&".join(
        part for part in parts.query.split("&")
        if unquote_plus(part.partition("=")[0]).lower() not in TRACKING_PARAMETERS
    )
    return urlunsplit((scheme, host, parts.path or "/", query, ""))


def canonical_id(value):
    try:
        return str(uuid.UUID(value))
    except (ValueError, TypeError, AttributeError) as error:
        raise WorkspaceError("story_id must be a UUID") from error


def outside_git(path):
    for ancestor in (path, *path.parents):
        marker = ancestor / ".git"
        # Sandboxes may expose an empty .git directory placeholder. A real Git
        # directory has HEAD; linked worktrees use a gitdir indirection file.
        is_directory = marker.is_dir() and (marker / "HEAD").exists()
        is_indirection = marker.is_file() and marker.read_bytes().startswith(b"gitdir:")
        if is_directory or is_indirection:
            raise WorkspaceError(f"data must stay outside Git worktrees: {path}")


def safe_path(path, *, directory):
    if path.is_symlink():
        raise WorkspaceError(f"refusing a symlink in the workspace: {path}")
    expected_type = path.is_dir() if directory else path.is_file()
    if path.exists() and not expected_type:
        raise WorkspaceError(f"unexpected filesystem entry: {path}")
    outside_git(path.resolve())


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise WorkspaceError(f"duplicate metadata key: {key}")
        result[key] = value
    return result


def read_identity(path):
    safe_path(path.parent, directory=True)
    safe_path(path, directory=False)
    try:
        record = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
        if not isinstance(record, dict) or record.get("schema_version") != 1:
            raise WorkspaceError("unsupported story.json schema")
        if record["story_id"] != canonical_id(record["story_id"]):
            raise WorkspaceError("story_id is not in canonical UUID form")
        if record["folder_name"] != path.parent.name:
            raise WorkspaceError("folder_name does not match its directory")
        if not re.fullmatch(r"[a-z0-9]+(?:_[a-z0-9]+)*", record["folder_name"]):
            raise WorkspaceError("folder_name must be ASCII snake_case")
        if clean_text(record["title"], "title") != record["title"]:
            raise WorkspaceError("title is not normalized")
        if record["author"] is not None:
            if clean_text(record["author"], "author") != record["author"]:
                raise WorkspaceError("author is not normalized")
        if canonical_url(record["canonical_source_url"]) != record["canonical_source_url"]:
            raise WorkspaceError("canonical_source_url is not normalized")
        sync = record["sync"]
        if sync["drive_root_folder_id"] != DRIVE_ROOT_ID:
            raise WorkspaceError("Drive root differs from this project")
        if sync["state"] not in {"pending_sync", "synced", "blocked", "sync_conflict"}:
            raise WorkspaceError("unknown sync state")
        remote = sync["drive_story_folder_id"]
        if remote is not None and (not isinstance(remote, str) or not remote.strip()):
            raise WorkspaceError("invalid Drive story folder ID")
        if sync["state"] == "synced" and remote is None:
            raise WorkspaceError("synced metadata lacks a Drive story folder ID")
        return record
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise WorkspaceError(f"invalid metadata at {path}: {error}; review it without overwriting") from error


def choose_identity(root, title, author, source_url, story_id):
    records = []
    if root.exists():
        for child in sorted(root.iterdir()):
            metadata = child / "story.json"
            if metadata.exists() or metadata.is_symlink():
                records.append((child, read_identity(metadata)))
    ids = [record["story_id"] for _, record in records]
    urls = [record["canonical_source_url"] for _, record in records]
    if len(ids) != len(set(ids)) or len(urls) != len(set(urls)):
        raise WorkspaceError("duplicate story IDs or source URLs; reconcile identities first")
    matches = [
        (path, record) for path, record in records
        if record["canonical_source_url"] == source_url
        or (story_id is not None and record["story_id"] == story_id)
    ]
    if matches:
        if len(matches) != 1:
            raise WorkspaceError("source URL and story_id resolve to different stories")
        path, record = matches[0]
        if record["canonical_source_url"] != source_url:
            raise WorkspaceError("story_id belongs to a different source URL; review the identity")
        if story_id is not None and record["story_id"] != story_id:
            raise WorkspaceError("source URL belongs to a different story_id")
        if record["title"] != title or (author is not None and record["author"] != author):
            raise WorkspaceError("source identity has a different title or author; review metadata")
        return path, record, False
    if story_id is not None:
        raise WorkspaceError("story_id was not found; restore its persisted story.json first")
    path = root / folder_name(title)
    if path.exists() or path.is_symlink():
        raise WorkspaceError(
            f"ambiguous folder collision at {path}; review source and author, then use a "
            "distinctive title or restore the matching story.json"
        )
    return path, {
        "schema_version": 1,
        "story_id": str(uuid.uuid4()),
        "folder_name": path.name,
        "title": title,
        "author": author,
        "canonical_source_url": source_url,
        "sync": {
            "state": "pending_sync",
            "drive_root_folder_id": DRIVE_ROOT_ID,
            "drive_story_folder_id": None,
        },
    }, True


def markdown_text(value):
    return re.sub(r"([\\`*_{}\[\]<>#!|])", r"\\\1", value)


def initial_index(record, stage=None, *, created=True):
    title = markdown_text(record["title"])
    heading = f"{title} — {stage}" if stage else title
    scope = STAGES[stage] if stage else "Story handoff across raw, score, and audio"
    name_map_rule = (
        "Review `name_map.yml` against every source proper name before adaptation; "
        "an empty map is incomplete.\n\n" if stage == "score" else ""
    )
    state = "not_started" if created else "needs_review"
    log = (
        "Workspace initialized locally; no artifacts captured or generated."
        if created else "Missing index initialized; existing artifacts require inventory review."
    )
    return (
        f"# {heading}\n\n{scope}.\n\n{name_map_rule}"
        f"- Story ID: `{record['story_id']}`\n"
        f"- Source: {markdown_text(record['canonical_source_url'])}\n"
        f"- State: {state}\n"
        "- Sync state: pending_sync (initializer only; no remote operation occurred)\n"
        "- Source revision / SHA-256: pending capture\n"
        "- Drive folder ID: pending\n\n"
        "## Artifact manifest\n\n"
        "| Path | SHA-256 | Input revisions / hashes | Status | Drive file ID |\n"
        "| --- | --- | --- | --- | --- |\n\n"
        "## Update log\n\n"
        f"- {log}\n\n"
        "## Blockers\n\n"
        "- Confirm source inventory and Drive synchronization before advancing this report.\n"
    )


def write_new(path, content):
    if path.exists():
        return False
    with path.open("x", encoding="utf-8", newline="\n") as output:
        output.write(content)
    return True


def prepare_workspace(root, title, source_url, author=None, story_id=None):
    root = Path(root).expanduser().resolve()
    safe_path(root, directory=True)
    title = clean_text(title, "title")
    author = clean_text(author, "author") if author is not None else None
    source_url = canonical_url(source_url)
    story_id = canonical_id(story_id) if story_id is not None else None
    story_path, record, created = choose_identity(root, title, author, source_url, story_id)
    for directory in (story_path, *(story_path / stage for stage in STAGES)):
        safe_path(directory, directory=True)
        safe_path(directory / "index.md", directory=False)
    name_map = story_path / "score" / "name_map.yml"
    safe_path(name_map, directory=False)
    safe_path(root / "index.md", directory=False)
    root.mkdir(parents=True, exist_ok=True)
    story_path.mkdir(exist_ok=not created)
    if created:
        write_new(story_path / "story.json", json.dumps(record, ensure_ascii=False, indent=2) + "\n")
    for stage in STAGES:
        (story_path / stage).mkdir(exist_ok=True)
        write_new(story_path / stage / "index.md", initial_index(record, stage, created=created))
    name_map_created = write_new(name_map, (
        "# Inventory is pending; an empty map is not complete coverage.\n"
        "schema_version: 1\n"
        "revision: 1\n"
        f"story_id: {json.dumps(record['story_id'])}\n"
        "inventory_status: pending\n"
        "entities: []\n"
        "unresolved: []\n"
    ))
    write_new(story_path / "index.md", initial_index(record, created=created))
    write_new(root / "index.md", (
        "# Cantos story workspace\n\n"
        f"Drive project root: `{DRIVE_ROOT_ID}`.\n\n"
        "Each story directory has an authoritative identity in `story.json`.\n"
        "The initializer preserves this index. Agents maintain this catalog and stage reports "
        "after each workflow operation and confirmed sync.\n\n"
        "| Story folder | Story ID | State | Drive folder ID |\n"
        "| --- | --- | --- | --- |\n"
    ))
    return {
        "path": str(story_path), "created": created,
        "name_map_created": name_map_created, "story": record,
    }


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", required=True, help="explicit local data root outside Git")
    parser.add_argument("--title", required=True)
    parser.add_argument("--author")
    parser.add_argument("--source-url", required=True, help="resolved canonical work URL")
    parser.add_argument("--story-id", help="reuse an existing persisted UUID")
    args = parser.parse_args(argv)
    try:
        result = prepare_workspace(args.root, args.title, args.source_url, args.author, args.story_id)
    except (WorkspaceError, OSError) as error:
        parser.exit(2, f"error: {error}\n")
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
