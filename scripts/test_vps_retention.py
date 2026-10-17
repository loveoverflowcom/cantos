"""Exercise real release-retention decisions with an inert Podman boundary.

Temporary incoming trees are real local files. No container, service or remote host runs.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = (ROOT / "infra/vps/bin/cantos-vps").read_text().removesuffix('\nmain "$@"\n')
CURRENT, PREVIOUS, TARGET, LEGACY = (character * 40 for character in "abcd")
ATTEMPT = "1" * 32


def metadata(target=TARGET, previous=LEGACY, attempt=None):
    values = {
        "target": target, "previous": previous,
        "oracle": "cantos_e1_oracle_fixture", "rehearsal": "cantos_e1_restore_fixture",
        "staging": "cantos_e2_stage_fixture", "retained": "cantos_e1_retained_fixture",
        "old_oid": "100", "old_limit": "-1", "new_oid": "200",
    }
    if attempt is not None:
        values["attempt"] = attempt
    return "".join(f"{key}={value}\n" for key, value in values.items())


class VpsRetentionTests(unittest.TestCase):
    def run_retention(self, journals=(), *, unreadable=False, extra_entries=False, foreign_images=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "state").mkdir()
            (root / "state/current").write_text(CURRENT + "\n")
            (root / "state/previous").write_text(PREVIOUS + "\n")
            # Retained revisions are oldest. The newest three and the current/previous
            # bundles must survive along with every old/new cutover dependency.
            uploads = ["9" * 40, TARGET, LEGACY, "e" * 40, "f" * 40, "0" * 40,
                       CURRENT, PREVIOUS]
            for number, revision in enumerate(uploads):
                path = root / "incoming" / revision
                (path / "tree").mkdir(parents=True)
                (path / ATTEMPT / "tree").mkdir(parents=True)
                (path / "tree/recovery-sentinel").write_text("preserve")
                (path / ATTEMPT / "tree/recovery-sentinel").write_text("preserve")
                os.utime(path, (1_000_000 + number, 1_000_000 + number))
            for name, contents, phase in journals:
                path = root / "state/epoch2-cutovers" / name
                path.mkdir(parents=True)
                if contents is not None:
                    (path / "meta").write_text(contents)
                if phase is not None:
                    (path / "phase").write_text(phase + "\n")
                if unreadable and contents is not None:
                    (path / "meta").chmod(0)
            if extra_entries:
                (root / "incoming/operator-notes").mkdir()
                (root / "incoming" / ("8" * 40)).symlink_to(root / "incoming/operator-notes", target_is_directory=True)
            body = r'''
podman() {
  if [[ $1 == images ]]; then printf '%s\n' "$TEST_IMAGES";
  elif [[ $1 == image && $2 == inspect ]]; then
    local tag=${!#}; tag=${tag##*:}
    printf '%s\n' "$tag" >>"$ROOT/inspected-images"
    if [[ $tag == ${TEST_FOREIGN_IMAGE:-} ]]; then printf 'foreign|%s|elsewhere\n' "$tag";
    elif [[ $tag == ${TEST_MISSING_IMAGE_LABEL:-} ]]; then return 1;
    else printf 'cantos-api|%s|https://github.com/loveoverflowcom/cantos\n' "$tag"; fi
  elif [[ $1 == rmi ]]; then printf '%s\n' "$2" >>"$ROOT/deleted-images";
  else exit 99; fi
}
prune_releases
'''
            env = dict(os.environ, CANTOS_VPS_ROOT=directory,
                       TEST_IMAGES="\n".join([CURRENT, PREVIOUS, TARGET, LEGACY, "e" * 40]))
            if foreign_images:
                env.update(TEST_FOREIGN_IMAGE=TARGET, TEST_MISSING_IMAGE_LABEL=LEGACY)
            result = subprocess.run(["bash"], input=SCRIPT + body, text=True,
                                    capture_output=True, env=env, check=False)
            deleted = ((root / "deleted-images").read_text().splitlines()
                       if (root / "deleted-images").exists() else [])
            surviving = {path.name for path in (root / "incoming").iterdir()}
            inspected = ((root / "inspected-images").read_text().splitlines()
                         if (root / "inspected-images").exists() else [])
            # Reset fixture-only permissions before TemporaryDirectory cleanup.
            for path in (root / "state").rglob("meta"):
                path.chmod(0o600)
            return result, deleted, surviving, inspected

    def test_older_protocol_journal_preserves_both_images_and_complete_incoming_trees(self):
        name = TARGET[:12] + "-123-456"
        result, deleted, surviving, inspected = self.run_retention([(name, metadata(), "complete")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(deleted, ["localhost/cantos-api:" + "e" * 40])
        self.assertIn(TARGET, surviving)
        self.assertIn(LEGACY, surviving)
        self.assertNotIn("9" * 40, surviving)
        self.assertNotIn("e" * 40, surviving)
        self.assertEqual(inspected, ["e" * 40])

    def test_attempt_protocol_journal_preserves_nested_attempt_bundle_dependencies(self):
        name = TARGET[:12] + "-" + ATTEMPT
        result, deleted, surviving, _ = self.run_retention([(name, metadata(attempt=ATTEMPT), "rolled-back")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("localhost/cantos-api:" + TARGET, deleted)
        self.assertNotIn("localhost/cantos-api:" + LEGACY, deleted)
        self.assertTrue({TARGET, LEGACY}.issubset(surviving))

    def test_later_invalid_journal_prevents_pruning_after_valid_metadata_was_collected(self):
        valid = (TARGET[:12] + "-123-456", metadata(), "complete")
        invalid = ("f" * 12 + "-123-456", "target=invalid\n", "complete")
        result, deleted, surviving, inspected = self.run_retention([valid, invalid])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(deleted, [])
        self.assertIn("9" * 40, surviving)
        self.assertIn("skipping all release pruning", result.stderr)
        self.assertEqual(inspected, [])

    def test_any_invalid_journal_skips_all_image_and_incoming_deletion(self):
        name = TARGET[:12] + "-123-456"
        cases = (
            (name, None, "complete"),
            (name, metadata(), None),
            (name, metadata(), "unknown-phase"),
            (name, metadata().replace("target=" + TARGET, "target=../foreign"), "complete"),
            (name, metadata() + "target=" + TARGET + "\n", "complete"),
            (name, metadata() + "unexpected=value\n", "complete"),
            (name, metadata() + "=value\n", "complete"),
            (name, metadata().replace("old_oid=100\n", ""), "complete"),
            ("foreign-123-456", metadata(), "complete"),
            (TARGET[:12] + "-" + ATTEMPT, metadata(attempt="2" * 32), "complete"),
        )
        for journal in cases:
            with self.subTest(journal=journal[0], metadata=journal[1]):
                result, deleted, surviving, inspected = self.run_retention([journal])
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertIn("skipping all release pruning", result.stderr)
                self.assertEqual(deleted, [])
                self.assertTrue({"9" * 40, TARGET, LEGACY, "e" * 40}.issubset(surviving))
                self.assertEqual(inspected, [])

    @unittest.skipIf(os.geteuid() == 0, "root can read mode000 metadata")
    def test_unreadable_journal_keeps_all_recovery_and_normal_artifacts(self):
        name = TARGET[:12] + "-123-456"
        result, deleted, surviving, _ = self.run_retention([(name, metadata(), "complete")], unreadable=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(deleted, [])
        self.assertIn("9" * 40, surviving)

    def test_without_journals_normal_image_and_three_upload_window_pruning_remains(self):
        result, deleted, surviving, _ = self.run_retention(extra_entries=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(deleted, ["localhost/cantos-api:" + value for value in (TARGET, LEGACY, "e" * 40)])
        self.assertTrue({CURRENT, PREVIOUS, "0" * 40}.issubset(surviving))
        self.assertFalse({"9" * 40, TARGET, LEGACY, "e" * 40, "f" * 40} & surviving)
        self.assertIn("operator-notes", surviving)
        self.assertIn("8" * 40, surviving)

    def test_foreign_or_unreadable_image_labels_never_authorize_image_deletion(self):
        result, deleted, _, inspected = self.run_retention(foreign_images=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(inspected, [TARGET, LEGACY, "e" * 40])
        self.assertEqual(deleted, ["localhost/cantos-api:" + "e" * 40])


if __name__ == "__main__":
    unittest.main()
