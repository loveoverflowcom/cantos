#!/usr/bin/env python3
"""Serve Studio against ONLY the disposable cluster retained by test_postgres.py."""
import json
import os
from pathlib import Path
from urllib.parse import urlparse

ROOT = Path(__file__).resolve().parents[1]


def studio_environment(config, inherited):
    """Select only a database emitted by the fresh disposable runner, never an inherited URL."""
    context = inherited.get("CANTOS_TEST_CONTEXT", "editorial")
    fields = {"editorial": "app_url", "catalog": "catalog_app_url", "audio": "audio_app_url"}
    if context not in fields:
        raise ValueError("test context must be editorial, catalog or audio")
    app_url = config[fields[context]]
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
    env = studio_environment(config, os.environ)
    os.chdir(ROOT)
    print(f"Test script ID: {config['script']}", flush=True)
    print("Sign in with the synthetic alice test token (64 lowercase a characters).", flush=True)
    binary = str(ROOT / "target/debug/cantos-server")
    os.execve(binary, [binary], env)


if __name__ == "__main__":
    main()
