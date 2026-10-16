#!/usr/bin/env python3
"""Seed or reset the synthetic Theatre catalog in ONLY the retained disposable cluster.

Primitive of `cargo xtask seed theatre`. It never reads DATABASE_URL or any inherited
PostgreSQL setting. The data comes from the `theatre_development_catalog_seed` PostgreSQL
test: real Studio HTTP and app-role writes through the domain's rights, approval and
publication rules, with only the test-only public-delivery observer substituted. The host
that later serves the database keeps publication delivery and playback unavailable.
"""
import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / "target" / "revision-evidence"
RUN_RECORD = EVIDENCE / "run.json"
SEED_RECORD = EVIDENCE / "theatre-seed.json"
SEED_TEST = ("audio_lifecycle_http::publication_http::theatre_dev_seed_http::"
             "theatre_development_catalog_seed")
SEED_DATABASE = re.compile(r"cantos_test_[0-9a-f]{32}")
FATES = ("public", "withdrawn", "private", "unpublished")
KEEP_HINT = "run `cargo xtask verify postgres --keep` first"


class Refusal(Exception):
    """A precondition that makes the target unsafe or unknown; nothing was changed."""


def retained_cluster(config):
    """(port, data directory) only for a cluster this repository's runner created and kept."""
    if config.get("retained") is not True or "theatre_app_url" not in config:
        raise Refusal(f"no retained disposable cluster with a Theatre fixture; {KEEP_HINT}")
    port = config.get("port")
    if not isinstance(port, int) or isinstance(port, bool) or not 1024 <= port <= 65535:
        raise Refusal(f"retained cluster record has no valid loopback port; {KEEP_HINT}")
    evidence = EVIDENCE.resolve()
    cluster = Path(str(config.get("cluster", ""))).resolve()
    if cluster.parent != evidence or not re.fullmatch(r"cluster-[0-9]+", cluster.name):
        raise Refusal(f"retained cluster is not owned by this checkout's runner; {KEEP_HINT}")
    data = cluster / "pgdata"
    if not (data / "PG_VERSION").is_file():
        raise Refusal(f"retained cluster data directory is missing; {KEEP_HINT}")
    return port, data


def admin_url(port):
    return f"postgresql://cantos_test_admin@127.0.0.1:{port}/postgres"


def app_url(port, database):
    return f"postgresql://cantos_app@127.0.0.1:{port}/{database}"


def protected_databases(config):
    """The verification fixtures the record still points at; a seed never drops them."""
    names = set()
    for key in ("app_url", "catalog_app_url", "audio_app_url", "theatre_app_url"):
        value = config.get(key)
        if isinstance(value, str):
            names.add(urlsplit(value).path.lstrip("/"))
    return names


def owned_seed_database(config, seed):
    """The previous seed database only when its record names this exact retained cluster."""
    if not isinstance(seed, dict):
        return None
    database = seed.get("database")
    if seed.get("cluster") != config.get("cluster") or seed.get("port") != config.get("port"):
        return None
    if not isinstance(database, str) or not SEED_DATABASE.fullmatch(database):
        raise Refusal("seed record names an unsafe database; remove it and seed again")
    if database in protected_databases(config):
        raise Refusal("seed record names a verification fixture database")
    return database


def validated_result(result, config):
    """Check what the seed test wrote before it is recorded or served."""
    if not isinstance(result, dict):
        raise Refusal("seed test produced an unexpected record")
    database = result.get("database")
    if result.get("format") != 1 or not isinstance(database, str) \
            or not SEED_DATABASE.fullmatch(database) or database in protected_databases(config):
        raise Refusal("seed test produced an unexpected record")
    works = result.get("works")
    if not isinstance(works, list) or not works:
        raise Refusal("seed test recorded no works")
    for work in works:
        if not isinstance(work, dict) or work.get("fate") not in FATES \
                or not isinstance(work.get("units"), list) or not work["units"]:
            raise Refusal("seed test recorded an invalid work")
    return result


def isolated_environment(inherited):
    """libpq PG* and CANTOS_* settings could redirect a connection; drop them all."""
    return {key: value for key, value in inherited.items()
            if not key.startswith(("PG", "CANTOS_")) and key != "DATABASE_URL"}


def psql(env, port, sql):
    result = subprocess.run(["psql", "-X", admin_url(port), "-v", "ON_ERROR_STOP=1", "-At", "-c", sql],
                            env=env, text=True, capture_output=True, timeout=30)
    if result.returncode:
        raise Refusal(f"retained cluster on 127.0.0.1:{port} is not reachable "
                      f"(stopped or rebooted?); {KEEP_HINT}")
    return result.stdout.strip()


def confirm_cluster(env, port, data):
    """The port must still be served by the retained data directory, not another server."""
    observed = psql(env, port, "SHOW data_directory")
    if Path(observed).resolve() != data.resolve():
        raise Refusal(f"127.0.0.1:{port} is served by another PostgreSQL data directory; {KEEP_HINT}")


def drop(env, port, database, config):
    if not SEED_DATABASE.fullmatch(database) or database in protected_databases(config):
        raise Refusal("refusing to drop a database that is not a recorded seed")
    psql(env, port, f'DROP DATABASE IF EXISTS "{database}" WITH (FORCE)')


def write_record(path, value):
    temporary = path.with_name(path.name + ".tmp")
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")
    os.replace(temporary, path)


def summary(record):
    lines = [f"Theatre seed catalog in {record['database']} "
             f"(rights valid until {record['rights_valid_until_ms']} ms):"]
    for work in record["works"]:
        visible = 0 if work["fate"] != "public" else sum(
            1 for unit in work["units"] if unit["publication_id"] and not unit["withdrawn"])
        lines.append(f"  {work['fate']:<11} {work['kind']:<7} {work['work_id']}  "
                     f"{visible}/{len(work['units'])} public units  {work['title']}")
    lines.append("Serve it with `cargo xtask run cantos-theatre --web --test-cluster`; "
                 "publication delivery and playback stay unavailable.")
    return "\n".join(lines)


def load(path):
    return json.loads(path.read_text()) if path.is_file() else None


def seed(config, env):
    port, data = retained_cluster(config)
    confirm_cluster(env, port, data)
    previous = owned_seed_database(config, load(SEED_RECORD))
    with tempfile.TemporaryDirectory(dir=EVIDENCE) as directory:
        output = Path(directory) / "seed.json"
        test_env = dict(env, CANTOS_TEST_CLUSTER_URL=admin_url(port),
                        CANTOS_THEATRE_SEED_RECORD=str(output))
        command = ["cargo", "test", "-p", "cantos-api", "--test", "catalog_http_postgres",
                   "--locked", "--offline", "--", "--ignored", "--exact", SEED_TEST,
                   "--test-threads=1"]
        if subprocess.run(command, cwd=ROOT, env=test_env).returncode:
            raise Refusal("the seed test failed; no seed record was written")
        if not output.is_file():
            raise Refusal("the seed test selected nothing or wrote no record")
        result = validated_result(json.loads(output.read_text()), config)
    record = dict(result, cluster=config["cluster"], port=port,
                  app_url=app_url(port, result["database"]))
    write_record(SEED_RECORD, record)
    if previous and previous != record["database"]:
        drop(env, port, previous, config)
        print(f"Dropped the previous seed database {previous}.")
    print(summary(record))


def reset(config, env):
    port, data = retained_cluster(config)
    confirm_cluster(env, port, data)
    record = load(SEED_RECORD)
    database = owned_seed_database(config, record)
    if database:
        drop(env, port, database, config)
        print(f"Dropped seed database {database}.")
    elif record is not None:
        print("The seed record belonged to another cluster; only the record was removed.")
    else:
        print("No Theatre seed record; nothing to reset.")
    SEED_RECORD.unlink(missing_ok=True)
    print("`cargo xtask run cantos-theatre --web --test-cluster` serves the empty fixture again.")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reset", action="store_true",
                        help="drop only the recorded seed database and its record")
    args = parser.parse_args(argv)
    config = load(RUN_RECORD)
    try:
        if config is None:
            raise Refusal(f"no retained cluster record; {KEEP_HINT}")
        env = isolated_environment(os.environ)
        (reset if args.reset else seed)(config, env)
    except Refusal as refusal:
        print(f"REFUSED: {refusal}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
