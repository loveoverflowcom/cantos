#!/usr/bin/env python3
"""Android Studio preview capture primitive; invoke through cargo xtask compose-preview.

No emulator, installation, IDE launch, Gradle bootstrap or app runtime is started.
Exit 3 means unavailable prerequisites; exit 2 means an invalid request.
"""

import argparse
import contextlib
import hashlib
import json
import os
import re
import selectors
import shutil
import signal
import stat
import struct
import subprocess
import sys
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MOBILE_DIRS = {
    "cantos": "apps/cantos/mobile",
    "cantos-studio": "apps/cantos-studio/mobile",
}
UNSUPPORTED = 3
MAX_COMMAND_LOG_BYTES = 4 * 1024 * 1024
MAX_PNG_BYTES = 32 * 1024 * 1024
MAX_SOURCE_BYTES = 4 * 1024 * 1024


class PreviewError(Exception):
    def __init__(self, message, code=1):
        super().__init__(message)
        self.code = code


class ProcessCleanupError(PreviewError):
    """An owned group was not proven stopped; this can never become missing provenance."""


class CaptureInterrupted(KeyboardInterrupt):
    def __init__(self, signum):
        super().__init__(f"capture interrupted by {signal.Signals(signum).name}")
        self.signum = signum


@contextlib.contextmanager
def capture_signals():
    previous = {}
    def interrupt(signum, _frame):
        raise CaptureInterrupted(signum)
    try:
        if os.name == "posix":
            for signum in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
                previous[signum] = signal.signal(signum, interrupt)
        yield
    finally:
        for signum, handler in previous.items():
            signal.signal(signum, handler)


def bounded_integer(minimum, maximum):
    def parse(value):
        try:
            number = int(value)
        except ValueError as exc:
            raise argparse.ArgumentTypeError("must be an integer") from exc
        if not minimum <= number <= maximum:
            raise argparse.ArgumentTypeError(f"must be between {minimum} and {maximum}")
        return number
    return parse


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--app", choices=MOBILE_DIRS, required=True)
    result.add_argument("--file", type=Path, required=True)
    result.add_argument("--composable", required=True)
    result.add_argument("--label", help="Scenario/profile label; preview annotations set display values")
    result.add_argument("--pid", type=bounded_integer(1, 2**32 - 1))
    result.add_argument("--timeout-seconds", type=bounded_integer(1, 300), default=60)
    result.add_argument("--android-cli", default="android", help="Official Android CLI executable")
    result.add_argument("--dry-run", action="store_true")
    return result


def request_paths(args, root):
    expected_mobile = root.resolve() / MOBILE_DIRS[args.app]
    mobile = expected_mobile.resolve()
    if mobile != expected_mobile:
        raise PreviewError("selected mobile project must not contain symlinks", 2)
    if not any((mobile / name).is_file() for name in ("settings.gradle.kts", "settings.gradle")):
        raise PreviewError(f"no Gradle project in {MOBILE_DIRS[args.app]}", UNSUPPORTED)
    source = (root / args.file).resolve()
    if not source.is_relative_to(mobile) or source.suffix != ".kt" or not source.is_file():
        raise PreviewError("--file must be an existing Kotlin file inside the selected mobile app", 2)
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", args.composable):
        raise PreviewError("--composable must be a simple Kotlin function name", 2)
    return mobile, source


def render_command(args, mobile, source, image):
    command = [
        args.android_cli, "studio", "render-compose-preview",
        f"--project={mobile}", f"--output-image-file={image}", "--print-semantics",
    ]
    if args.pid is not None:
        command.append(f"--pid={args.pid}")
    return command + [str(source), args.composable]


def semantics_json(stdout):
    """Keep the CLI's JSON unchanged; tolerate surrounding human-readable log lines.

    Accept one object/array, reject ambiguous output rather than choosing an invented schema.
    """
    decoder = json.JSONDecoder()
    values = []
    consumed = 0
    offset = 0
    for line in stdout.splitlines(keepends=True):
        start = offset + len(line) - len(line.lstrip())
        if start >= consumed and line.lstrip().startswith(("{", "[")):
            try:
                value, length = decoder.raw_decode(stdout[start:])
            except ValueError:
                pass
            else:
                if isinstance(value, (dict, list)):
                    values.append(value)
                    consumed = start + length
                    remainder = stdout[consumed:].split("\n", 1)[0].lstrip()
                    if remainder.startswith(("{", "[")):
                        raise PreviewError("multiple semantics JSON values on one line; inspect render.stdout.log")
        offset += len(line)
    if len(values) != 1:
        raise PreviewError("expected one semantics JSON object/array; inspect render.stdout.log")
    return values[0]


def png_header_dimensions(data):
    """Check the PNG header, not visual quality or full pixel decoding."""
    if len(data) < 45 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[8:16] != b"\x00\x00\x00\rIHDR":
        raise PreviewError("renderer did not produce a PNG with an IHDR header")
    width, height = struct.unpack(">II", data[16:24])
    if not width or not height:
        raise PreviewError("renderer produced a zero-size PNG")
    return {"width_px": width, "height_px": height}


def open_regular_file(path, limit):
    """Reject special files before reading; no-follow also closes the final-component race."""
    try:
        descriptor = os.open(path, os.O_RDONLY | os.O_NONBLOCK | os.O_NOFOLLOW)
    except OSError as exc:
        raise PreviewError(f"{path.name} must be a regular file without symlinks: {exc}") from exc
    try:
        facts = os.fstat(descriptor)
        if not stat.S_ISREG(facts.st_mode) or facts.st_nlink != 1:
            raise PreviewError(f"{path.name} must be a regular file with one link")
        if facts.st_size > limit:
            raise PreviewError(f"{path.name} exceeds the {limit} byte limit")
        return os.fdopen(descriptor, "rb")
    except BaseException:
        os.close(descriptor)
        raise


def stream_sha256(stream, limit):
    digest = hashlib.sha256()
    total = 0
    while chunk := stream.read(min(64 * 1024, limit - total + 1)):
        total += len(chunk)
        if total > limit:
            raise PreviewError(f"file grew beyond the {limit} byte limit while reading")
        digest.update(chunk)
    return digest.hexdigest()


def sha256(path, limit=None):
    limit = MAX_PNG_BYTES if limit is None else limit
    with open_regular_file(path, limit) as stream:
        return stream_sha256(stream, limit)


def png_evidence(image):
    with open_regular_file(image, MAX_PNG_BYTES) as stream:
        os.fchmod(stream.fileno(), 0o600)
        dimensions = png_header_dimensions(stream.read(45))
        stream.seek(0)
        return dimensions, stream_sha256(stream, MAX_PNG_BYTES)


def png_dimensions(image):
    return png_evidence(image)[0]


def open_artifact(path):
    """Never overwrite a renderer-created link/file, even inside a fresh bundle."""
    try:
        descriptor = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    except OSError as exc:
        raise PreviewError(f"could not create private artifact {path.name}: {exc}") from exc
    return os.fdopen(descriptor, "wb")


def write_artifact(path, text):
    data = text.encode("utf-8")
    if len(data) > MAX_COMMAND_LOG_BYTES:
        raise PreviewError(f"{path.name} exceeds the {MAX_COMMAND_LOG_BYTES} byte limit")
    with open_artifact(path) as stream:
        stream.write(data)


def git_provenance(root, bundle):
    def output(*args):
        try:
            return execute(["git", *args], f"git-{args[0]}", root, bundle, 10).strip()
        except ProcessCleanupError:
            raise
        except PreviewError:
            return None
    status = output("status", "--porcelain")
    return {"head": output("rev-parse", "HEAD"), "dirty": None if status is None else bool(status)}


def signal_owned_group(process, signum):
    try:
        os.killpg(process.pid, signum)
    except PermissionError:
        # Darwin excludes zombies from group signalling and can report EPERM until reaped.
        # Reap only our leader and retry once; a live group's permission failure still escapes.
        process.poll()
        os.killpg(process.pid, signum)


def stop_owned_group(process):
    """Only the session created for this command is ours; never signal the connected IDE."""
    cleanup_failure = None
    try:
        signal_owned_group(process, signal.SIGTERM)
        deadline = time.monotonic() + 0.2
        while time.monotonic() < deadline:
            process.poll()
            try:
                signal_owned_group(process, 0)
            except ProcessLookupError:
                break
            time.sleep(0.01)
    except ProcessLookupError:
        pass
    except OSError as exc:
        cleanup_failure = exc
    finally:
        try:
            signal_owned_group(process, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError as exc:
            cleanup_failure = cleanup_failure or exc
        # A denied group operation must not leave the direct child unreaped or pipes open.
        if process.poll() is None:
            try:
                process.kill()
            except ProcessLookupError:
                pass
            except OSError as exc:
                cleanup_failure = cleanup_failure or exc
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired as exc:
            cleanup_failure = cleanup_failure or exc
    if cleanup_failure is not None:
        raise ProcessCleanupError(f"could not clean up owned command group {process.pid}: {cleanup_failure}") from cleanup_failure


def execute(command, phase, root, bundle, timeout, failure_code=1):
    if os.name != "posix":
        raise PreviewError("preview capture needs POSIX owned process-group cleanup", UNSUPPORTED)
    stdout = bytearray()
    with open_artifact(bundle / f"{phase}.stdout.log") as stdout_log, \
            open_artifact(bundle / f"{phase}.stderr.log") as stderr_log:
        try:
            process = subprocess.Popen(
                command, cwd=root, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                stderr=subprocess.PIPE, start_new_session=True,
            )
        except OSError as exc:
            raise PreviewError(f"could not start {phase}: {exc}", failure_code) from exc
        try:
            with selectors.DefaultSelector() as selector:
                for pipe, name, log in (
                    (process.stdout, "stdout", stdout_log), (process.stderr, "stderr", stderr_log),
                ):
                    os.set_blocking(pipe.fileno(), False)
                    selector.register(pipe, selectors.EVENT_READ, (name, log))
                totals = {"stdout": 0, "stderr": 0}
                deadline = time.monotonic() + timeout
                stopped = False
                while selector.get_map() or process.poll() is None:
                    if process.poll() is not None and not stopped:
                        stop_owned_group(process)
                        stopped = True
                    remaining = deadline - time.monotonic()
                    if remaining <= 0:
                        raise PreviewError(f"{phase} timed out after {timeout}s; inspect {bundle}", failure_code)
                    for key, _ in selector.select(min(remaining, 0.05)):
                        chunk = os.read(key.fileobj.fileno(), 64 * 1024)
                        if not chunk:
                            selector.unregister(key.fileobj)
                            continue
                        name, log = key.data
                        retained = chunk[:max(0, MAX_COMMAND_LOG_BYTES - totals[name])]
                        log.write(retained)
                        if name == "stdout":
                            stdout.extend(retained)
                        totals[name] += len(chunk)
                        if totals[name] > MAX_COMMAND_LOG_BYTES:
                            raise PreviewError(
                                f"{phase} {name} exceeded the {MAX_COMMAND_LOG_BYTES} byte limit; inspect {bundle}",
                                failure_code,
                            )
        finally:
            try:
                stop_owned_group(process)
            finally:
                process.stdout.close()
                process.stderr.close()
    if process.returncode:
        raise PreviewError(f"{phase} exited with {process.returncode}; inspect {bundle}", failure_code)
    try:
        return stdout.decode("utf-8")
    except UnicodeError as exc:
        raise PreviewError(f"{phase} output is not UTF-8; raw output retained", failure_code) from exc


def capture(args, root=ROOT):
    mobile, source = request_paths(args, root)
    # Refuse a symlink that would move evidence out of the ignored artifacts tree.
    evidence = root / "artifacts/ui-evidence/compose-preview" / args.app
    if evidence.resolve() != root.resolve() / evidence.relative_to(root):
        raise PreviewError("preview evidence directory must not contain symlinks", 2)
    if args.dry_run:
        command = render_command(args, mobile, source, evidence / "<new-run>" / "preview.png")
        print("NOT_RUN: plan only; CLI, IDE, render and semantics were not checked")
        print(json.dumps(command, ensure_ascii=False))
        return 0
    if os.name != "posix":
        raise PreviewError("preview capture needs POSIX owned process-group cleanup", UNSUPPORTED)
    if shutil.which(args.android_cli) is None:
        raise PreviewError("official Android CLI unavailable; install it and open Android Studio Quail 2+", UNSUPPORTED)

    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:12]
    bundle = evidence / run_id
    bundle.mkdir(mode=0o700, parents=True, exist_ok=False)
    image = bundle / "preview.png"
    command = render_command(args, mobile, source, image)
    metadata = {
        "schema_version": 1,
        "status": "NOT_RUN",
        "app": args.app,
        "project": str(mobile),
        "source": str(source.relative_to(root.resolve())),
        "composable": args.composable,
        "label": args.label,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "renderer": "Android Studio layout renderer",
        "command": command,
        "timeout_seconds_per_command": args.timeout_seconds,
        "image_inspected": False,
        "semantics_asserted": False,
    }
    try:
        metadata["source_sha256"] = sha256(source, MAX_SOURCE_BYTES)
        metadata["git"] = git_provenance(root, bundle)
        metadata["android_cli_version"] = execute(
            [args.android_cli, "--version"], "version", root, bundle, args.timeout_seconds, UNSUPPORTED,
        ).strip()
        help_text = execute(
            [args.android_cli, "studio", "render-compose-preview", "--help"],
            "help", root, bundle, args.timeout_seconds, UNSUPPORTED,
        )
        if not all(flag in help_text for flag in ("--print-semantics", "--output-image-file", "--project")):
            raise PreviewError("android executable lacks the required preview command; legacy SDK tools are not supported", UNSUPPORTED)
        execute(
            [args.android_cli, "studio", "check"], "studio-check", root, bundle,
            args.timeout_seconds, UNSUPPORTED,
        )
        stdout = execute(command, "render", root, bundle, args.timeout_seconds)
        metadata["dimensions"], metadata["image_sha256"] = png_evidence(image)
        tree = semantics_json(stdout)
        if sha256(source, MAX_SOURCE_BYTES) != metadata["source_sha256"]:
            raise PreviewError("preview source changed during capture; rerun the same scenario")
        write_artifact(
            bundle / "semantics.json", json.dumps(tree, ensure_ascii=False, separators=(",", ":"), allow_nan=False) + "\n",
        )
        metadata["semantics_sha256"] = sha256(bundle / "semantics.json")
        metadata["status"] = "PASS"
    except PreviewError as exc:
        metadata["status"] = "UNSUPPORTED" if exc.code == UNSUPPORTED else "FAIL"
        metadata["error"] = str(exc)
        raise
    except (OSError, ValueError) as exc:
        metadata["status"] = "FAIL"
        metadata["error"] = str(exc)
        raise PreviewError(f"invalid or missing render output: {exc}") from exc
    except KeyboardInterrupt as exc:
        metadata["status"] = "FAIL"
        metadata["error"] = str(exc) or "capture interrupted"
        raise
    finally:
        write_artifact(bundle / "capture.json", json.dumps(metadata, ensure_ascii=False, indent=2) + "\n")
        print(f"Preview artifacts: {bundle}")
    print("PASS: PNG and semantics JSON captured; pixels and semantics assertions remain uninspected")
    return 0


def main():
    args = parser().parse_args()
    with capture_signals():
        try:
            return capture(args)
        except PreviewError as exc:
            status = "UNSUPPORTED" if exc.code == UNSUPPORTED else "FAIL"
            print(f"{status}: {exc}", file=sys.stderr)
            return exc.code
        except OSError as exc:
            print(f"FAIL: {exc}", file=sys.stderr)
            return 1
        except KeyboardInterrupt as exc:
            print(f"FAIL: {str(exc) or 'capture interrupted'}", file=sys.stderr)
            return 128 + exc.signum if isinstance(exc, CaptureInterrupted) else 130


if __name__ == "__main__":
    sys.exit(main())
