"""Check the disposable mutation workspace without compiling source mutations."""

from contextlib import redirect_stdout
import io
from pathlib import Path
import tomllib
import unittest
from unittest.mock import patch

import mutate_script_ir as mutations


class BaselineReached(Exception):
    """Stop after inspecting the actual copy passed to the baseline runner."""


class MutationWorkspaceTest(unittest.TestCase):
    def test_baseline_copy_contains_every_declared_workspace_manifest(self):
        manifest = tomllib.loads((mutations.ROOT / "Cargo.toml").read_text())
        members = manifest["workspace"]["members"]

        def inspect_copy(workspace, _env, *, timeout):
            self.assertEqual(timeout, 600)
            self.assertEqual((Path(workspace) / "rust-toolchain.toml").read_bytes(),
                             (mutations.ROOT / "rust-toolchain.toml").read_bytes())
            for member in members:
                with self.subTest(member=member):
                    copied = Path(workspace) / member / "Cargo.toml"
                    self.assertTrue(copied.is_file(), f"missing workspace member: {member}")
                    self.assertEqual(copied.read_bytes(),
                                     (mutations.ROOT / member / "Cargo.toml").read_bytes())
            raise BaselineReached

        with patch.object(mutations, "run", side_effect=inspect_copy):
            with redirect_stdout(io.StringIO()), self.assertRaises(BaselineReached):
                mutations.main()
