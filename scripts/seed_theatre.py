#!/usr/bin/env python3
"""Seed synthetic Theatre data through Studio use cases and real publication storage.

Default/reset operate only on the proven retained disposable cluster. Named persistent
inputs select an existing app DB/session explicitly, with no database administration;
a private write-ahead journal makes interrupted effects replay their original operations.
"""
import argparse
import fcntl
import json
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path
from urllib.parse import urlsplit
from retained_postgres import OWNERSHIP_RECORD, validate_ownership

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
    if config.get("purpose") == "retained-disposable-cluster":
        try:
            return validate_ownership(config,EVIDENCE)
        except ValueError as error:
            raise Refusal(str(error)) from None
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
    records=[config]
    if config.get("purpose")=="retained-disposable-cluster":
        complete=load(RUN_RECORD)
        if isinstance(complete,dict) and complete.get("cluster")==config.get("cluster") \
                and complete.get("port")==config.get("port"):
            records.append(complete)
    for record in records:
        for key in ("app_url", "catalog_app_url", "audio_app_url", "theatre_app_url"):
            value = record.get(key)
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
    valid_until = result.get("rights_valid_until_ms")
    if not isinstance(valid_until, int) or isinstance(valid_until, bool):
        raise Refusal("seed test recorded no rights window")
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
    for work in record["works"]:
        for unit in work["units"]:
            lines.append(f"    unit={unit['unit_id']} publication={unit['publication_id']} "
                         f"rendition={unit.get('rendition_id', 'unknown')}")
    lines.append("Manifest/grant/byte acceptance is separate from native playback.")
    return "\n".join(lines)


def load(path):
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text())
    except (OSError, ValueError) as error:
        raise Refusal(f"{path.name} is unreadable; reconcile it before running the command again") from error


def seed(config, env):
    port, data = retained_cluster(config)
    confirm_cluster(env, port, data)
    previous = owned_seed_database(config, load(SEED_RECORD))
    with tempfile.TemporaryDirectory(dir=EVIDENCE) as directory:
        output = Path(directory) / "seed.json"
        media = Path(tempfile.mkdtemp(prefix="theatre-seed-", dir=data.parent))
        os.chmod(media,0o700)
        private, publication = media / "private", media / "publication"
        private.mkdir(mode=0o700)
        publication.mkdir(mode=0o700)
        key = media / "playback.key"
        descriptor = os.open(key, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(descriptor,"wb") as stream:
            stream.write(os.urandom(32))
        test_env = dict(env, CANTOS_TEST_CLUSTER_URL=admin_url(port),
                        CANTOS_THEATRE_SEED_RECORD=str(output),
                        CANTOS_THEATRE_SEED_PRIVATE_ROOT=str(private),
                        CANTOS_THEATRE_SEED_PUBLICATION_ROOT=str(publication),
                        CANTOS_THEATRE_SEED_PLAYBACK_KEY_FILE=str(key))
        command = ["cargo", "test", "-p", "cantos-api", "--test", "catalog_http_postgres",
                   "--locked", "--offline", "--", "--ignored", "--exact", SEED_TEST,
                   "--test-threads=1"]
        if subprocess.run(command, cwd=ROOT, env=test_env).returncode:
            raise Refusal("the seed test failed; no seed record was written")
        if not output.is_file():
            raise Refusal("the seed test selected nothing or wrote no record")
        result = validated_result(json.loads(output.read_text()), config)
    record = dict(result, cluster=config["cluster"], port=port,
                  cluster_ownership=config.get("purpose"),
                  verification_scope=config.get("scope","full"),
                  verification_status=config.get("gate_status","PASS"),
                  full_postgres_status=config.get("full_gate_status","PASS"),
                  app_url=app_url(port, result["database"]))
    write_record(SEED_RECORD, record)
    if previous and previous != record["database"]:
        drop(env, port, previous, config)
        print(f"Dropped the previous seed database {previous}.")
    print(summary(record))
    print(f"Cluster verification: {record['verification_scope']} {record['verification_status']}; "
          f"full PostgreSQL gate {record['full_postgres_status']} (ownership never implies acceptance).")


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



def persistent_seed(args, inherited):
    """No database administration: use the chosen app connection and existing Studio session."""
    required = (args.api_origin, args.web_origin, args.database_url_env, args.studio_token_env, args.rights_ref,
                args.private_root, args.publication_root, args.record)
    if args.reset or not all(required):
        raise Refusal("persistent seed requires all explicit inputs and never permits --reset")
    cookie_name=args.studio_cookie_name or "__Secure-cantos_studio"
    if cookie_name not in ("__Secure-cantos_studio","cantos_session"):
        raise Refusal("unsupported Studio cookie name")
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]{0,62}", args.environment):
        raise Refusal("environment must be a lowercase name")
    for origin in (args.api_origin, args.web_origin):
        parsed = urlsplit(origin)
        loopback = parsed.hostname in ("127.0.0.1", "localhost", "::1")
        if parsed.scheme not in ("https", "http") or parsed.scheme == "http" and not loopback \
                or not parsed.hostname or parsed.username or parsed.password \
                or parsed.path or parsed.query or parsed.fragment or any(c.isspace() for c in origin):
            raise Refusal("API/Web origins must be bare HTTPS origins (HTTP only for loopback development)")
    selected = {}
    for flag, name in (("DATABASE_URL", args.database_url_env), ("STUDIO_TOKEN", args.studio_token_env)):
        if not re.fullmatch(r"[A-Z][A-Z0-9_]*", name) or name not in inherited or not inherited[name]:
            raise Refusal("explicit credential variable is absent or invalid (values are never printed)")
        selected["CANTOS_THEATRE_SEED_" + flag] = inherited[name]
    paths = [Path(value) for value in (args.private_root, args.publication_root, args.record)]
    if any(not path.is_absolute() or path.is_symlink() for path in paths):
        raise Refusal("persistent roots and receipt must be absolute and not symbolic links")
    private, publication, receipt = [path.resolve() for path in paths]
    if private == publication or private in publication.parents or publication in private.parents:
        raise Refusal("private and publication storage roots must be separate")
    if receipt == ROOT or ROOT in receipt.parents and "artifacts" not in receipt.relative_to(ROOT).parts:
        raise Refusal("keep the persistent receipt outside Git or under ignored artifacts/")
    if not private.is_dir() or not publication.is_dir() or not receipt.parent.is_dir():
        raise Refusal("operator must create the private roots and receipt directory first")
    env = isolated_environment(inherited)
    env.update(selected)
    env.update(CANTOS_THEATRE_SEED_ENVIRONMENT=args.environment,
               CANTOS_THEATRE_SEED_API_ORIGIN=args.api_origin,
               CANTOS_THEATRE_SEED_WEB_ORIGIN=args.web_origin,
               CANTOS_THEATRE_SEED_STUDIO_COOKIE_NAME=cookie_name,
               CANTOS_THEATRE_SEED_RIGHTS_REF=args.rights_ref,
               CANTOS_THEATRE_SEED_PRIVATE_ROOT=str(private),
               CANTOS_THEATRE_SEED_PUBLICATION_ROOT=str(publication),
               CANTOS_THEATRE_SEED_RECORD=str(receipt),
               CANTOS_THEATRE_SEED_JOURNAL=str(receipt.with_suffix(".journal.json")))
    lock_path = receipt.with_suffix(".lock")
    with lock_path.open("a") as lock:
        os.chmod(lock_path, 0o600)
        try:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except BlockingIOError as error:
            raise Refusal("this seed receipt is already in use") from error
        command = ["cargo", "test", "-p", "cantos-api", "--test", "catalog_http_postgres",
                   "--locked", "--offline", "--", "--ignored", "--exact", SEED_TEST,
                   "--test-threads=1"]
        if subprocess.run(command, cwd=ROOT, env=env).returncode:
            raise Refusal("seed incomplete; keep the private journal and rerun the same command to reconcile; "
                          "do not delete the journal or reset the database")
        result = load(receipt)
        if not isinstance(result, dict) or result.get("environment") != args.environment \
                or result.get("dataset") != "theatre-catalog-v2":
            raise Refusal("seed produced no matching persistent receipt")
    print(summary(result))
    print(f"Receipt: {receipt} (keep its private recovery journal; no database was reset).")

def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reset", action="store_true",
                        help="drop only the recorded seed database and its record")
    parser.add_argument("--environment")
    parser.add_argument("--api-origin")
    parser.add_argument("--web-origin")
    parser.add_argument("--database-url-env")
    parser.add_argument("--studio-token-env")
    parser.add_argument("--studio-cookie-name",choices=("__Secure-cantos_studio","cantos_session"))
    parser.add_argument("--rights-ref")
    parser.add_argument("--private-root")
    parser.add_argument("--publication-root")
    parser.add_argument("--record")
    args = parser.parse_args(argv)
    try:
        if args.environment:
            persistent_seed(args, os.environ)
            return 0
        if any((args.api_origin, args.web_origin, args.database_url_env, args.studio_token_env, args.studio_cookie_name, args.rights_ref,
                args.private_root, args.publication_root, args.record)):
            raise Refusal("persistent inputs require --environment")
        ownership_path=ROOT / OWNERSHIP_RECORD
        config = load(ownership_path) if ownership_path.is_file() else load(RUN_RECORD)
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
