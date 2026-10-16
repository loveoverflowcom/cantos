#!/usr/bin/env python3
"""Serve the API against ONLY the disposable cluster retained by test_postgres.py."""
import json
import os
import re
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]
SEED_RECORD = "target/revision-evidence/theatre-seed.json"


def theatre_seed_url(config, seed):
    """The seeded app-role URL only when `cargo xtask seed theatre` recorded it for this cluster."""
    if seed is None or seed.get("cluster") != config.get("cluster") or seed.get("port") != config.get("port"):
        return None
    database = seed.get("database")
    if not isinstance(database, str) or not re.fullmatch(r"cantos_test_[0-9a-f]{32}", database):
        raise ValueError("unsafe Theatre seed record; run `cargo xtask seed theatre --reset`")
    return f"postgresql://cantos_app@127.0.0.1:{config['port']}/{database}"


def studio_environment(config, inherited, seed=None):
    """Select only a database emitted by the fresh disposable runner, never an inherited URL."""
    context = inherited.get("CANTOS_TEST_CONTEXT", "editorial")
    fields = {"editorial": "app_url", "catalog": "catalog_app_url", "audio": "audio_app_url",
              "theatre": "theatre_app_url"}
    if context not in fields:
        raise ValueError("test context must be editorial, catalog, audio or theatre")
    if context == "theatre" and (config.get("retained") is not True or "theatre_app_url" not in config):
        raise ValueError("Theatre requires a retained test fixture; run `cargo xtask verify postgres --keep` again")
    seeded = theatre_seed_url(config, seed) if context == "theatre" else None
    app_url = seeded or config[fields[context]]
    url = urlparse(app_url)
    if (url.scheme != "postgresql" or url.hostname != "127.0.0.1"
            or url.username != "cantos_app" or url.password is not None
            or not url.path.startswith("/cantos_test_") or url.query or url.fragment):
        raise ValueError("expected disposable test database")
    env = dict(inherited, CANTOS_ENV="development", CANTOS_BIND_ADDRESS="127.0.0.1:8080", DATABASE_URL=app_url)
    if context == "audio":
        root = Path(config["audio_root"])
        if not root.is_absolute():
            raise ValueError("expected absolute private test audio root")
        env["CANTOS_AUDIO_ROOT"] = str(root)
    return env


def main():
    config = json.loads((ROOT / "target/revision-evidence/run.json").read_text())
    seed_path = ROOT / SEED_RECORD
    seed = json.loads(seed_path.read_text()) if os.environ.get("CANTOS_TEST_CONTEXT") == "theatre" \
        and seed_path.is_file() else None
    env = studio_environment(config, os.environ, seed)
    os.chdir(ROOT)
    if env.get("CANTOS_TEST_CONTEXT") == "theatre" and env["DATABASE_URL"] != config["theatre_app_url"]:
        public = [work for work in seed["works"] if work["fate"] == "public"]
        print(f"Theatre seed catalog: {len(public)} public works from `cargo xtask seed theatre` "
              f"(rights valid until {seed['rights_valid_until_ms']} ms); "
              "identity, publication delivery and playback remain separate.", flush=True)
    elif env.get("CANTOS_TEST_CONTEXT") == "theatre":
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
