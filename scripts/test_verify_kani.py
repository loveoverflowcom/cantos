#!/usr/bin/env python3
"""Fail-closed parser and execution-boundary tests using no real verifier.

Regular-output fixtures mirror the retained Kani 0.68.0 output shape. Fake
installed binaries exercise the adapter without installation, network or proofs.
"""

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import verify_kani as verifier


HARNESS = (
    "contexts::catalog::domain::revision::kani_proofs::"
    "catalog_revision_admission_and_successor"
)
UNREACHABLE = HARNESS + ".assertion.2"
KANI_VERSION = "0.68.0"
CBMC_VERSION = "6.11.0"
RUSTC_VERSION = "rustc 1.100.0-nightly (8925ea358 2026-08-20)"
TOOLCHAIN = "nightly-2026-08-21-x86_64-unknown-linux-gnu"
TARGET = "x86_64-unknown-linux-gnu"
PROCESS_BOUNDARY_SUPPORTED = os.name == "posix" and hasattr(os, "WNOWAIT")


def check_block(number, identifier, status, description="fixture property"):
    return (
        f"Check {number}: {identifier}\n"
        f"\t - Status: {status}\n"
        f'\t - Description: "{description}"\n'
        "\t - Location: services/cantos-api/src/contexts/catalog/domain/"
        f"revision.rs:100:9 in function {HARNESS}\n"
    )


ORDINARY_BLOCK = check_block(1, HARNESS + ".assertion.1", "SUCCESS")
UNREACHABLE_BLOCK = check_block(2, UNREACHABLE, "UNREACHABLE")
SAFETY_BLOCK = check_block(3, HARNESS + ".arithmetic_overflow.1", "SUCCESS")
COVER_BLOCK = check_block(4, HARNESS + ".cover.1", "SATISFIED", "zero rejected")
SECOND_COVER_BLOCK = check_block(
    5, HARNESS + ".cover.2", "SATISFIED", "first accepted"
)
SUMMARY = (
    "SUMMARY:\n"
    " ** 0 of 3 failed (1 unreachable)\n\n"
    " ** 2 of 2 cover properties satisfied\n\n"
    "VERIFICATION:- SUCCESSFUL\n"
    "Verification Time: 0.1s\n\n"
    "Manual Harness Summary:\n"
    "Complete - 1 successfully verified harnesses, 0 failures, 1 total.\n"
)
VALID_OUTPUT = (
    "Kani Rust Verifier 0.68.0 (standalone)\n"
    f"Checking harness {HARNESS}...\n"
    + "\n".join(
        [ORDINARY_BLOCK, UNREACHABLE_BLOCK, SAFETY_BLOCK, COVER_BLOCK, SECOND_COVER_BLOCK]
    )
    + "\n"
    + SUMMARY
)


class ValidateOutputTest(unittest.TestCase):
    """The pure core accepts only complete, nonvacuous, consistent evidence."""

    def validate(self, text=VALID_OUTPUT, covers=2, allowed=(UNREACHABLE,)):
        return verifier.validate_output(text, HARNESS, covers, allowed)

    def assert_rejected(self, text, covers=2, allowed=(UNREACHABLE,)):
        with self.assertRaises(verifier.KaniError):
            self.validate(text, covers, allowed)

    def test_regular_output_counts_ordinary_checks_separately_from_covers(self):
        result = self.validate()
        self.assertEqual(result["checks"], 3)
        self.assertEqual(result["covers"], 2)
        self.assertEqual(set(result["unreachable"]), {UNREACHABLE})

    def test_no_unreachable_properties_need_no_allowance(self):
        text = VALID_OUTPUT.replace("Status: UNREACHABLE", "Status: SUCCESS").replace(
            " (1 unreachable)", ""
        )
        result = self.validate(text, allowed=())
        self.assertEqual(result["checks"], 3)
        self.assertEqual(set(result["unreachable"]), set())

    def test_success_markers_without_check_blocks_are_rejected(self):
        self.assert_rejected(f"Checking harness {HARNESS}...\n" + SUMMARY)

    def test_covers_alone_cannot_make_an_empty_proof_successful(self):
        text = (
            f"Checking harness {HARNESS}...\n"
            + check_block(1, HARNESS + ".cover.1", "SATISFIED")
            + check_block(2, HARNESS + ".cover.2", "SATISFIED")
            + SUMMARY.replace("0 of 3 failed (1 unreachable)", "0 of 0 failed (0 unreachable)")
        )
        self.assert_rejected(text, allowed=())

    def test_every_ordinary_failure_or_unknown_status_is_rejected(self):
        for status in ("FAILURE", "UNDETERMINED", "ERROR", "UNKNOWN", "SATISFIED"):
            with self.subTest(status=status):
                self.assert_rejected(
                    VALID_OUTPUT.replace("Status: SUCCESS", f"Status: {status}", 1)
                )

    def test_every_unsatisfied_or_misclassified_cover_is_rejected(self):
        for status in ("FAILED", "FAILURE", "UNSATISFIABLE", "UNREACHABLE", "SUCCESS"):
            with self.subTest(status=status):
                self.assert_rejected(
                    VALID_OUTPUT.replace("Status: SATISFIED", f"Status: {status}", 1),
                    allowed=(UNREACHABLE, HARNESS + ".cover.1"),
                )

    def test_unlisted_unreachable_check_is_rejected(self):
        self.assert_rejected(VALID_OUTPUT, allowed=())

    def test_a_similar_id_does_not_authorize_an_unreachable_check(self):
        self.assert_rejected(VALID_OUTPUT, allowed=(UNREACHABLE + "0",))

    def test_wrong_missing_or_repeated_harness_is_rejected(self):
        marker = f"Checking harness {HARNESS}...\n"
        for text in (
            VALID_OUTPUT.replace(marker, "Checking harness another::proof...\n"),
            VALID_OUTPUT.replace(marker, ""),
            marker + VALID_OUTPUT,
            VALID_OUTPUT + "Checking harness another::proof...\n",
        ):
            with self.subTest(text=text[:100]):
                self.assert_rejected(text)

    def test_duplicate_check_numbers_and_ids_are_rejected(self):
        for text in (
            VALID_OUTPUT.replace("Check 3:", "Check 1:"),
            VALID_OUTPUT.replace(HARNESS + ".arithmetic_overflow.1", HARNESS + ".assertion.1"),
            VALID_OUTPUT.replace(ORDINARY_BLOCK, ORDINARY_BLOCK + ORDINARY_BLOCK),
        ):
            with self.subTest(text=text[:100]):
                self.assert_rejected(text)

    def test_malformed_check_headers_are_rejected(self):
        for header in ("Check nope:", "Check 0:", "Check 1", "Check 1: "):
            with self.subTest(header=header):
                replacement = (
                    header if header.endswith(" ") else header + " " + HARNESS + ".assertion.1"
                )
                text = VALID_OUTPUT.replace(
                    "Check 1: " + HARNESS + ".assertion.1", replacement
                )
                self.assert_rejected(text)

    def test_each_check_requires_status_description_and_location(self):
        lines = ORDINARY_BLOCK.splitlines(keepends=True)
        for index in (1, 2, 3):
            with self.subTest(missing=lines[index].strip()):
                self.assert_rejected(
                    VALID_OUTPUT.replace(
                        ORDINARY_BLOCK, "".join(lines[:index] + lines[index + 1 :])
                    )
                )

    def test_duplicate_check_fields_are_rejected(self):
        for line in ORDINARY_BLOCK.splitlines(keepends=True)[1:]:
            with self.subTest(field=line.strip()):
                self.assert_rejected(VALID_OUTPUT.replace(line, line + line, 1))

    def test_empty_check_field_values_are_rejected(self):
        for line in ORDINARY_BLOCK.splitlines(keepends=True)[1:]:
            with self.subTest(field=line.strip()):
                empty = line.split(":", 1)[0] + ":\n"
                self.assert_rejected(VALID_OUTPUT.replace(line, empty, 1))

    def test_check_summary_must_agree_with_observed_noncover_properties(self):
        for summary in (
            "0 of 2 failed (1 unreachable)",
            "0 of 5 failed (1 unreachable)",
            "1 of 3 failed (1 unreachable)",
            "0 of 3 failed (0 unreachable)",
            "0 of 3 failed (2 unreachable)",
        ):
            with self.subTest(summary=summary):
                self.assert_rejected(VALID_OUTPUT.replace("0 of 3 failed (1 unreachable)", summary))

    def test_cover_summary_must_agree_with_fixture_and_requested_count(self):
        for summary in (
            "1 of 2 cover properties satisfied",
            "2 of 3 cover properties satisfied",
            "1 of 1 cover properties satisfied",
        ):
            with self.subTest(summary=summary):
                self.assert_rejected(
                    VALID_OUTPUT.replace("2 of 2 cover properties satisfied", summary)
                )
        self.assert_rejected(VALID_OUTPUT, covers=1)
        self.assert_rejected(VALID_OUTPUT, covers=3)

    def test_exact_registered_cover_ids_are_required(self):
        self.assert_rejected(
            VALID_OUTPUT.replace(HARNESS + ".cover.2", HARNESS + ".cover.20")
        )

    def test_removed_unreachable_property_cannot_reuse_a_stale_allowance(self):
        text = VALID_OUTPUT.replace("Status: UNREACHABLE", "Status: SUCCESS").replace(
            " (1 unreachable)", ""
        )
        self.assert_rejected(text)

    def test_missing_malformed_or_duplicate_summary_is_rejected(self):
        for text in (
            VALID_OUTPUT.replace("SUMMARY:\n", ""),
            VALID_OUTPUT.replace("0 of 3 failed (1 unreachable)", "zero of three failed"),
            VALID_OUTPUT.replace(" ** 2 of 2 cover properties satisfied\n", ""),
            VALID_OUTPUT + "\nSUMMARY:\n ** 0 of 3 failed (1 unreachable)\n",
        ):
            with self.subTest(text=text[-150:]):
                self.assert_rejected(text)

    def test_missing_failed_or_duplicated_verification_is_rejected(self):
        marker = "VERIFICATION:- SUCCESSFUL\n"
        for text in (
            VALID_OUTPUT.replace(marker, ""),
            VALID_OUTPUT.replace(marker, "VERIFICATION:- FAILED\n"),
            VALID_OUTPUT.replace(marker, "VERIFICATION:- SUCCESSFUL extra\n"),
            VALID_OUTPUT + marker,
        ):
            with self.subTest(text=text[-150:]):
                self.assert_rejected(text)

    def test_only_exact_one_successful_harness_completion_is_accepted(self):
        marker = "Complete - 1 successfully verified harnesses, 0 failures, 1 total.\n"
        for completion in (
            "",
            "Complete - 0 successfully verified harnesses, 0 failures, 0 total.\n",
            "Complete - 1 successfully verified harnesses, 1 failures, 2 total.\n",
            "Complete - 2 successfully verified harnesses, 0 failures, 2 total.\n",
            marker + marker,
        ):
            with self.subTest(completion=completion):
                self.assert_rejected(VALID_OUTPUT.replace(marker, completion))


def compatibility_args(**overrides):
    values = {
        "kani_version": KANI_VERSION,
        "cbmc_version": CBMC_VERSION,
        "rustc_version": RUSTC_VERSION,
        "toolchain": TOOLCHAIN,
        "target": TARGET,
    }
    values.update(overrides)
    return argparse.Namespace(**values)


class FakeInstalledBundle:
    """Executable local doubles in the documented already-installed layout."""

    def __init__(self, root):
        self.root = Path(root)
        self.bundle = self.root / "installed" / ("kani-" + KANI_VERSION)
        self.calls = self.root / "calls.jsonl"
        (self.bundle / "bin").mkdir(parents=True)
        (self.bundle / "toolchain/bin").mkdir(parents=True)
        (self.bundle / "rustc-version").write_text(RUSTC_VERSION + "\n", encoding="utf-8")
        (self.bundle / "rust-toolchain-version").write_text(TOOLCHAIN + "\n", encoding="utf-8")
        self.write_driver()
        self.write_executable("bin/kani-compiler", "compiler", f"print({RUSTC_VERSION!r})\n")
        self.write_executable("bin/cbmc", "cbmc", f"print({CBMC_VERSION!r})\n")
        self.write_rustc()
        self.write_executable("toolchain/bin/cargo", "cargo", "raise SystemExit(99)\n")
        # Calling a wrapper is forbidden: it may install an absent bundle.
        self.write_executable("bin/kani", "wrapper", "raise SystemExit(98)\n")
        self.write_executable("bin/cargo-kani", "wrapper", "raise SystemExit(98)\n")

    def write_executable(self, relative, program, body):
        path = self.bundle / relative
        path.write_text(
            f"#!{sys.executable}\n"
            "import json, os, pathlib, sys\n"
            f"with pathlib.Path({str(self.calls)!r}).open('a', encoding='utf-8') as calls:\n"
            "    calls.write(json.dumps("
            f"{{'program': {program!r}, 'argv': sys.argv[1:]}}) + '\\n')\n"
            + body,
            encoding="utf-8",
        )
        path.chmod(0o755)
        return path

    def write_driver(self, version=None, proof_body=None, version_body=""):
        if version is None:
            version = f"Kani Rust Verifier {KANI_VERSION} (cargo plugin)\nCBMC {CBMC_VERSION}"
        if proof_body is None:
            proof_body = f"print({VALID_OUTPUT!r}, end='')\n"
        return self.write_executable(
            "bin/kani-driver",
            "driver",
            "if '--version' in sys.argv:\n"
            + "".join("    " + line + "\n" for line in version_body.splitlines())
            + f"    print({version!r})\n"
            "else:\n"
            + "".join("    " + line + "\n" for line in proof_body.splitlines()),
        )

    def write_rustc(self, version=RUSTC_VERSION, target=TARGET):
        verbose = (
            version + "\n"
            "binary: rustc\n"
            "commit-hash: 8925ea358fixture\n"
            "commit-date: 2026-08-20\n"
            f"host: {target}\n"
            "release: 1.100.0-nightly\n"
            "LLVM version: 22.1.0"
        )
        return self.write_executable(
            "toolchain/bin/rustc",
            "rustc",
            f"print({verbose!r} if '-vV' in sys.argv else {version!r})\n",
        )

    def read_calls(self):
        if not self.calls.exists():
            return []
        return [json.loads(line) for line in self.calls.read_text(encoding="utf-8").splitlines()]


@unittest.skipUnless(PROCESS_BOUNDARY_SUPPORTED, "probe requires a POSIX waitid boundary")
class InstalledBundleTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.fake = FakeInstalledBundle(self.root)
        self.environment = {
            "PATH": os.defpath,
            "HOME": str(self.root),
            "KANI_HOME": str(self.fake.bundle.parent),
        }

    def tearDown(self):
        self.directory.cleanup()

    def probe(self, **overrides):
        with patch.dict(os.environ, self.environment, clear=True), patch.object(
            sys, "platform", "linux"
        ):
            return verifier.probe(self.fake.bundle, compatibility_args(**overrides))

    def test_probe_uses_direct_driver_and_existing_toolchain_only(self):
        result = self.probe()
        self.assertIsInstance(result, dict)
        calls = self.fake.read_calls()
        self.assertIn({"program": "driver", "argv": ["kani", "--version"]}, calls)
        self.assertIn({"program": "rustc", "argv": ["--version"]}, calls)
        self.assertIn({"program": "rustc", "argv": ["-vV"]}, calls)
        self.assertFalse(any(call["program"] in {"cargo", "wrapper"} for call in calls))

    def test_relative_kani_home_probe_resolves_the_bundle_before_changing_cwd(self):
        self.environment["KANI_HOME"] = os.path.relpath(self.fake.bundle.parent, Path.cwd())
        with patch.dict(os.environ, self.environment, clear=True), patch.object(
            sys, "platform", "linux"
        ):
            bundle = Path(os.environ["KANI_HOME"]) / ("kani-" + KANI_VERSION)
            self.assertFalse(bundle.is_absolute())
            result = verifier.probe(bundle, compatibility_args())
        self.assertEqual(result["bundle"], str(self.fake.bundle.resolve()))
        self.assertIn(
            {"program": "driver", "argv": ["kani", "--version"]}, self.fake.read_calls()
        )
        self.assertFalse(
            any(call["program"] in {"cargo", "wrapper"} for call in self.fake.read_calls())
        )

    def test_missing_bundle_does_not_call_an_installing_wrapper(self):
        with self.assertRaises(verifier.KaniError):
            verifier.probe(self.root / "not-installed", compatibility_args())
        self.assertEqual(self.fake.read_calls(), [])

    def test_every_required_installed_file_is_checked(self):
        for relative in (
            "bin/kani-driver",
            "bin/kani-compiler",
            "bin/cbmc",
            "toolchain/bin/cargo",
            "toolchain/bin/rustc",
            "rustc-version",
            "rust-toolchain-version",
        ):
            with self.subTest(missing=relative):
                path = self.fake.bundle / relative
                saved = path.with_name(path.name + ".saved")
                path.rename(saved)
                try:
                    with self.assertRaises(verifier.KaniError):
                        self.probe()
                finally:
                    saved.rename(path)
        self.assertFalse(any(call["program"] == "wrapper" for call in self.fake.read_calls()))

    def test_nonexecutable_driver_is_rejected_without_installation(self):
        (self.fake.bundle / "bin/kani-driver").chmod(0o644)
        with self.assertRaises(verifier.KaniError):
            self.probe()
        self.assertEqual(self.fake.read_calls(), [])

    def test_unregistered_target_is_rejected_before_any_probe_process(self):
        with self.assertRaises(verifier.KaniError):
            self.probe(target="aarch64-unknown-linux-gnu")
        self.assertEqual(self.fake.read_calls(), [])

    def test_kani_and_cbmc_version_mismatches_are_rejected(self):
        for version in (
            "Kani Rust Verifier 0.67.0 (cargo plugin)\nCBMC 6.11.0",
            "Kani Rust Verifier 0.68.0 (cargo plugin)\nCBMC 6.10.0",
            "Kani Rust Verifier 0.68.0 (cargo plugin)",
            "Kani Rust Verifier 0.68.0 (cargo plugin)\nCBMC 6.11.0\nunexpected output",
        ):
            with self.subTest(version=version):
                self.fake.write_driver(version=version)
                with self.assertRaises(verifier.KaniError):
                    self.probe()

    def test_rustc_version_and_target_mismatches_are_rejected(self):
        for version, target in (
            ("rustc 1.87.0 (fixture)", TARGET),
            (RUSTC_VERSION, "aarch64-unknown-linux-gnu"),
        ):
            with self.subTest(version=version, target=target):
                self.fake.write_rustc(version, target)
                with self.assertRaises(verifier.KaniError):
                    self.probe()

    def test_bundle_toolchain_and_rustc_sentinel_mismatches_are_rejected(self):
        for relative, value in (
            ("rust-toolchain-version", "nightly-2026-08-20-x86_64-unknown-linux-gnu"),
            ("rustc-version", "rustc 1.87.0 (fixture)"),
        ):
            with self.subTest(sentinel=relative):
                path = self.fake.bundle / relative
                original = path.read_text(encoding="utf-8")
                path.write_text(value + "\n", encoding="utf-8")
                try:
                    with self.assertRaises(verifier.KaniError):
                        self.probe()
                finally:
                    path.write_text(original, encoding="utf-8")


class ChildEnvironmentTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.fake = FakeInstalledBundle(self.directory.name)

    def tearDown(self):
        self.directory.cleanup()

    def test_existing_bundle_path_and_toolchain_are_child_only(self):
        original = {
            "PATH": "/fixture/system/bin",
            "HOME": "/fixture/home",
            "LANG": "C.UTF-8",
            "RUSTUP_TOOLCHAIN": "1.87.0",
        }
        saved = dict(original)
        child = verifier.child_environment(self.fake.bundle, original)
        self.assertEqual(original, saved)
        self.assertEqual(
            child["PATH"].split(os.pathsep),
            [
                str(self.fake.bundle / "bin"),
                str(self.fake.bundle / "toolchain/bin"),
                "/fixture/system/bin",
            ],
        )
        self.assertEqual(child["RUSTUP_TOOLCHAIN"], TOOLCHAIN)
        self.assertEqual(child["HOME"], "/fixture/home")
        self.assertEqual(child["LANG"], "C.UTF-8")

    def test_weakening_environment_is_rejected_by_name_without_value_disclosure(self):
        # This list is independent of the implementation's forbidden-key table.
        names = (
            "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTFLAGS",
            "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
            "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS",
        )
        private_value = "private-value-must-not-appear --no-unwinding-checks"
        for name in names:
            with self.subTest(name=name):
                with self.assertRaises(verifier.KaniError) as caught:
                    verifier.child_environment(self.fake.bundle, {name: private_value})
                self.assertIn(name, str(caught.exception))
                self.assertNotIn(private_value, str(caught.exception))
                self.assertNotIn("private-value-must-not-appear", str(caught.exception))

    def test_multiple_rejected_names_are_reported_without_their_values(self):
        with self.assertRaises(verifier.KaniError) as caught:
            verifier.child_environment(
                self.fake.bundle, {"RUSTC_WRAPPER": "secret-one", "RUSTFLAGS": "secret-two"}
            )
        self.assertIn("RUSTC_WRAPPER", str(caught.exception))
        self.assertIn("RUSTFLAGS", str(caught.exception))
        self.assertNotIn("secret-one", str(caught.exception))
        self.assertNotIn("secret-two", str(caught.exception))

    def test_diagnostic_or_superseded_variables_are_not_speculatively_rejected(self):
        original = {
            "CARGO": "/unused/cargo", "KANI_LOG": "info", "TIME_COMPILER": "1",
            "FLAMEGRAPH": "compiler", "CARGO_TARGET_DIR": "/unused/target",
            "KANI_FLAGS": "not-consumed-by-this-release",
        }
        child = verifier.child_environment(self.fake.bundle, original)
        for key, value in original.items():
            self.assertEqual(child[key], value)

    def test_rustup_linker_entries_are_removed_without_mutating_parent(self):
        original = {
            "LD_LIBRARY_PATH": "/fixture/rustup/toolchains/nightly/lib:/fixture/other/lib",
            "DYLD_FALLBACK_LIBRARY_PATH": (
                "/fixture/other/lib:/fixture/rustup/toolchains/nightly/lib"
            ),
        }
        saved = dict(original)
        child = verifier.child_environment(self.fake.bundle, original)
        self.assertEqual(original, saved)
        self.assertEqual(child["LD_LIBRARY_PATH"], "/fixture/other/lib")
        self.assertEqual(child["DYLD_FALLBACK_LIBRARY_PATH"], "/fixture/other/lib")


class ConfigurationIdentityTest(unittest.TestCase):
    """The approved identity includes absent as well as existing Cargo candidates."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.checkout = self.root / "projects/checkout"
        self.checkout.mkdir(parents=True)
        (self.checkout / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        self.api_manifest = self.checkout / "services/cantos-api/Cargo.toml"
        self.api_manifest.parent.mkdir(parents=True)
        self.api_manifest.write_text("[package]\nname = 'fixture'\n", encoding="utf-8")
        self.environment = {
            "HOME": str(self.root / "home"),
            "CARGO_HOME": str(self.root / "cargo-home"),
        }

    def tearDown(self):
        self.directory.cleanup()

    def test_configuration_digest_consumes_only_limit_plus_one_from_controlled_reader(self):
        class EndlessReader:
            def __init__(self):
                self.requests = []

            def __enter__(self):
                return self

            def __exit__(self, *_args):
                return False

            def read(self, count):
                self.requests.append(count)
                return b' ' * count

        reader = EndlessReader()
        with patch.object(Path, "open", return_value=reader):
            with self.assertRaisesRegex(verifier.KaniError, "(?i)configuration.*large"):
                verifier.configuration_sha256(self.api_manifest)
        self.assertEqual(reader.requests, [1024 * 1024 + 1])

    def test_configuration_digest_hashes_exact_buffer_and_accepts_limit(self):
        for content in (b"# exact bytes\r\n[build]\njobs=2\n", b' ' * (1024 * 1024)):
            with self.subTest(size=len(content)):
                self.api_manifest.write_bytes(content)
                self.assertEqual(
                    verifier.configuration_sha256(self.api_manifest),
                    hashlib.sha256(content).hexdigest(),
                )

    def test_configuration_identity_refuses_bounded_oversize_fixture(self):
        self.api_manifest.write_bytes(b' ' * (1024 * 1024 + 1))
        with self.assertRaisesRegex(verifier.KaniError, "(?i)configuration.*large"):
            verifier.configuration_identity(self.checkout, self.environment)

    def expected_identity(self, cargo_home):
        # Independent oracle for the fixed candidate set, with no TOML interpretation.
        checkout = self.checkout.resolve()
        candidates = {checkout / "Cargo.toml", self.api_manifest.resolve()}
        for ancestor in (checkout, *checkout.parents):
            candidates.update(ancestor / ".cargo" / name for name in ("config", "config.toml"))
        candidates.update(cargo_home / name for name in ("config", "config.toml"))
        return {
            str(path): hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
            for path in candidates
        }

    def test_every_manifest_and_ancestor_cargo_candidate_is_bound_even_when_absent(self):
        identity = verifier.configuration_identity(self.checkout, self.environment)
        self.assertEqual(identity, self.expected_identity(self.root / "cargo-home"))
        self.assertIsNone(identity[str(self.checkout / ".cargo/config")])
        self.assertIsNone(identity[str(self.checkout / ".cargo/config.toml")])
        self.assertTrue(all(Path(path).is_absolute() for path in identity))

    def test_both_existing_cargo_filenames_are_hashed_at_every_selected_location(self):
        for parent in (
            self.checkout / ".cargo", self.root / ".cargo", self.root / "cargo-home"
        ):
            parent.mkdir(parents=True)
            for name in ("config", "config.toml"):
                (parent / name).write_text("[build]\njobs = 2\n", encoding="utf-8")
        identity = verifier.configuration_identity(self.checkout, self.environment)
        self.assertEqual(identity, self.expected_identity(self.root / "cargo-home"))
        digest = hashlib.sha256(b"[build]\njobs = 2\n").hexdigest()
        self.assertEqual(identity[str(self.root / "cargo-home/config")], digest)
        self.assertEqual(identity[str(self.root / "cargo-home/config.toml")], digest)

    def test_home_cargo_fallback_is_bound_when_cargo_home_is_unset(self):
        del self.environment["CARGO_HOME"]
        cargo_home = self.root / "home/.cargo"
        cargo_home.mkdir(parents=True)
        (cargo_home / "config.toml").write_text("[build]\njobs = 1\n", encoding="utf-8")
        identity = verifier.configuration_identity(self.checkout, self.environment)
        self.assertEqual(identity, self.expected_identity(cargo_home))
        self.assertIsNone(identity[str(cargo_home / "config")])
        self.assertEqual(
            identity[str(cargo_home / "config.toml")],
            hashlib.sha256(b"[build]\njobs = 1\n").hexdigest(),
        )


@unittest.skipUnless(PROCESS_BOUNDARY_SUPPORTED, "runner requires a POSIX waitid boundary")
class BoundedRunTest(unittest.TestCase):
    """Fault injection bounds output/time and leaves no executing owned child."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.output = self.root / "retained.log"

    def tearDown(self):
        self.directory.cleanup()

    def run_python(self, source, timeout=3, max_bytes=4096):
        return verifier.bounded_run(
            [sys.executable, "-c", source],
            self.root,
            dict(os.environ),
            timeout,
            self.output,
            max_bytes=max_bytes,
        )

    def test_stdout_and_stderr_are_merged_and_retained(self):
        result = self.run_python(
            "import sys; print('stdout evidence', flush=True); "
            "print('stderr evidence', file=sys.stderr, flush=True)"
        )
        self.assertIsInstance(result, subprocess.CompletedProcess)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "stdout evidence\nstderr evidence\n")
        self.assertEqual(self.output.read_text(encoding="utf-8"), result.stdout)

    def test_nonzero_exit_remains_visible_to_the_calling_adapter(self):
        result = self.run_python("print('failed proof output', flush=True); raise SystemExit(7)")
        self.assertEqual(result.returncode, 7)
        self.assertIn("failed proof output", result.stdout)
        self.assertEqual(self.output.read_text(encoding="utf-8"), result.stdout)

    def test_timeout_retains_output_and_is_not_a_success(self):
        started = time.monotonic()
        with self.assertRaisesRegex(verifier.KaniError, "(?i)time"):
            self.run_python(
                "import time; print('before timeout', flush=True); time.sleep(30)",
                timeout=0.3,
            )
        self.assertLess(time.monotonic() - started, 5)
        self.assertIn("before timeout", self.output.read_text(encoding="utf-8"))

    def test_output_limit_retains_only_bounded_output_and_is_not_a_success(self):
        with self.assertRaisesRegex(verifier.KaniError, "(?i)output|limit|cap"):
            self.run_python("print('x' * 100000, flush=True)", max_bytes=1024)
        self.assertGreater(self.output.stat().st_size, 0)
        self.assertLessEqual(self.output.stat().st_size, 1024)

    def test_output_exactly_at_limit_is_allowed(self):
        result = self.run_python("import sys; sys.stdout.write('x' * 1024)", max_bytes=1024)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "x" * 1024)
        self.assertEqual(self.output.stat().st_size, 1024)

    def process_is_executing(self, pid):
        try:
            os.kill(pid, 0)
        except ProcessLookupError:
            return False
        # A killed orphan may await host-init reaping; it cannot execute.
        stat = Path(f"/proc/{pid}/stat")
        try:
            return stat.read_text(encoding="utf-8").rsplit(")", 1)[1].split()[0] != "Z"
        except FileNotFoundError:
            return not Path("/proc").exists()

    @unittest.skipUnless(os.name == "posix", "process-group cleanup requires POSIX")
    def test_timeout_and_output_cap_stop_the_entire_owned_process_group(self):
        for mode in ("timeout", "output"):
            with self.subTest(mode=mode):
                self.output = self.root / f"{mode}-retained.log"
                pids_path = self.root / f"{mode}-pids.json"
                source = (
                    "import json, os, pathlib, subprocess, sys, time\n"
                    "child = subprocess.Popen("
                    "[sys.executable, '-c', 'import time; time.sleep(30)'])\n"
                    f"pathlib.Path({str(pids_path)!r}).write_text("
                    "json.dumps([os.getpid(), child.pid]))\n"
                    "print('owned children started', flush=True)\n"
                    + (
                        "time.sleep(30)\n"
                        if mode == "timeout"
                        else "while True:\n    print('x' * 4096, flush=True)\n"
                    )
                )
                try:
                    with self.assertRaises(verifier.KaniError):
                        self.run_python(source, timeout=0.4, max_bytes=1024)
                    self.assertTrue(pids_path.is_file(), "helper did not reach the fault point")
                    pids = json.loads(pids_path.read_text(encoding="utf-8"))
                    deadline = time.monotonic() + 3
                    while (
                        any(self.process_is_executing(pid) for pid in pids)
                        and time.monotonic() < deadline
                    ):
                        time.sleep(0.02)
                    self.assertFalse(any(self.process_is_executing(pid) for pid in pids))
                finally:
                    if pids_path.is_file():
                        pids = json.loads(pids_path.read_text(encoding="utf-8"))
                        for pid in pids:
                            try:
                                os.kill(pid, signal.SIGKILL)
                            except ProcessLookupError:
                                pass

    def test_exited_leader_cannot_leave_a_descendant_holding_the_output_pipe(self):
        pid_path = self.root / "descendant.pid"
        source = (
            "import pathlib, subprocess, sys\n"
            "child = subprocess.Popen([sys.executable, '-c', 'import time; time.sleep(30)'])\n"
            f"pathlib.Path({str(pid_path)!r}).write_text(str(child.pid))\n"
            "print('leader complete', flush=True)\n"
        )
        started = time.monotonic()
        try:
            result = self.run_python(source)
            self.assertEqual(result.returncode, 0)
            self.assertIn("leader complete", result.stdout)
            self.assertLess(time.monotonic() - started, 3)
            self.assertTrue(pid_path.is_file())
            pid = int(pid_path.read_text(encoding="utf-8"))
            self.assertFalse(self.process_is_executing(pid))
        finally:
            if pid_path.is_file():
                try:
                    os.kill(int(pid_path.read_text(encoding="utf-8")), signal.SIGKILL)
                except ProcessLookupError:
                    pass


@unittest.skipUnless(PROCESS_BOUNDARY_SUPPORTED, "adapter requires a POSIX waitid boundary")
class VerifyAdapterTest(unittest.TestCase):
    """The real adapter runs fake installed executables and retains its evidence."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        self.fake = FakeInstalledBundle(self.root)
        self.checkout = self.root / "checkout"
        self.checkout.mkdir()
        self.source = Path("services/cantos-api/src/contexts/catalog/domain/revision.rs")
        source = self.checkout / self.source
        source.parent.mkdir(parents=True)
        source.write_text(
            "// Disposable proof input; the fake driver never compiles it.\n",
            encoding="utf-8",
        )
        (self.checkout / "Cargo.toml").write_text("[workspace]\nmembers = []\n", encoding="utf-8")
        (self.checkout / "services/cantos-api/Cargo.toml").write_text(
            "[package]\nname = 'cantos-api'\nversion = '0.1.0'\n", encoding="utf-8"
        )
        self.transitive_source = Path("services/cantos-api/src/contexts/catalog/domain/snapshot.rs")
        (self.checkout / self.transitive_source).write_text(
            "// Separate transitive API input.\n", encoding="utf-8"
        )
        self.contract_source = Path("packages/contracts/api/src/lib.rs")
        (self.checkout / self.contract_source).parent.mkdir(parents=True)
        (self.checkout / self.contract_source).write_text(
            "// Separate contracts input.\n", encoding="utf-8"
        )
        for relative in (
            "packages/contracts/api/Cargo.toml",
            "packages/contracts/api/build.rs",
            "services/cantos-api/build.rs",
            "packages/contracts/schema/script-ir.schema.json",
            "scripts/verify_kani.py",
            "xtask/src/kani.rs",
        ):
            path = self.checkout / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("disposable model input\n", encoding="utf-8")
        (self.checkout / ".cargo").mkdir()
        (self.checkout / ".cargo/config.toml").write_text("[build]\njobs = 2\n", encoding="utf-8")
        self.lock = self.checkout / "Cargo.lock"
        self.lock.write_text("# disposable immutable lock fixture\nversion = 4\n", encoding="utf-8")
        self.original_lock = self.lock.read_bytes()
        self.environment = {
            "PATH": os.defpath,
            "HOME": str(self.root),
            "KANI_HOME": str(self.fake.bundle.parent),
        }
        self.args = compatibility_args(
            probe=False,
            harness=HARNESS,
            source=str(self.source),
            unwind=2,
            cover_count=2,
            allow_unreachable=[UNREACHABLE],
            approved_config=json.dumps(verifier.configuration_identity(self.checkout, self.environment)),
        )
        self.git_identity = {
            "revision": "a" * 40,
            "status_sha256": "b" * 64,
            "diff_sha256": "c" * 64,
            "dirty": False,
        }

    def tearDown(self):
        self.directory.cleanup()

    def verify(self):
        with patch.dict(os.environ, self.environment, clear=True), patch.object(
            sys, "platform", "linux"
        ), patch.object(verifier, "git_identity", return_value=self.git_identity):
            return verifier.verify(self.args, root=self.checkout)

    def approve_configuration(self):
        self.args.approved_config = json.dumps(
            verifier.configuration_identity(self.checkout, self.environment)
        )

    def proof_calls(self):
        return [
            call for call in self.fake.read_calls()
            if call["program"] == "driver" and "--version" not in call["argv"]
        ]

    def assert_configuration_refused_before_execution(self):
        with self.assertRaisesRegex(verifier.KaniError, "(?i)configuration|approved"):
            self.verify()
        self.assertEqual(self.fake.read_calls(), [], "stale approval must fail before any probe")
        self.assertEqual(self.proof_calls(), [])
        self.assertEqual(self.lock.read_bytes(), self.original_lock)

    def external_configuration_candidates(self):
        self.environment["CARGO_HOME"] = str(self.root / "cargo-home")
        for parent in (self.root / ".cargo", self.root / "cargo-home"):
            parent.mkdir(parents=True, exist_ok=True)
            for name in ("config", "config.toml"):
                yield parent / name

    def assert_configuration_change_rejected_after_success(self, effect):
        artifacts = self.checkout / "artifacts/agent-runs"
        before = set(artifacts.glob("kani-*"))
        previous_proofs = len(self.proof_calls())
        self.fake.write_driver(proof_body=effect + "\n" + f"print({VALID_OUTPUT!r}, end='')\n")
        with self.assertRaisesRegex(verifier.KaniError, "(?i)configuration|approved"):
            self.verify()
        fresh = set(artifacts.glob("kani-*")) - before
        self.assertEqual(len(fresh), 1)
        artifact = fresh.pop()
        self.assertEqual((artifact / "raw.log").read_text(encoding="utf-8"), VALID_OUTPUT)
        metadata = json.loads((artifact / "metadata.json").read_text(encoding="utf-8"))
        self.assertEqual(metadata["status"], "FAIL")
        self.assertNotEqual(
            metadata["approved_configuration_sha256"], metadata["configuration_after_sha256"]
        )
        self.assertEqual(metadata["approved_configuration"], json.loads(self.args.approved_config))
        self.assertEqual(len(self.proof_calls()), previous_proofs + 1)
        self.assertEqual(self.lock.read_bytes(), self.original_lock)
        return metadata

    def retained(self):
        paths = list((self.checkout / "artifacts/agent-runs").glob("kani-*"))
        self.assertEqual(len(paths), 1, "one invocation must retain one fresh artifact directory")
        raw = paths[0] / "raw.log"
        metadata = paths[0] / "metadata.json"
        self.assertTrue(raw.is_file())
        self.assertTrue(metadata.is_file())
        record = json.loads(metadata.read_text(encoding="utf-8"))
        return raw.read_text(encoding="utf-8"), record

    def test_one_fixed_direct_driver_proof_retains_logs_metadata_and_lock_identity(self):
        result = self.verify()
        self.assertIsInstance(result, dict)
        self.assertEqual(self.lock.read_bytes(), self.original_lock)
        raw, metadata = self.retained()
        self.assertEqual(raw, VALID_OUTPUT)
        serialized = json.dumps(metadata)
        self.assertIn(HARNESS, serialized)
        self.assertIn(hashlib.sha256(self.original_lock).hexdigest(), serialized)
        proofs = self.proof_calls()
        self.assertEqual(len(proofs), 1)
        command = proofs[0]["argv"]
        self.assertEqual(command[command.index("--harness") + 1], HARNESS)
        self.assertEqual(command[command.index("--unwind") + 1], "2")
        self.assertEqual(command[command.index("--output-format") + 1], "regular")
        self.assertFalse(any(call["program"] == "wrapper" for call in self.fake.read_calls()))
        for forbidden in (
            "--no-unwinding-checks", "--no-overflow-checks", "--cbmc-args", "--quiet"
        ):
            self.assertNotIn(forbidden, command)

    def test_relative_kani_home_verify_resolves_the_bundle_before_changing_cwd(self):
        self.environment["KANI_HOME"] = os.path.relpath(self.fake.bundle.parent, Path.cwd())
        result = self.verify()
        self.assertEqual(result["status"], "PASS")
        self.assertEqual(result["tools"]["bundle"], str(self.fake.bundle.resolve()))
        self.assertEqual(len(self.proof_calls()), 1)
        self.assertTrue(Path(result["command"][0]).is_absolute())
        self.assertEqual(self.retained()[0], VALID_OUTPUT)

    def test_missing_approved_configuration_is_refused_before_any_probe(self):
        del self.args.approved_config
        self.assert_configuration_refused_before_execution()

    def test_malformed_approved_configuration_is_refused_before_any_probe(self):
        for value in (None, "", "not-json", "[]", "null", "{}", '{"relative": null}'):
            with self.subTest(approved=value):
                self.args.approved_config = value
                self.assert_configuration_refused_before_execution()

    def test_missing_candidate_or_malformed_digest_is_refused_before_any_probe(self):
        approved = json.loads(self.args.approved_config)
        manifest = str(self.checkout / "Cargo.toml")
        missing = dict(approved)
        del missing[manifest]
        candidates = [missing]
        candidates.extend({**approved, manifest: digest} for digest in (1, "g" * 64, "a" * 63))
        for candidate in candidates:
            with self.subTest(candidate=candidate):
                self.args.approved_config = json.dumps(candidate)
                self.assert_configuration_refused_before_execution()

    def test_persistent_manifest_kani_weakening_after_wav_approval_never_reaches_driver(self):
        self.args.harness = (
            "contexts::media::domain::wav_inspection::chunks::kani_proofs::"
            "wav_payload_range_is_checked"
        )
        self.args.source = "services/cantos-api/src/contexts/media/domain/wav_inspection/chunks.rs"
        self.args.unwind = 2
        self.args.cover_count = 7
        self.args.allow_unreachable = []
        wav_source = self.checkout / self.args.source
        wav_source.parent.mkdir(parents=True)
        wav_source.write_text("// Disposable WAV proof input.\n", encoding="utf-8")
        self.approve_configuration()
        manifest = self.checkout / "Cargo.toml"
        weakened = (
            "[workspace]\nmembers = []\n"
            "[workspace.metadata.kani.flags]\nno-overflow-checks = true\n"
        )
        manifest.write_text(weakened, encoding="utf-8")
        self.assert_configuration_refused_before_execution()
        self.assertEqual(manifest.read_text(encoding="utf-8"), weakened)

    def test_oversized_configuration_after_approval_never_reaches_probe(self):
        (self.checkout / "Cargo.toml").write_bytes(b' ' * (1024 * 1024 + 1))
        self.assert_configuration_refused_before_execution()

    def test_oversized_configuration_after_success_cannot_retain_pass(self):
        path = self.checkout / "Cargo.toml"
        self.fake.write_driver(proof_body=(
            f"pathlib.Path({str(path)!r}).write_bytes(b' ' * (1024 * 1024 + 1))\n"
            f"print({VALID_OUTPUT!r}, end='')\n"
        ))
        with self.assertRaisesRegex(verifier.KaniError, "(?i)configuration"):
            self.verify()
        raw, metadata = self.retained()
        self.assertEqual(raw, VALID_OUTPUT)
        self.assertEqual(metadata["status"], "FAIL")
        self.assertNotIn("model_inputs_after_sha256", metadata)
        self.assertEqual(self.lock.read_bytes(), self.original_lock)

    def test_new_ancestor_and_cargo_home_candidates_after_approval_fail_before_probe(self):
        for path in self.external_configuration_candidates():
            with self.subTest(created=path):
                self.assertFalse(path.exists())
                self.approve_configuration()
                path.write_text("[build]\njobs = 1\n", encoding="utf-8")
                try:
                    self.assert_configuration_refused_before_execution()
                    self.assertTrue(path.is_file())
                finally:
                    path.unlink()

    def test_deleted_ancestor_and_cargo_home_candidates_after_approval_fail_before_probe(self):
        for path in self.external_configuration_candidates():
            with self.subTest(deleted=path):
                path.write_text("[build]\njobs = 2\n", encoding="utf-8")
                self.approve_configuration()
                path.unlink()
                self.assert_configuration_refused_before_execution()
                self.assertFalse(path.exists())

    def test_configuration_edit_during_version_probe_never_reaches_driver_proof(self):
        path = self.checkout / ".cargo/config.toml"
        changed = "[build]\njobs = 1\n"
        self.fake.write_driver(
            version_body=f"pathlib.Path({str(path)!r}).write_text({changed!r})"
        )
        with self.assertRaisesRegex(verifier.KaniError, "(?i)configuration|approved"):
            self.verify()
        self.assertIn(
            {"program": "driver", "argv": ["kani", "--version"]}, self.fake.read_calls()
        )
        self.assertEqual(self.proof_calls(), [])
        self.assertEqual(path.read_text(encoding="utf-8"), changed)
        self.assertEqual(self.lock.read_bytes(), self.original_lock)

    def test_configuration_edit_after_model_snapshot_never_reaches_driver_proof(self):
        path = self.checkout / ".cargo/config.toml"
        original_source_identity = verifier.source_tree_identity

        def edit_after_snapshot(root):
            identity = original_source_identity(root)
            path.write_text("[build]\njobs = 1\n", encoding="utf-8")
            return identity

        with patch.object(verifier, "source_tree_identity", side_effect=edit_after_snapshot):
            with self.assertRaisesRegex(verifier.KaniError, "(?i)configuration|approved"):
                self.verify()
        self.assertIn(
            {"program": "driver", "argv": ["kani", "--version"]}, self.fake.read_calls()
        )
        self.assertEqual(self.proof_calls(), [])
        artifacts = list((self.checkout / "artifacts/agent-runs").glob("kani-*"))
        self.assertEqual(len(artifacts), 1)
        self.assertFalse((artifacts[0] / "raw.log").exists())
        metadata = json.loads((artifacts[0] / "metadata.json").read_text(encoding="utf-8"))
        self.assertEqual(metadata["status"], "FAIL")
        self.assertNotEqual(
            metadata["approved_configuration_sha256"], metadata["configuration_after_sha256"]
        )
        self.assertEqual(self.lock.read_bytes(), self.original_lock)

    def test_new_ancestor_and_cargo_home_candidates_after_success_cannot_retain_pass(self):
        for path in self.external_configuration_candidates():
            with self.subTest(created=path):
                self.assertFalse(path.exists())
                self.approve_configuration()
                try:
                    metadata = self.assert_configuration_change_rejected_after_success(
                        f"pathlib.Path({str(path)!r}).write_text('[build]\\njobs = 1\\n')"
                    )
                    self.assertIsNone(metadata["approved_configuration"][str(path)])
                    self.assertTrue(path.is_file())
                finally:
                    path.unlink(missing_ok=True)

    def test_deleted_ancestor_and_cargo_home_candidates_after_success_cannot_retain_pass(self):
        for path in self.external_configuration_candidates():
            with self.subTest(deleted=path):
                original = b"[build]\njobs = 2\n"
                path.write_bytes(original)
                self.approve_configuration()
                metadata = self.assert_configuration_change_rejected_after_success(
                    f"pathlib.Path({str(path)!r}).unlink()"
                )
                self.assertEqual(
                    metadata["approved_configuration"][str(path)], hashlib.sha256(original).hexdigest()
                )
                self.assertFalse(path.exists())

    def test_failed_process_is_not_accepted_despite_complete_success_output(self):
        self.fake.write_driver(proof_body=f"print({VALID_OUTPUT!r}, end='')\nraise SystemExit(7)\n")
        with self.assertRaises(verifier.KaniError):
            self.verify()
        raw, metadata = self.retained()
        self.assertEqual(raw, VALID_OUTPUT)
        self.assertIn("7", json.dumps(metadata))
        self.assertEqual(self.lock.read_bytes(), self.original_lock)

    def test_invalid_verifier_output_is_retained_but_never_accepted(self):
        invalid = VALID_OUTPUT.replace("Status: SUCCESS", "Status: FAILURE", 1)
        self.fake.write_driver(proof_body=f"print({invalid!r}, end='')\n")
        with self.assertRaises(verifier.KaniError):
            self.verify()
        raw, _ = self.retained()
        self.assertEqual(raw, invalid)
        self.assertEqual(self.lock.read_bytes(), self.original_lock)

    def test_lock_change_is_rejected_even_after_an_otherwise_successful_proof(self):
        changed = "# changed by fake verifier\nversion = 4\n"
        self.fake.write_driver(
            proof_body=(
                f"pathlib.Path({str(self.lock)!r}).write_text({changed!r})\n"
                f"print({VALID_OUTPUT!r}, end='')\n"
            )
        )
        with self.assertRaisesRegex(verifier.KaniError, "(?i)lock"):
            self.verify()
        raw, metadata = self.retained()
        self.assertEqual(raw, VALID_OUTPUT)
        serialized = json.dumps(metadata)
        self.assertIn(hashlib.sha256(self.original_lock).hexdigest(), serialized)
        self.assertIn(hashlib.sha256(changed.encode("utf-8")).hexdigest(), serialized)
        # Detection must not silently rewrite the verifier's changed lock.
        self.assertEqual(self.lock.read_text(encoding="utf-8"), changed)

    def test_lock_change_is_also_checked_when_the_driver_fails(self):
        self.fake.write_driver(
            proof_body=(
                f"pathlib.Path({str(self.lock)!r}).write_text('changed on failure\\n')\n"
                "raise SystemExit(7)\n"
            )
        )
        with self.assertRaisesRegex(verifier.KaniError, "(?i)lock"):
            self.verify()
        _, metadata = self.retained()
        self.assertIn(hashlib.sha256(self.original_lock).hexdigest(), json.dumps(metadata))
        self.assertIn(hashlib.sha256(self.lock.read_bytes()).hexdigest(), json.dumps(metadata))

    def test_source_and_configuration_change_are_rejected_even_after_success(self):
        for relative in (self.source, Path(".cargo/config.toml")):
            with self.subTest(changed=str(relative)):
                path = self.checkout / relative
                self.fake.write_driver(
                    proof_body=(
                        f"pathlib.Path({str(path)!r}).write_text('changed by fake verifier\\n')\n"
                        f"print({VALID_OUTPUT!r}, end='')\n"
                    )
                )
                with self.assertRaisesRegex(verifier.KaniError, "(?i)source|configuration"):
                    self.verify()
                self.assertEqual(self.lock.read_bytes(), self.original_lock)
        records = list((self.checkout / "artifacts/agent-runs").glob("kani-*/metadata.json"))
        self.assertEqual(len(records), 2)
        for record in records:
            self.assertEqual(json.loads(record.read_text(encoding="utf-8"))["status"], "FAIL")

    def assert_model_change_rejected(self, effect):
        source_before = (self.checkout / self.source).read_bytes()
        self.fake.write_driver(
            proof_body=effect + "\n" + f"print({VALID_OUTPUT!r}, end='')\n"
        )
        with self.assertRaisesRegex(verifier.KaniError, "(?i)source|configuration|model|input"):
            self.verify()
        raw, metadata = self.retained()
        self.assertEqual(raw, VALID_OUTPUT)
        self.assertEqual(metadata["status"], "FAIL")
        self.assertRegex(metadata["model_inputs_sha256"], "^[0-9a-f]{64}$")
        self.assertNotEqual(
            metadata["model_inputs_sha256"], metadata["model_inputs_after_sha256"]
        )
        self.assertEqual((self.checkout / self.source).read_bytes(), source_before)
        self.assertEqual(self.lock.read_bytes(), self.original_lock)
        return metadata

    def test_transitive_api_source_mutation_is_rejected_after_successful_output(self):
        path = self.checkout / self.transitive_source
        original_digest = hashlib.sha256(path.read_bytes()).hexdigest()
        metadata = self.assert_model_change_rejected(
            f"pathlib.Path({str(path)!r}).write_text('changed transitive API source\\n')"
        )
        self.assertEqual(metadata["model_inputs"][str(self.transitive_source)], original_digest)
        self.assertEqual(path.read_text(encoding="utf-8"), "changed transitive API source\n")

    def test_new_transitive_api_source_is_rejected_after_successful_output(self):
        path = self.checkout / "services/cantos-api/src/new_transitive_input.rs"
        self.assertFalse(path.exists())
        self.assert_model_change_rejected(
            f"pathlib.Path({str(path)!r}).write_text('new transitive API source\\n')"
        )
        self.assertEqual(path.read_text(encoding="utf-8"), "new transitive API source\n")

    def test_deleted_transitive_api_source_is_rejected_after_successful_output(self):
        path = self.checkout / self.transitive_source
        original_digest = hashlib.sha256(path.read_bytes()).hexdigest()
        metadata = self.assert_model_change_rejected(f"pathlib.Path({str(path)!r}).unlink()")
        self.assertEqual(metadata["model_inputs"][str(self.transitive_source)], original_digest)
        self.assertFalse(path.exists())

    def test_contracts_source_mutation_is_rejected_after_successful_output(self):
        path = self.checkout / self.contract_source
        original_digest = hashlib.sha256(path.read_bytes()).hexdigest()
        metadata = self.assert_model_change_rejected(
            f"pathlib.Path({str(path)!r}).write_text('changed contracts source\\n')"
        )
        self.assertEqual(metadata["model_inputs"][str(self.contract_source)], original_digest)
        self.assertEqual(path.read_text(encoding="utf-8"), "changed contracts source\n")

    def test_timeout_retains_evidence_and_cannot_be_reported_as_proof_success(self):
        self.fake.write_driver(
            proof_body="import time\nprint('partial verifier output', flush=True)\ntime.sleep(30)\n"
        )
        with patch.object(verifier, "TIMEOUT_SECONDS", 0.3):
            with self.assertRaisesRegex(verifier.KaniError, "(?i)time"):
                self.verify()
        raw, metadata = self.retained()
        self.assertIn("partial verifier output", raw)
        self.assertIn(hashlib.sha256(self.original_lock).hexdigest(), json.dumps(metadata))
        self.assertEqual(self.lock.read_bytes(), self.original_lock)

    def test_output_limit_retains_bounded_evidence_and_lock_identity(self):
        self.fake.write_driver(proof_body="print('x' * 100000, flush=True)\n")
        with patch.object(verifier, "MAX_LOG_BYTES", 1024):
            with self.assertRaisesRegex(verifier.KaniError, "(?i)output|limit|cap"):
                self.verify()
        raw, metadata = self.retained()
        self.assertLessEqual(len(raw.encode("utf-8")), 1024)
        self.assertIn(hashlib.sha256(self.original_lock).hexdigest(), json.dumps(metadata))
        self.assertEqual(self.lock.read_bytes(), self.original_lock)


if __name__ == "__main__":
    unittest.main()
