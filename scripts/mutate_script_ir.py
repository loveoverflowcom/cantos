#!/usr/bin/env python3
"""Run nine bounded source mutations against the actual Rust contract tests.

Uses a disposable copy, offline Cargo and an isolated target directory; it never
edits the working tree or rewrites fixtures. This is a targeted assertion check,
not a general mutation engine or a correctness proof.
"""

import hashlib
import os
import shutil
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EXPECTED_CONTRACT_TESTS = 16
MUTANTS = (
    (
        "duplicate identity admitted",
        "validation.rs",
        "self.ids.get(id)",
        "self.ids.get(id).filter(|_| false)",
    ),
    (
        "unknown speaker admitted",
        "validation.rs",
        "if !speakers.contains(&raw.speaker_id)",
        "if false && !speakers.contains(&raw.speaker_id)",
    ),
    (
        "missing narrator admitted",
        "validation.rs",
        "if narrators != 1",
        "if narrators > 1",
    ),
    (
        "cross-scene cue admitted",
        "validation.rs",
        "if !local_lines.contains(&cue.anchor.dialogue_id)",
        "if false && !local_lines.contains(&cue.anchor.dialogue_id)",
    ),
    (
        "missing provenance admitted",
        "validation.rs",
        "if !sources.contains(&raw.work.source_ref)",
        "if false && !sources.contains(&raw.work.source_ref)",
    ),
    (
        "normalization expansion admitted",
        "validation.rs",
        "if text.chars().count() > 10_000",
        "if false && text.chars().count() > 10_000",
    ),
    (
        "speech intensity omitted",
        "canonical.rs",
        "delivery.intensity_permille.to_string()",
        '"0".to_string()',
    ),
    (
        "normalized export byte bound skipped",
        "mod.rs",
        "if script.export_bytes().len() > wire::MAX_DOCUMENT_BYTES",
        "if false && script.export_bytes().len() > wire::MAX_DOCUMENT_BYTES",
    ),
    (
        "intensity token ignored",
        "wire.rs",
        "exact_unsigned_integer(number.get())",
        'exact_unsigned_integer("300")',
    ),
)


def run(workspace, env):
    return subprocess.run(
        ["cargo", "test", "-p", "cantos-api", "--offline", "--locked", "--test", "script_ir"],
        cwd=workspace,
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        timeout=60,
        check=False,
    )


def main():
    module = ROOT / "services/cantos-api/src/script_ir"
    for name in ("validation.rs", "canonical.rs", "mod.rs", "wire.rs"):
        print(f"input {name}: {hashlib.sha256((module / name).read_bytes()).hexdigest()}", flush=True)
    with tempfile.TemporaryDirectory(prefix="cantos-script-ir-mutations-") as temp:
        workspace = Path(temp) / "repo"
        workspace.mkdir()
        for name in ("Cargo.toml", "Cargo.lock"):
            shutil.copy2(ROOT / name, workspace / name)
        # Cargo resolves every workspace manifest, even for a package-scoped test, so each
        # member is copied without generated bundles.
        for member in ("services/cantos-api", "apps/cantos-forge", "apps/cantos-theatre/web",
                       "apps/cantos-studio/web", "packages/contracts", "packages/ui-system", "xtask"):
            shutil.copytree(ROOT / member, workspace / member, ignore=shutil.ignore_patterns("dist"))
        env = dict(os.environ, CARGO_TARGET_DIR=str(Path(temp) / "target"))
        baseline = run(workspace, env)
        summary = f"test result: ok. {EXPECTED_CONTRACT_TESTS} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
        if baseline.returncode != 0 or summary not in baseline.stdout:
            raise SystemExit(f"Mutation baseline did not pass all {EXPECTED_CONTRACT_TESTS} tests:\n" + baseline.stdout)
        print(f"baseline: passed ({EXPECTED_CONTRACT_TESTS} tests)", flush=True)
        counts = dict(killed=0, survived=0, timeout=0, unviable=0)
        for label, filename, needle, replacement in MUTANTS:
            target = workspace / "services/cantos-api/src/script_ir" / filename
            original = target.read_text(encoding="utf-8")
            if original.count(needle) != 1:
                raise SystemExit(f"Mutation target changed: {label}")
            target.write_text(original.replace(needle, replacement), encoding="utf-8")
            try:
                result = run(workspace, env)
                if result.returncode == 0:
                    outcome = "survived"
                elif "test result: FAILED" in result.stdout:
                    outcome = "killed"
                else:
                    outcome = "unviable"
                    print(result.stdout)
            except subprocess.TimeoutExpired:
                outcome = "timeout"
            finally:
                target.write_text(original, encoding="utf-8")
            counts[outcome] += 1
            print(f"{outcome}: {label}", flush=True)
        print(counts, flush=True)
        if counts != dict(killed=len(MUTANTS), survived=0, timeout=0, unviable=0):
            raise SystemExit("Classify each survivor, timeout or unviable mutation before claiming success.")


if __name__ == "__main__":
    main()
