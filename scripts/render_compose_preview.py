#!/usr/bin/env python3
"""Android Studio preview capture primitive; invoke through cargo xtask compose-preview.

No emulator, installation, IDE launch, Gradle bootstrap or app runtime is started.
Exit 3 means unavailable prerequisites; exit 2 means an invalid request.
"""

import argparse
import hashlib
import json
import re
import shutil
import struct
import subprocess
import sys
import uuid
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MOBILE_DIRS = {
    "cantos": "apps/cantos/mobile",
    "cantos-studio": "apps/cantos-studio/mobile",
}
UNSUPPORTED = 3


class PreviewError(Exception):
    def __init__(self, message, code=1):
        super().__init__(message)
        self.code = code


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
    mobile = (root / MOBILE_DIRS[args.app]).resolve()
    if not mobile.is_relative_to(root.resolve()):
        raise PreviewError("mobile project must stay inside the repository", 2)
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


def png_dimensions(image):
    """Check the PNG header, not visual quality or full pixel decoding."""
    data = image.read_bytes()
    if len(data) < 45 or data[:8] != b"\x89PNG\r\n\x1a\n" or data[8:16] != b"\x00\x00\x00\rIHDR":
        raise PreviewError("renderer did not produce a PNG with an IHDR header")
    width, height = struct.unpack(">II", data[16:24])
    if not width or not height:
        raise PreviewError("renderer produced a zero-size PNG")
    return {"width_px": width, "height_px": height}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git_provenance(root):
    def output(*args):
        try:
            result = subprocess.run(
                ["git", *args], cwd=root, stdin=subprocess.DEVNULL,
                capture_output=True, check=True, timeout=10,
            )
            return result.stdout.decode("utf-8").strip()
        except (OSError, subprocess.SubprocessError, UnicodeError):
            return None
    status = output("status", "--porcelain")
    return {"head": output("rev-parse", "HEAD"), "dirty": None if status is None else bool(status)}


def execute(command, phase, root, bundle, timeout, failure_code=1):
    stdout_path = bundle / f"{phase}.stdout.log"
    stderr_path = bundle / f"{phase}.stderr.log"
    try:
        result = subprocess.run(
            command, cwd=root, stdin=subprocess.DEVNULL, capture_output=True, timeout=timeout,
        )
    except subprocess.TimeoutExpired as exc:
        stdout_path.write_bytes(exc.stdout or b"")
        stderr_path.write_bytes(exc.stderr or b"")
        raise PreviewError(f"{phase} timed out after {timeout}s; inspect {bundle}", failure_code) from exc
    except OSError as exc:
        raise PreviewError(f"could not start {phase}: {exc}", failure_code) from exc
    stdout_path.write_bytes(result.stdout)
    stderr_path.write_bytes(result.stderr)
    if result.returncode:
        raise PreviewError(f"{phase} exited with {result.returncode}; inspect {bundle}", failure_code)
    try:
        return result.stdout.decode("utf-8")
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
    if shutil.which(args.android_cli) is None:
        raise PreviewError("official Android CLI unavailable; install it and open Android Studio Quail 2+", UNSUPPORTED)

    run_id = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ") + "-" + uuid.uuid4().hex[:12]
    bundle = evidence / run_id
    bundle.mkdir(parents=True, exist_ok=False)
    image = bundle / "preview.png"
    command = render_command(args, mobile, source, image)
    metadata = {
        "schema_version": 1,
        "status": "NOT_RUN",
        "app": args.app,
        "project": str(mobile),
        "source": str(source.relative_to(root.resolve())),
        "source_sha256": sha256(source),
        "composable": args.composable,
        "label": args.label,
        "started_at": datetime.now(timezone.utc).isoformat(),
        "git": git_provenance(root),
        "renderer": "Android Studio layout renderer",
        "command": command,
        "timeout_seconds_per_command": args.timeout_seconds,
        "image_inspected": False,
        "semantics_asserted": False,
    }
    try:
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
        metadata["dimensions"] = png_dimensions(image)
        tree = semantics_json(stdout)
        if sha256(source) != metadata["source_sha256"]:
            raise PreviewError("preview source changed during capture; rerun the same scenario")
        (bundle / "semantics.json").write_text(
            json.dumps(tree, ensure_ascii=False, indent=2, allow_nan=False) + "\n", encoding="utf-8",
        )
        metadata["image_sha256"] = sha256(image)
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
    finally:
        (bundle / "capture.json").write_text(
            json.dumps(metadata, ensure_ascii=False, indent=2) + "\n", encoding="utf-8",
        )
        print(f"Preview artifacts: {bundle}")
    print("PASS: PNG and semantics JSON captured; pixels and semantics assertions remain uninspected")
    return 0


def main():
    args = parser().parse_args()
    try:
        return capture(args)
    except PreviewError as exc:
        status = "UNSUPPORTED" if exc.code == UNSUPPORTED else "FAIL"
        print(f"{status}: {exc}", file=sys.stderr)
        return exc.code
    except OSError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
