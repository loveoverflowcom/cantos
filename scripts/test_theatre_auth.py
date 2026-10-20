#!/usr/bin/env python3
"""Implementation primitive for explicit `cargo xtask verify theatre-auth` lanes.

Never selects a real Firebase project, reads cloud credentials, installs a tool or
uses an inherited database. Runtime sockets are confined by a fresh Linux network
namespace, in addition to the emulator's own outbound-socket guard.
"""

import argparse
import contextlib
import errno
import json
import os
from pathlib import Path
import re
import selectors
import signal
import socket
import subprocess
import sys
import tempfile
import time

# -I prevents inherited Python configuration; this is the repository-owned module path.
sys.path.insert(0, str(Path(__file__).resolve().parent))
from test_postgres import free_port

ROOT = Path(__file__).resolve().parents[1]
TEST = ("platform::theatre_registration::firebase_enrollment::emulator_tests::"
        "owned_auth_emulator_registration_http_and_postgres_fences")
SCENARIOS = ["emitted_action", "no_confirmation_or_polling", "wrong_operation_or_malformed",
             "confirmation_race_one_winner", "setup_is_not_session", "replay_unavailable",
             "preconsumed_action_unavailable", "action_bound_cancel"]
MAX_OUTPUT = 4 * 1024 * 1024
NAMESPACE = ["unshare", "--user", "--map-current-user", "--net", "--keep-caps",
             "--pid", "--fork", "--kill-child=KILL", "--mount-proc"]


class Unsupported(Exception):
    """A named local prerequisite is absent; no acceptance was obtained."""


def isolated_environment(inherited):
    """Keep tool locations, never provider, DB, proxy, loader or application inputs."""
    environment = {key: inherited[key] for key in ("PATH", "LANG", "LC_ALL", "TZ")
                   if key in inherited}
    original_home = Path(inherited.get("HOME", "/nonexistent"))
    for key, default in (("CARGO_HOME", original_home / ".cargo"),
                         ("RUSTUP_HOME", original_home / ".rustup")):
        environment[key] = inherited.get(key, str(default))
    environment.update(HOME="/nonexistent/cantos-no-user-home", RUSTUP_TOOLCHAIN="1.98.1",
                       RUSTUP_AUTO_INSTALL="0", CARGO_NET_OFFLINE="true", NO_COLOR="1")
    return environment


def captured(command, environment, timeout=15):
    """Raw child output stays transient and is never included in a failure message."""
    process = subprocess.Popen(command, cwd=ROOT, env=environment, stdin=subprocess.DEVNULL,
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                               start_new_session=True)
    buffers = {"stdout": bytearray(), "stderr": bytearray()}
    deadline = time.monotonic() + timeout

    def exited():
        # Retain the leader's PID until the last owned group signal. poll()/communicate()
        # would reap it and permit reuse while a descendant still owns a captured pipe.
        return os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None

    try:
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ, "stdout")
            selector.register(process.stderr, selectors.EVENT_READ, "stderr")
            while selector.get_map() or not exited():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise subprocess.TimeoutExpired(command[0], timeout)
                for key, _ in selector.select(min(remaining, 0.1)):
                    chunk = os.read(key.fd, 65536)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    buffers[key.data].extend(chunk)
                    if sum(map(len, buffers.values())) > MAX_OUTPUT:
                        raise RuntimeError("child output exceeded the private 4 MiB bound; output suppressed")
        # Cleanup signals every remaining owned descendant before the leader is reaped.
        os.killpg(process.pid, signal.SIGKILL)
        return subprocess.CompletedProcess(command, process.wait(), bytes(buffers["stdout"]),
                                           bytes(buffers["stderr"]))
    finally:
        if process.returncode is None:
            # The owned namespace launcher propagates TERM to its init process. Let its
            # finally block stop PostgreSQL before the bounded kill fallback. If the init
            # process cannot clean up, the kernel kills every PID-namespace descendant.
            os.killpg(process.pid, signal.SIGTERM)
            grace = time.monotonic() + 5
            while not exited() and time.monotonic() < grace:
                time.sleep(0.025)
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=3)
        process.stdout.close()
        process.stderr.close()


def checked(command, environment, timeout=15):
    completed = captured(command, environment, timeout)
    if completed.returncode:
        raise RuntimeError("owned fixture subprocess failed; raw output suppressed")
    return completed.stdout.decode("utf-8", errors="strict")


def emulator_module(inherited):
    selected = inherited.get("CANTOS_TEST_FIREBASE_AUTH_MODULE")
    if not selected or not Path(selected).is_absolute():
        raise Unsupported("set CANTOS_TEST_FIREBASE_AUTH_MODULE to an absolute installed "
                          "firebase-tools 14.2.2 lib/emulator/auth/server.js; nothing is installed")
    module = Path(selected).resolve()
    try:
        package = json.loads((module.parents[3] / "package.json").read_text())
    except (OSError, ValueError, IndexError):
        raise Unsupported("selected Auth module lacks its reviewed firebase-tools package metadata") from None
    if (not module.is_file() or tuple(module.parts[-4:]) != ("lib", "emulator", "auth", "server.js")
            or package.get("name") != "firebase-tools" or package.get("version") != "14.2.2"):
        raise Unsupported("selected Auth module must be firebase-tools 14.2.2; no global upgrade is attempted")
    return str(module), package["version"]


def prerequisites(suite, inherited, environment):
    if suite == "firebase-web-e2e":
        raise Unsupported("real disposable Kanidm credential-setup/OIDC host fixture is not implemented; "
                          "browser helper or mock Kanidm cannot satisfy full local Web acceptance")
    if sys.platform != "linux" or os.geteuid() == 0:
        raise Unsupported("this isolated lane requires non-root Linux with unprivileged user/network namespaces")
    module, firebase_version = emulator_module(inherited)
    versions = {"firebase_tools": firebase_version, "python": sys.version.split()[0]}
    for name, command in (("node", ["node", "--version"]), ("cargo", ["cargo", "--version"]),
                          ("initdb", ["initdb", "--version"]), ("pg_ctl", ["pg_ctl", "--version"]),
                          ("psql", ["psql", "--version"]), ("unshare", ["unshare", "--version"]),
                          ("ip", ["ip", "-Version"])):
        try:
            output = checked(command, environment)
        except (OSError, RuntimeError, subprocess.TimeoutExpired):
            raise Unsupported(f"provide installed {name} on PATH; this lane installs nothing") from None
        versions[name] = output.strip().splitlines()[0]
    if any(re.search(r"\(PostgreSQL\) 17(?:\.|$)", versions[name]) is None
           for name in ("initdb", "pg_ctl", "psql")):
        raise Unsupported("provide PostgreSQL 17 initdb, pg_ctl and psql together on PATH")
    node = re.fullmatch(r"v(\d+)\.\d+\.\d+", versions["node"])
    if node is None or int(node[1]) < 22:
        raise Unsupported("provide stable installed Node 22 or newer for the reviewed Auth emulator contract")
    probe = captured(NAMESPACE + ["ip", "link", "set", "lo", "up"], environment)
    if probe.returncode:
        raise Unsupported("enable unprivileged user/network/PID namespaces with owned loopback capability; "
                          "unshare probe refused, no fixture was created")
    return module, versions


def source_identity(environment):
    try:
        sha = checked(["git", "rev-parse", "HEAD"], environment).strip()
    except OSError:
        raise Unsupported("provide installed Git on PATH to identify the fixture source revision") from None
    if re.fullmatch(r"[0-9a-f]{40}", sha) is None:
        raise RuntimeError("could not identify fixture source revision")
    dirty = bool(checked(["git", "status", "--porcelain", "--untracked-files=normal"], environment))
    return {"sha": sha, "dirty": dirty}


def assert_network_guard(environment):
    """Verify the actual child namespace and negative destinations before starting fixtures."""
    if os.getpid() != 1 or not environment.get("CANTOS_TEST_PARENT_NETNS"):
        raise RuntimeError("fixture must be the init process of its owned PID namespace")
    if os.readlink("/proc/self/ns/net") == environment.get("CANTOS_TEST_PARENT_NETNS"):
        raise RuntimeError("fixture did not enter its owned network namespace")
    if socket.if_nameindex() != [(1, "lo")]:
        raise RuntimeError("fixture network namespace has an unexpected interface")
    checked(["ip", "link", "set", "lo", "up"], environment)
    # No request bytes, DNS, route, proxy or external network is involved. Check both
    # documentation ranges and an ordinary public address to detect an escaped namespace.
    for destination in ("192.0.2.1", "203.0.113.1", "1.1.1.1"):
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as probe:
            probe.settimeout(0.2)
            result = probe.connect_ex((destination, 443))
        if result not in (errno.ENETUNREACH, errno.EHOSTUNREACH):
            raise RuntimeError("forbidden destination was not refused by the kernel namespace")


def passing_test_count(output):
    found = re.findall(r"test result: ok\. ([0-9]+) passed; ([0-9]+) failed;", output)
    if len(found) != 1 or found[0] != ("1", "0"):
        raise RuntimeError("exact acceptance test did not report one passing test; output suppressed")
    return 1


def interrupted(_signal, _frame):
    raise InterruptedError("local fixture interrupted")


def run_isolated(environment):
    assert_network_guard(environment)
    for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(number, interrupted)
    # Only this run's directory can be cleaned. It contains no retained test inbox or token log.
    # The outside launcher owns deletion, including when namespace init is killed.
    temporary = environment.get("CANTOS_TEST_RUN_DIRECTORY")
    if not temporary:
        raise RuntimeError("fixture requires its launcher's owned private directory")
    with contextlib.nullcontext(temporary) as temporary:
        directory = Path(temporary)
        directory.chmod(0o700)
        data = directory / "pgdata"
        owned_env = dict(environment, HOME=str(directory / "home"),
                         XDG_CONFIG_HOME=str(directory / "config"), TMPDIR=temporary)
        port = free_port()
        url = f"postgresql://cantos_test_admin@127.0.0.1:{port}/postgres"
        owned_env["CANTOS_TEST_CLUSTER_URL"] = url
        started = False
        try:
            checked(["initdb", "-D", str(data), "--username=cantos_test_admin", "--auth=trust",
                     "--no-locale", "--encoding=UTF8"], owned_env, 60)
            # Same durable native disposable-cluster settings as test_postgres.py, with
            # this run's private socket directory so simultaneous runs share no socket.
            options = (f"-h 127.0.0.1 -p {port} -k {directory} "
                       "-c max_wal_size=64MB -c min_wal_size=32MB -c checkpoint_timeout=30s "
                       "-c fsync=on -c synchronous_commit=on -c full_page_writes=on")
            checked(["pg_ctl", "-D", str(data), "-l", str(directory / "postgres.log"),
                     "-o", options, "-w", "-t", "30", "start"], owned_env, 45)
            started = True
            checked(["psql", "-X", url, "-v", "ON_ERROR_STOP=1", "-c",
                     "CREATE ROLE cantos_app LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE;"], owned_env)
            # Exact named cfg(test) fixture, not a test executable/URL supplied by the caller.
            output = checked(["cargo", "test", "-p", "cantos-server", "--lib", "--locked", "--offline",
                              TEST, "--", "--ignored", "--exact", "--test-threads=1"], owned_env, 1200)
            return passing_test_count(output)
        finally:
            if started:
                # A second signal cannot skip cleanup. Never kill by process name/port.
                for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
                    signal.signal(number, signal.SIG_IGN)
                checked(["pg_ctl", "-D", str(data), "-m", "immediate", "-w", "-t", "15", "stop"],
                        owned_env, 25)


def envelope(suite):
    return {"schema_version": 1, "suite": suite, "status": "FAIL", "reason": "",
            "source": None, "versions": {}, "topology": {}, "scenarios": [], "tests_passed": 0,
            "layers": {"firebase_enrollment_integration": "NOT_RUN", "full_local_web_e2e": "NOT_RUN"}}


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--suite", required=True, choices=("firebase-emulator", "firebase-web-e2e"))
    parser.add_argument("--isolated", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    inherited = dict(os.environ)
    environment = isolated_environment(inherited)
    result = envelope(args.suite)
    code = 1
    try:
        if sys.platform != "linux" or not hasattr(os, "waitid") or not hasattr(os, "WNOWAIT"):
            raise Unsupported("this isolated lane requires Linux waitid and unprivileged user/network/PID namespaces")
        for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(number, interrupted)
        if not __debug__:
            raise RuntimeError("fixture assertions require Python without optimization")
        if args.isolated:
            # This branch only accepts the explicit module and parent namespace marker
            # from its owned launcher; it does not read any provider or DB environment.
            environment["CANTOS_TEST_FIREBASE_AUTH_MODULE"] = emulator_module(inherited)[0]
            environment["CANTOS_TEST_PARENT_NETNS"] = inherited.get("CANTOS_TEST_PARENT_NETNS", "")
            environment["CANTOS_TEST_RUN_DIRECTORY"] = inherited.get("CANTOS_TEST_RUN_DIRECTORY", "")
            count = run_isolated(environment)
            result.update(status="PASS", reason="actual HTTP + PostgreSQL + owned Auth Emulator; Kanidm mocked",
                          scenarios=SCENARIOS, tests_passed=count,
                          topology={"runtime_network": "owned loopback-only Linux namespace",
                                    "firebase": "owned Auth Emulator", "kanidm": "recording mock",
                                    "api": "real TCP HTTP handlers/application", "postgres": "owned disposable cluster"},
                          layers={"firebase_enrollment_integration": "PASS", "full_local_web_e2e": "NOT_RUN"})
            code = 0
        else:
            result["source"] = source_identity(environment)
            module, versions = prerequisites(args.suite, inherited, environment)
            result["versions"] = versions
            with tempfile.TemporaryDirectory(prefix="cantos-theatre-auth-", dir="/tmp") as temporary:
                Path(temporary).chmod(0o700)
                owned_env = dict(environment, CANTOS_TEST_FIREBASE_AUTH_MODULE=module,
                                 CANTOS_TEST_PARENT_NETNS=os.readlink("/proc/self/ns/net"),
                                 CANTOS_TEST_RUN_DIRECTORY=temporary)
                child = captured(NAMESPACE + [sys.executable, "-I", str(Path(__file__).resolve()),
                                  "--suite", args.suite, "--isolated"], owned_env, 1500)
            summary = json.loads(child.stdout)
            if (summary.get("suite") != args.suite or summary.get("schema_version") != 1
                    or summary.get("status") != "PASS" or child.returncode != 0
                    or summary.get("tests_passed") != 1 or summary.get("scenarios") != SCENARIOS):
                raise RuntimeError("isolated fixture failed; raw output suppressed")
            summary.update(source=result["source"], versions=versions)
            result = summary
            code = 0
    except Unsupported as error:
        result.update(status="UNSUPPORTED", reason=str(error))
        result["layers"]["full_local_web_e2e" if args.suite == "firebase-web-e2e"
                         else "firebase_enrollment_integration"] = "UNSUPPORTED"
        code = 3
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError, InterruptedError, KeyboardInterrupt):
        result.update(status="FAIL", reason="owned fixture failed, timed out or was interrupted; raw output suppressed")
        result["layers"]["firebase_enrollment_integration"] = "FAIL"
    print(json.dumps(result, separators=(",", ":")))
    return code


if __name__ == "__main__":
    sys.exit(main())
