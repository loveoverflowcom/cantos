#!/usr/bin/env python3
"""Serve the API against ONLY the disposable cluster retained by test_postgres.py."""
import json
import os
import re
from pathlib import Path
from urllib.parse import urlparse

from retained_postgres import OWNERSHIP_RECORD, confirm_running, validate_ownership

ROOT = Path(__file__).resolve().parents[1]
SEED_RECORD = "target/revision-evidence/theatre-seed.json"


def theatre_seed_url(config, seed):
    """The seeded app-role URL only when `cargo xtask seed theatre` recorded it for this cluster."""
    if not isinstance(seed, dict) or seed.get("cluster") != config.get("cluster") or seed.get("port") != config.get("port"):
        return None
    database = seed.get("database")
    if not isinstance(database, str) or not re.fullmatch(r"cantos_test_[0-9a-f]{32}", database):
        raise ValueError("unsafe Theatre seed record; run `cargo xtask seed theatre --reset`")
    return f"postgresql://cantos_app@127.0.0.1:{config['port']}/{database}"


def theatre_seed_media(config, seed):
    """Only retained seed-owned storage may enable media on the disposable API host."""
    fields = ("private_root", "publication_root", "playback_key_file")
    if not any(field in seed for field in fields):
        return {}  # Older catalog-only receipts leave public delivery unavailable.
    if not all(isinstance(seed.get(field), str) for field in fields):
        raise ValueError("incomplete Theatre seed media receipt; seed again")
    cluster = Path(config["cluster"]).resolve()
    paths = [Path(seed[field]) for field in fields]
    if any(not path.is_absolute() or path.is_symlink() or not path.exists()
           or not path.resolve().is_relative_to(cluster) for path in paths):
        raise ValueError("Theatre seed media must remain within its retained cluster")
    private, publication, key = paths
    if (private.parent != publication.parent or key.parent != private.parent
            or not private.parent.name.startswith("theatre-seed-")
            or private.name != "private" or publication.name != "publication" or key.name != "playback.key"):
        raise ValueError("unexpected Theatre seed media layout")
    return {"CANTOS_AUDIO_ROOT": str(private), "CANTOS_PUBLICATION_ROOT": str(publication),
            "CANTOS_PLAYBACK_KEY_FILE": str(key), "CANTOS_PUBLIC_MEDIA_ORIGIN": "https://localhost:8080"}


def studio_environment(config, inherited, seed=None):
    """Select only a database emitted by the fresh disposable runner, never an inherited URL."""
    context = inherited.get("CANTOS_TEST_CONTEXT", "editorial")
    fields = {"editorial": "app_url", "catalog": "catalog_app_url", "audio": "audio_app_url",
              "theatre": "theatre_app_url"}
    if context not in fields:
        raise ValueError("test context must be editorial, catalog, audio or theatre")
    seeded = theatre_seed_url(config, seed) if context == "theatre" else None
    if config.get("purpose") == "retained-disposable-cluster":
        if context != "theatre" or config.get("retained") is not True or seeded is None:
            raise ValueError("retained cluster ownership requires a matching Theatre seed receipt")
    elif context == "theatre" and (config.get("retained") is not True or "theatre_app_url" not in config):
        raise ValueError("Theatre requires a retained test fixture; run `cargo xtask verify postgres --keep` again")
    app_url = seeded or config[fields[context]]
    url = urlparse(app_url)
    if (url.scheme != "postgresql" or url.hostname != "127.0.0.1"
            or url.username != "cantos_app" or url.password is not None
            or not url.path.startswith("/cantos_test_") or url.query or url.fragment):
        raise ValueError("expected disposable test database")
    env = dict(inherited, CANTOS_ENV="development", CANTOS_BIND_ADDRESS="127.0.0.1:8080", DATABASE_URL=app_url)
    if context == "theatre":
        for name in ("CANTOS_STORAGE_CONFIG", "CANTOS_AUDIO_ROOT", "CANTOS_PUBLICATION_ROOT",
                     "CANTOS_PUBLIC_MEDIA_ORIGIN", "CANTOS_PLAYBACK_KEY_FILE"):
            env.pop(name, None)
        if seeded:
            env.update(theatre_seed_media(config, seed))
    if context == "audio":
        root = Path(config["audio_root"])
        if not root.is_absolute():
            raise ValueError("expected absolute private test audio root")
        env["CANTOS_AUDIO_ROOT"] = str(root)
    return env


def serving_config(context, seed):
    """Ownership proves one seeded Theatre target, never the full runner's other fixtures."""
    ownership_path = ROOT / OWNERSHIP_RECORD
    if context == "theatre" and seed is not None and ownership_path.is_file():
        ownership = json.loads(ownership_path.read_text())
        if not isinstance(ownership, dict):
            raise ValueError("invalid retained cluster ownership record")
        if (not isinstance(seed, dict) or seed.get("cluster") != ownership.get("cluster")
                or seed.get("port") != ownership.get("port")):
            raise ValueError("Theatre seed does not match retained cluster ownership; seed again")
        validate_ownership(ownership, ROOT / "target/revision-evidence")
        return ownership
    return json.loads((ROOT / "target/revision-evidence/run.json").read_text())


def main():
    context = os.environ.get("CANTOS_TEST_CONTEXT", "editorial")
    seed_path = ROOT / SEED_RECORD
    seed = None
    if context == "theatre" and seed_path.is_file():
        seed = json.loads(seed_path.read_text())
        if not isinstance(seed, dict):
            raise ValueError("invalid Theatre seed receipt")
    config = serving_config(context, seed)
    env = studio_environment(config, os.environ, seed)
    if config.get("purpose") == "retained-disposable-cluster":
        confirm_running(config, ROOT / "target/revision-evidence", os.environ)
    os.chdir(ROOT)
    if context == "theatre" and theatre_seed_url(config, seed) is not None:
        public = [work for work in seed["works"] if work["fate"] == "public"]
        print(f"Theatre seed catalog: {len(public)} public works from `cargo xtask seed theatre` "
              f"(rights valid until {seed['rights_valid_until_ms']} ms); "
              "identity and native playback remain separate.", flush=True)
        if "CANTOS_PUBLICATION_ROOT" in env:
            print("Local publication storage enabled. Follow grant paths on loopback HTTP for API checks; "
                  "the HTTPS grant origin requires separately configured TLS for a native player.", flush=True)
    elif context == "theatre":
        if seed is not None:
            print("Theatre seed record belongs to another cluster; ignored.", flush=True)
        print("Theatre test fixture: empty public catalog; identity and playback remain separate.", flush=True)
        print("Seed synthetic works with `cargo xtask seed theatre`.", flush=True)
    else:
        print(f"Test script ID: {config['script']}", flush=True)
        print("Sign in with the synthetic alice test token (64 lowercase a characters).", flush=True)
    binary = str(ROOT / "target/debug/cantos-server")
    os.execve(binary, [binary], env)


if __name__ == "__main__":
    main()
