#!/usr/bin/env python3
"""Primitive for the opt-in `cargo xtask verify native-isolation` guard.

A prebuilt, named Rust libtest runs with no inherited credentials or PATH tools.
Linux uses fresh user/network/PID namespaces; macOS reports best-effort limitations.
An optional prebuilt Identity child executable is passed by absolute path through exactly one
test-only key and bound by digest before and after the run. This guards a selected test, never
establishes that it exercised production binaries or how they were built.
"""

import argparse
import errno
import hashlib
import json
import os
from pathlib import Path
import re
import select
import selectors
import shutil
import signal
import socket
import stat
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
MAX_OUTPUT = 1024 * 1024
FORBIDDEN = re.compile(r"^(?:podman|docker|psql|postgres|kanidm.*|firebase|redis.*|feldera)(?:[-.]?[0-9].*)?$", re.I)
NAMESPACE = ["--user", "--map-current-user", "--net", "--keep-caps", "--pid",
             "--fork", "--kill-child=KILL", "--mount-proc"]
ENVIRONMENT_KEYS = frozenset({"PATH", "HOME", "TMPDIR", "TMP", "TEMP", "XDG_CONFIG_HOME",
                              "XDG_CACHE_HOME", "XDG_DATA_HOME", "LANG", "LC_ALL", "TZ",
                              "NO_COLOR", "CANTOS_NATIVE_TEST_DIRECTORY"})
# The one optional, test-only input: the absolute path of the supplied Identity executable.
IDENTITY_KEY = "CANTOS_TEST_IDENTITY_BINARY"
SCHEMA_VERSION = 2


class GuardFailure(Exception):
    """Fixed, public diagnostic; child output is never attached."""


class Unsupported(Exception):
    """An absent host capability, without installation or fallback."""


class OwnedTemporaryDirectory(tempfile.TemporaryDirectory):
    """Defer cancellation before cleanup detaches the finalizer, through directory removal."""

    def cleanup(self):
        previous_mask = signal.pthread_sigmask(
            signal.SIG_BLOCK, {signal.SIGINT, signal.SIGTERM, signal.SIGHUP})
        try:
            super().cleanup()
        finally:
            # A pending cancellation may raise here, after the owned directory is gone.
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)


def sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


class ExitObserver:
    """Observe exit without reaping: retain the leader PID until final group cleanup."""

    def __init__(self, pid):
        self.pid = pid
        self.queue = None
        self.done = False
        if sys.platform == "darwin":
            self.queue = select.kqueue()
            event = select.kevent(pid, filter=select.KQ_FILTER_PROC,
                                 flags=select.KQ_EV_ADD | select.KQ_EV_ONESHOT,
                                 fflags=select.KQ_NOTE_EXIT)
            try:
                self.queue.control([event], 0, 0)
            except ProcessLookupError:
                # An exited, unreaped child still reserves its PID.
                self.done = True
        elif not all(hasattr(os, name) for name in ("waitid", "WNOWAIT")):
            raise Unsupported("host lacks non-reaping child exit observation")

    def exited(self):
        if self.queue is not None:
            self.done = self.done or bool(self.queue.control(None, 1, 0))
            return self.done
        return os.waitid(os.P_PID, self.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is not None

    def close(self):
        if self.queue is not None:
            self.queue.close()


def signal_group(pid, number):
    try:
        os.killpg(pid, number)
    except ProcessLookupError:
        pass
    except PermissionError:
        # Darwin reports EPERM (rather than ESRCH) for a process group whose
        # sole remaining member is its unreaped zombie leader. The reserved PID
        # cannot be reused. A live leader or a foreign/sandbox-denied group is
        # still a real cleanup failure and must not be silently ignored.
        if sys.platform != "darwin":
            raise
        try:
            os.getpgid(pid)
        except ProcessLookupError:
            return
        raise


def captured(command, environment, directory, timeout, max_output=MAX_OUTPUT):
    """Bound output/time and clean only the group created here, before reaping."""
    process = None
    observer = None
    output = {"stdout": bytearray(), "stderr": bytearray()}
    try:
        process = subprocess.Popen(command, cwd=directory, env=environment,
                                   stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE, start_new_session=True)
        observer = ExitObserver(process.pid)
        deadline = time.monotonic() + timeout
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ, "stdout")
            selector.register(process.stderr, selectors.EVENT_READ, "stderr")
            while selector.get_map() or not observer.exited():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise GuardFailure("owned subprocess exceeded its time bound")
                for key, _ in selector.select(min(remaining, 0.05)):
                    block = os.read(key.fd, 65536)
                    if not block:
                        selector.unregister(key.fileobj)
                    else:
                        output[key.data].extend(block)
                        if sum(map(len, output.values())) > max_output:
                            raise GuardFailure("owned subprocess exceeded its output bound")
                # A descendant may retain output after the leader exits. Stop it now,
                # while the unreaped leader still reserves the process-group identity.
                if observer.exited():
                    signal_group(process.pid, signal.SIGKILL)
        signal_group(process.pid, signal.SIGKILL)
        code = process.wait(timeout=3)
        return subprocess.CompletedProcess(command, code, bytes(output["stdout"]),
                                           bytes(output["stderr"]))
    except OSError:
        raise GuardFailure("owned subprocess could not start") from None
    finally:
        # A repeated cancellation must not interrupt kill/reap or descriptor cleanup.
        # Keep handlers unchanged and defer these signals until all owned resources
        # are released; restoring the old mask may then raise the pending cancellation.
        cancellation = {signal.SIGINT, signal.SIGTERM, signal.SIGHUP}
        previous_mask = signal.pthread_sigmask(signal.SIG_BLOCK, cancellation)
        try:
            try:
                if process is not None and process.returncode is None:
                    signal_group(process.pid, signal.SIGKILL)
                    process.wait(timeout=3)
            finally:
                try:
                    if observer is not None:
                        observer.close()
                finally:
                    if process is not None:
                        try:
                            process.stdout.close()
                        finally:
                            process.stderr.close()
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous_mask)


def private_environment(directory, identity=None):
    paths = {name: directory / name for name in ("path", "home", "tmp", "config", "cache", "data")}
    for path in paths.values():
        path.mkdir(mode=0o700)
    environment = {"PATH": str(paths["path"]), "HOME": str(paths["home"]),
                   "TMPDIR": str(paths["tmp"]), "TMP": str(paths["tmp"]), "TEMP": str(paths["tmp"]),
                   "XDG_CONFIG_HOME": str(paths["config"]), "XDG_CACHE_HOME": str(paths["cache"]),
                   "XDG_DATA_HOME": str(paths["data"]), "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
                   "NO_COLOR": "1", "CANTOS_NATIVE_TEST_DIRECTORY": str(directory)}
    if identity is not None:
        environment[IDENTITY_KEY] = str(identity)
    return environment


def audit_environment(environment, identity=None):
    """Exactly the allowlist, plus the Identity key only when that exact child was supplied."""
    expected = ENVIRONMENT_KEYS | ({IDENTITY_KEY} if identity is not None else set())
    if set(environment) != expected:
        raise GuardFailure("credential or unapproved environment input detected")
    if identity is not None and environment[IDENTITY_KEY] != str(identity):
        raise GuardFailure("credential or unapproved environment input detected")


def audit_path(environment):
    """Inspect PATH entries including versioned/wildcard tool names; never execute found tools."""
    for item in environment["PATH"].split(os.pathsep):
        directory = Path(item)
        if not item or not directory.is_dir():
            raise GuardFailure("test PATH is not a private tool-free directory")
        for entry in directory.iterdir():
            if FORBIDDEN.fullmatch(entry.name) and os.access(entry, os.X_OK):
                raise GuardFailure("forbidden infrastructure executable detected")
        if any(directory.iterdir()):
            raise GuardFailure("test PATH contains an unexpected executable or file")


def negative_controls(environment, directory, identity=None):
    """Introduce real faults and require the same pre-test audits to reject them."""
    audit_environment(environment, identity)
    audit_path(environment)
    canary = Path(environment["PATH"]) / "podman"
    canary.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
    canary.chmod(0o700)
    try:
        try:
            audit_path(environment)
        except GuardFailure:
            pass
        else:
            raise GuardFailure("executable negative control was not rejected")
    finally:
        canary.unlink()
    for tool in ("podman", "docker", "psql", "postgres", "kanidmd", "firebase", "redis-server", "feldera"):
        try:
            completed = captured([tool, "--version"], environment, directory, 2)
        except GuardFailure as error:
            if str(error) != "owned subprocess could not start":
                raise
        else:
            del completed
            raise GuardFailure("forbidden infrastructure executable could run")
    poisoned = dict(environment, AWS_SECRET_ACCESS_KEY="synthetic-negative-control")
    try:
        audit_environment(poisoned, identity)
    except GuardFailure:
        pass
    else:
        raise GuardFailure("credential negative control was not rejected")
    audit_path(environment)
    return {"executable": "PASS", "credential": "PASS", "network": "NOT_RUN"}


def assert_unreachable(destination):
    family = socket.AF_INET6 if ":" in destination[0] else socket.AF_INET
    with socket.socket(family, socket.SOCK_STREAM) as probe:
        probe.settimeout(0.2)
        result = probe.connect_ex(destination)
    if result not in (errno.ENETUNREACH, errno.EHOSTUNREACH):
        raise GuardFailure("network guard allowed a reachable or unclassified destination")


def network_controls(parent_namespace, ip, environment, directory):
    if os.getpid() != 1 or os.readlink("/proc/self/ns/net") == parent_namespace:
        raise GuardFailure("fixture did not enter owned network and PID namespaces")
    if socket.if_nameindex() != [(1, "lo")]:
        raise GuardFailure("owned network namespace contains an unexpected interface")
    result = captured([ip, "link", "set", "lo", "up"], environment, directory, 3)
    if result.returncode:
        raise GuardFailure("owned loopback interface could not be enabled")
    for destination in (("192.0.2.1", 443), ("203.0.113.1", 443), ("1.1.1.1", 443), ("2001:db8::1", 443)):
        assert_unreachable(destination)
    # A real connectable socket must be classified as a guard failure. No request bytes
    # or DNS escape the namespace; the listener and selected port belong to this control.
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.bind(("127.0.0.1", 0))
        listener.listen(1)
        try:
            assert_unreachable(listener.getsockname())
        except GuardFailure:
            pass
        else:
            raise GuardFailure("network negative control was not rejected")
    return "PASS"


def passing_test_count(output, name):
    try:
        text = output.decode("utf-8", errors="strict")
    except UnicodeError:
        raise GuardFailure("selected test returned invalid output") from None
    summaries = re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out; finished in .+$", text, re.M)
    named = re.findall(r"^test (.+) \.\.\. (.+)$", text, re.M)
    if len(summaries) != 1 or summaries[0][:4] != ("1", "0", "0", "0") or named != [(name, "ok")]:
        raise GuardFailure("exact selected test did not report one passing test")
    return 1


def regular_executable(value, label):
    path = Path(value)
    message = label + " must be an absolute regular executable file"
    if not path.is_absolute() or not path.is_file() or path.is_symlink() or not os.access(path, os.X_OK):
        raise GuardFailure(message)
    if not stat.S_ISREG(path.stat().st_mode):
        raise GuardFailure(message)
    return path


def test_binary(value):
    return regular_executable(value, "test binary")


def identity_binary(value):
    """The supplied child executable, distinct from the test executable it is run by."""
    if value is None:
        return None
    return regular_executable(value, "identity binary")


def framed_hash(digest, value):
    """Bind arbitrary path/content bytes without ambiguous concatenation."""
    digest.update(len(value).to_bytes(8, "big"))
    digest.update(value)


def stable_stat(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size, value.st_mtime_ns,
            value.st_ctime_ns)


def raw_source_file(path):
    """Snapshot raw bytes/mode without Git conversions or following symlinks."""
    try:
        before = path.lstat()
    except FileNotFoundError:
        return b"missing", b"", None
    if stat.S_ISLNK(before.st_mode):
        content = os.readlink(os.fsencode(path))
        raw_digest = hashlib.sha256(content).digest()
        object_id = hashlib.sha1(b"blob " + str(len(content)).encode() + b"\0" + content).hexdigest()
        git_mode = b"120000"
    elif stat.S_ISREG(before.st_mode):
        # O_NOFOLLOW closes the lstat/open symlink race. O_NONBLOCK avoids blocking
        # if a regular source file is concurrently replaced with a FIFO/device.
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(descriptor, "rb") as source:
            opened = os.fstat(source.fileno())
            if stable_stat(opened) != stable_stat(before):
                raise GuardFailure("source input changed while snapshotting")
            raw = hashlib.sha256()
            blob = hashlib.sha1(b"blob " + str(before.st_size).encode() + b"\0")
            for block in iter(lambda: source.read(1024 * 1024), b""):
                raw.update(block)
                blob.update(block)
            if stable_stat(os.fstat(source.fileno())) != stable_stat(before):
                raise GuardFailure("source input changed while snapshotting")
        raw_digest = raw.digest()
        object_id = blob.hexdigest()
        git_mode = b"100755" if before.st_mode & stat.S_IXUSR else b"100644"
    else:
        raise GuardFailure("source snapshot requires regular files or symlinks")
    if stable_stat(path.lstat()) != stable_stat(before):
        raise GuardFailure("source input changed while snapshotting")
    return str(before.st_mode).encode(), raw_digest, (git_mode, object_id.encode())


def source_identity(directory):
    git = shutil.which("git")
    if not git:
        raise Unsupported("installed Git is required for source attribution")
    environment = {"PATH": "/usr/bin:/bin", "HOME": str(directory), "LANG": "C", "LC_ALL": "C",
                   "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": os.devnull,
                   "GIT_OPTIONAL_LOCKS": "0"}
    git_command = [git, "--no-pager", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false"]

    def plumbing(arguments):
        completed = captured(git_command + arguments, environment, ROOT, 5, 8 * 1024 * 1024)
        if completed.returncode:
            raise GuardFailure("source identity could not be determined")
        return completed.stdout

    sha = plumbing(["rev-parse", "HEAD"]).decode("ascii").strip()
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise GuardFailure("source identity could not be determined")
    # These plumbing commands enumerate metadata/paths only. Working-tree status,
    # diff and hash-object conversion commands can invoke configured clean filters.
    index = plumbing(["ls-files", "--stage", "-z"])
    tree = plumbing(["ls-tree", "-r", "-z", "HEAD"])
    untracked = plumbing(["ls-files", "--others", "--exclude-standard", "-z"])
    index_entries = {}
    head_entries = {}
    conflicted = False
    paths = set()
    for row in index.split(b"\0"):
        if row:
            metadata, relative = row.split(b"\t", 1)
            mode, object_id, stage = metadata.split()
            paths.add(relative)
            if stage != b"0":
                conflicted = True
            else:
                index_entries[relative] = (mode, object_id)
    for row in tree.split(b"\0"):
        if row:
            metadata, relative = row.split(b"\t", 1)
            mode, kind, object_id = metadata.split()
            if kind != b"blob":
                raise GuardFailure("source snapshot does not support tracked submodules")
            paths.add(relative)
            head_entries[relative] = (mode, object_id)
    extra_paths = {relative for relative in untracked.split(b"\0") if relative}
    paths.update(extra_paths)
    dirty = conflicted or bool(extra_paths) or index_entries != head_entries
    snapshot = hashlib.sha256(b"cantos-native-raw-source-v1\0")
    for metadata in (sha.encode(), index, tree):
        framed_hash(snapshot, metadata)
    for relative in sorted(paths):
        relative_path = Path(os.fsdecode(relative))
        if relative_path.is_absolute() or ".." in relative_path.parts:
            raise GuardFailure("source input path escaped the repository")
        mode, content_hash, actual = raw_source_file(ROOT / relative_path)
        framed_hash(snapshot, relative)
        framed_hash(snapshot, mode)
        framed_hash(snapshot, content_hash)
        dirty = dirty or actual != index_entries.get(relative)
    return {"sha": sha, "dirty": dirty, "diff_sha256": snapshot.hexdigest(),
            "guard_sha256": sha256(Path(__file__))}


def envelope():
    return {"schema_version": SCHEMA_VERSION, "status": "FAIL", "reason": "guard did not complete",
            "source": None, "binary_sha256": None, "identity_binary_sha256": None, "tests_passed": 0,
            "test_name": None, "timeout_seconds": None,
            "controls": {"executable": "NOT_RUN", "credential": "NOT_RUN", "network": "NOT_RUN"},
            "network_mode": "none", "limitations": [
                "PATH removal is not an exec sandbox: absolute executable paths remain accessible",
                "private HOME and environment removal do not sandbox filesystem reads",
                "binary digest does not establish a source-to-build provenance chain",
                "identity child digest proves only which file was supplied and that it stayed unchanged",
                "guard evidence does not establish production binaries, auth E2E, database or Kani acceptance"]}


def run_target(args, directory, result, parent_namespace=None, ip=None):
    child = identity_binary(args.identity_binary)
    environment = private_environment(directory, child)
    result["controls"] = negative_controls(environment, directory, child)
    if parent_namespace is not None:
        result["network_mode"] = "linux-network-namespace"
        result["controls"]["network"] = network_controls(parent_namespace, ip, environment, directory)
    else:
        result["network_mode"] = "macos-best-effort"
        result["controls"]["network"] = "UNSUPPORTED"
        result["limitations"].append("macOS does not enforce egress denial; kernel network negative control was not run")
    binary = test_binary(args.test_binary)
    if child is not None and child.resolve() == binary.resolve():
        raise GuardFailure("identity binary must differ from the test binary")
    before = sha256(binary)
    result["binary_sha256"] = before
    child_before = sha256(child) if child is not None else None
    result["identity_binary_sha256"] = child_before
    completed = captured([str(binary), "--exact", args.test, "--test-threads=1"], environment, directory, args.timeout_seconds)
    if sha256(binary) != before:
        raise GuardFailure("selected test binary changed during execution")
    if child is not None and sha256(child) != child_before:
        raise GuardFailure("identity binary changed during execution")
    if completed.returncode:
        raise GuardFailure("selected test exited unsuccessfully; raw output suppressed")
    result["tests_passed"] = passing_test_count(completed.stdout, args.test)
    audit_environment(environment, child)
    audit_path(environment)
    result["status"] = "PASS" if parent_namespace is not None else "UNSUPPORTED"
    result["reason"] = ("all isolation controls and the exact selected test passed" if parent_namespace is not None
                        else "selected test and tool/credential controls passed; macOS egress isolation is unsupported")


def interrupted(_number, _frame):
    raise GuardFailure("owned guard invocation was interrupted")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", required=True)
    parser.add_argument("--test", required=True)
    parser.add_argument("--timeout-seconds", type=int, default=60, choices=range(1, 121), metavar="1..120")
    parser.add_argument("--identity-binary")
    parser.add_argument("--isolated-directory", help=argparse.SUPPRESS)
    parser.add_argument("--parent-netns", help=argparse.SUPPRESS)
    parser.add_argument("--ip", help=argparse.SUPPRESS)
    args = parser.parse_args(argv)
    result = envelope()
    result["timeout_seconds"] = args.timeout_seconds
    try:
        if not args.test or len(args.test) > 512 or not re.fullmatch(r"[A-Za-z0-9_:]+", args.test):
            raise GuardFailure("selected test name must be a bounded Rust test path")
        result["test_name"] = args.test
        test_binary(args.test_binary)
        identity_binary(args.identity_binary)
        if sys.platform not in ("linux", "darwin"):
            raise Unsupported("native isolation is implemented only for Linux and macOS")
        for number in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
            signal.signal(number, interrupted)
        if args.isolated_directory:
            if sys.platform != "linux" or not args.parent_netns or not args.ip:
                raise GuardFailure("invalid owned namespace invocation")
            run_target(args, Path(args.isolated_directory), result, args.parent_netns, args.ip)
        else:
            # Ignore ambient TMPDIR and avoid inherited credential or application locations.
            with OwnedTemporaryDirectory(prefix="cantos-native-isolation-", dir="/tmp") as temporary:
                directory = Path(temporary)
                directory.chmod(0o700)
                result["source"] = source_identity(directory)
                before_source = result["source"]
                if sys.platform == "darwin":
                    run_target(args, directory, result)
                else:
                    unshare = shutil.which("unshare")
                    ip = shutil.which("ip")
                    if not unshare or not ip:
                        raise Unsupported("installed unshare and ip with user/network/PID namespace support are required")
                    parent_namespace = os.readlink("/proc/self/ns/net")
                    command = [unshare] + NAMESPACE + [sys.executable, "-I", str(Path(__file__).resolve()),
                               "--test-binary", args.test_binary, "--test", args.test,
                               "--timeout-seconds", str(args.timeout_seconds),
                               "--isolated-directory", str(directory), "--parent-netns", parent_namespace, "--ip", ip]
                    if args.identity_binary is not None:
                        command += ["--identity-binary", args.identity_binary]
                    clean = {"PATH": "/usr/bin:/bin", "LANG": "C", "LC_ALL": "C", "HOME": str(directory)}
                    # Probe permission separately: a rejected namespace is UNSUPPORTED,
                    # a failure after successful provisioning is a genuine guard FAIL.
                    probe = captured([unshare] + NAMESPACE + [ip, "link", "set", "lo", "up"], clean, directory, 5)
                    if probe.returncode:
                        raise Unsupported("host refused owned user/network/PID namespace provisioning")
                    child = captured(command, clean, directory, args.timeout_seconds + 20)
                    try:
                        isolated = json.loads(child.stdout)
                    except (ValueError, UnicodeError):
                        raise GuardFailure("owned namespace returned an invalid guard receipt") from None
                    if isolated.get("schema_version") != SCHEMA_VERSION or isolated.get("status") not in ("PASS", "FAIL"):
                        raise GuardFailure("owned namespace returned an invalid guard receipt")
                    if child.returncode != (0 if isolated["status"] == "PASS" else 1):
                        raise GuardFailure("owned namespace returned an inconsistent guard receipt")
                    isolated["source"] = result["source"]
                    result = isolated
                if source_identity(directory) != before_source:
                    raise GuardFailure("source inputs changed during guard execution")
    except Unsupported as error:
        result.update(status="UNSUPPORTED", reason=str(error))
    except (GuardFailure, OSError, ValueError, subprocess.SubprocessError) as error:
        result.update(status="FAIL", reason=str(error) if isinstance(error, GuardFailure)
                      else "guard could not complete; raw diagnostics suppressed")
    print(json.dumps(result, sort_keys=True))
    return {"PASS": 0, "FAIL": 1, "UNSUPPORTED": 3}[result["status"]]


if __name__ == "__main__":
    sys.exit(main())
