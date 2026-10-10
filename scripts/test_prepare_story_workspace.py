"""Example evidence for identity isolation and non-destructive initialization."""

import json
import tempfile
import unittest
import uuid
from pathlib import Path

from prepare_story_workspace import WorkspaceError, canonical_url, prepare_workspace


class StoryWorkspaceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "stories"

    def prepare(self, **changes):
        options = {
            "root": self.root,
            "title": "Đường Đến Bình Yên",
            "author": "Tác Giả",
            "source_url": "https://example.org/duong-den-binh-yen/",
        }
        options.update(changes)
        return prepare_workspace(**options)

    def snapshot(self):
        return {
            str(path.relative_to(self.root)): path.read_bytes()
            for path in self.root.rglob("*") if path.is_file()
        }

    def test_initialization_creates_stable_identity_and_pending_stage_reports(self):
        result = self.prepare()
        self.assertTrue(result["created"])
        story = self.root / "duong_den_binh_yen"
        self.assertEqual(result["path"], str(story))
        self.assertEqual(set(self.snapshot()), {
            "index.md", "duong_den_binh_yen/story.json", "duong_den_binh_yen/index.md",
            "duong_den_binh_yen/raw/index.md", "duong_den_binh_yen/score/index.md",
            "duong_den_binh_yen/audio/index.md", "duong_den_binh_yen/score/name_map.yml",
        })
        record = json.loads((story / "story.json").read_text(encoding="utf-8"))
        self.assertEqual(uuid.UUID(record["story_id"]).version, 4)
        self.assertEqual(record["sync"], {
            "state": "pending_sync",
            "drive_root_folder_id": "185z1tU88YvQkcS6mjCQsXhxiYwebkhqJ",
            "drive_story_folder_id": None,
        })
        self.assertTrue(result["name_map_created"])
        name_map = (story / "score" / "name_map.yml").read_text(encoding="utf-8")
        self.assertIn('story_id: "' + record["story_id"] + '"\n', name_map)
        self.assertIn("schema_version: 1\nrevision: 1\n", name_map)
        self.assertIn("inventory_status: pending\nentities: []\nunresolved: []\n", name_map)
        score_index = (story / "score" / "index.md").read_text(encoding="utf-8")
        self.assertIn("every source proper name before adaptation", score_index)
        self.assertIn("an empty map is incomplete", score_index)
        for stage in ("raw", "score", "audio"):
            report = (story / stage / "index.md").read_text(encoding="utf-8")
            self.assertIn("State: not_started", report)
            self.assertIn("Source revision / SHA-256: pending capture", report)
            self.assertIn("Drive file ID", report)
            self.assertIn("## Update log", report)
            self.assertIn("## Blockers", report)
        self.assertTrue(all(b"\r" not in content for content in self.snapshot().values()))

    def test_rerun_reuses_source_identity_and_preserves_all_existing_bytes(self):
        first = self.prepare()
        story = Path(first["path"])
        (story / "raw" / "chapter_001.txt").write_text("Nguồn gốc.\n", encoding="utf-8")
        (story / "score" / "index.md").write_text("# Human review\nAccepted draft.\n", encoding="utf-8")
        (story / "score" / "name_map.yml").write_text(
            "schema_version: 1\nrevision: 2\ninventory_status: in_progress\n"
            "entities:\n  - original: Bạch Tiểu Thuần\n    replacement: An Vân\n",
            encoding="utf-8",
        )
        (self.root / "index.md").write_text("# Manually maintained catalog\n", encoding="utf-8")
        before = self.snapshot()
        second = self.prepare(source_url="HTTPS://EXAMPLE.ORG:443/duong-den-binh-yen/?utm_source=test#list")
        self.assertFalse(second["created"])
        self.assertFalse(second["name_map_created"])
        self.assertEqual(first["story"]["story_id"], second["story"]["story_id"])
        self.assertEqual(self.snapshot(), before)

    def test_source_normalization_preserves_content_query_encoding_and_order(self):
        self.assertEqual(
            canonical_url("HTTP://Example.org:80/book?id=12&name=a%20b&tag=x&tag=y&utm_medium=mail#top"),
            "http://example.org/book?id=12&name=a%20b&tag=x&tag=y",
        )
        self.assertNotEqual(
            canonical_url("https://example.org/book?id=12"),
            canonical_url("https://example.org/book?id=13"),
        )
        self.assertEqual(canonical_url("https://example.org:8443"), "https://example.org:8443/")

    def test_same_title_from_different_source_never_merges(self):
        self.prepare()
        before = self.snapshot()
        with self.assertRaisesRegex(WorkspaceError, "ambiguous folder collision"):
            self.prepare(source_url="https://other.example/book/")
        self.assertEqual(self.snapshot(), before)

    def test_accent_folding_collision_never_merges(self):
        self.prepare()
        with self.assertRaisesRegex(WorkspaceError, "ambiguous folder collision"):
            self.prepare(title="Duong Den Binh Yen", source_url="https://example.org/another/")

    def test_existing_source_rejects_contradictory_title_and_author(self):
        self.prepare()
        before = self.snapshot()
        for changes in ({"title": "Different work"}, {"author": "Different author"}):
            with self.subTest(changes=changes):
                with self.assertRaisesRegex(WorkspaceError, "different title or author"):
                    self.prepare(**changes)
        self.assertEqual(self.snapshot(), before)

    def test_explicit_id_reuses_persisted_identity_but_cannot_change_source(self):
        first = self.prepare()
        story_id = first["story"]["story_id"]
        self.assertFalse(self.prepare(story_id=story_id)["created"])
        with self.assertRaisesRegex(WorkspaceError, "different source URL"):
            self.prepare(story_id=story_id, source_url="https://example.org/new/")
        with self.assertRaisesRegex(WorkspaceError, "different story_id"):
            self.prepare(story_id=str(uuid.uuid4()))

    def test_unknown_explicit_id_does_not_seed_a_new_story(self):
        with self.assertRaisesRegex(WorkspaceError, "story_id was not found"):
            self.prepare(story_id=str(uuid.uuid4()))
        self.assertFalse(self.root.exists())

    def test_malformed_metadata_is_preserved_and_blocks_initialization(self):
        first = self.prepare()
        metadata = Path(first["path"]) / "story.json"
        for malformed in ("{broken", '{"schema_version":1,"schema_version":2}', "[]"):
            with self.subTest(malformed=malformed):
                metadata.write_text(malformed, encoding="utf-8")
                before = self.snapshot()
                with self.assertRaisesRegex(WorkspaceError, "invalid metadata"):
                    self.prepare()
                self.assertEqual(self.snapshot(), before)

    def test_renamed_directory_with_mismatched_metadata_requires_review(self):
        first = self.prepare()
        Path(first["path"]).rename(self.root / "renamed")
        with self.assertRaisesRegex(WorkspaceError, "folder_name does not match"):
            self.prepare()

    def test_duplicate_story_id_requires_reconciliation_without_writes(self):
        first = self.prepare()
        second = self.prepare(title="Một Truyện Khác", source_url="https://example.org/another/")
        metadata = Path(second["path"]) / "story.json"
        record = second["story"]
        record["story_id"] = first["story"]["story_id"]
        metadata.write_text(json.dumps(record), encoding="utf-8")
        before = self.snapshot()
        with self.assertRaisesRegex(WorkspaceError, "duplicate story IDs or source URLs"):
            self.prepare()
        self.assertEqual(self.snapshot(), before)

    def test_synced_metadata_without_remote_identity_is_rejected(self):
        first = self.prepare()
        metadata = Path(first["path"]) / "story.json"
        record = first["story"]
        record["sync"]["state"] = "synced"
        metadata.write_text(json.dumps(record), encoding="utf-8")
        before = self.snapshot()
        with self.assertRaisesRegex(WorkspaceError, "synced metadata lacks"):
            self.prepare()
        self.assertEqual(self.snapshot(), before)

    def test_sync_conflict_identity_is_preserved_on_rerun(self):
        first = self.prepare()
        metadata = Path(first["path"]) / "story.json"
        record = first["story"]
        record["sync"]["state"] = "sync_conflict"
        record["sync"]["drive_story_folder_id"] = "existing_drive_folder"
        metadata.write_text(json.dumps(record), encoding="utf-8")
        before = self.snapshot()
        repeated = self.prepare()
        self.assertFalse(repeated["created"])
        self.assertEqual(repeated["story"]["story_id"], first["story"]["story_id"])
        self.assertEqual(repeated["story"]["sync"]["state"], "sync_conflict")
        self.assertEqual(self.snapshot(), before)

    def test_obsolete_conflict_state_is_rejected_without_overwriting_metadata(self):
        first = self.prepare()
        metadata = Path(first["path"]) / "story.json"
        record = first["story"]
        record["sync"]["state"] = "conflict"
        metadata.write_text(json.dumps(record), encoding="utf-8")
        before = self.snapshot()
        with self.assertRaisesRegex(WorkspaceError, "unknown sync state"):
            self.prepare()
        self.assertEqual(self.snapshot(), before)

    def test_missing_index_on_existing_story_never_claims_an_empty_stage(self):
        first = self.prepare()
        stage = Path(first["path"]) / "raw"
        (stage / "index.md").unlink()
        artifact = stage / "chapter_001.txt"
        artifact.write_text("Already captured.\n", encoding="utf-8")
        self.prepare()
        report = (stage / "index.md").read_text(encoding="utf-8")
        self.assertIn("State: needs_review", report)
        self.assertIn("existing artifacts require inventory review", report)
        self.assertEqual(artifact.read_text(encoding="utf-8"), "Already captured.\n")

    def test_existing_unidentified_directory_is_never_adopted(self):
        target = self.root / "duong_den_binh_yen"
        target.mkdir(parents=True)
        (target / "notes.txt").write_text("Unidentified source", encoding="utf-8")
        before = self.snapshot()
        with self.assertRaisesRegex(WorkspaceError, "ambiguous folder collision"):
            self.prepare()
        self.assertEqual(self.snapshot(), before)

    def test_git_worktree_directory_and_linked_worktree_file_are_rejected(self):
        for marker_type in ("directory", "file"):
            with self.subTest(marker_type=marker_type):
                repo = Path(self.temp.name) / marker_type
                repo.mkdir()
                marker = repo / ".git"
                if marker_type == "directory":
                    marker.mkdir()
                    (marker / "HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
                else:
                    marker.write_text("gitdir: /some/git/worktrees/test\n", encoding="utf-8")
                with self.assertRaisesRegex(WorkspaceError, "outside Git worktrees"):
                    self.prepare(root=repo / "nested" / "data")
                self.assertFalse((repo / "nested").exists())

    def test_root_symlink_into_git_is_rejected(self):
        repo = Path(self.temp.name) / "repo"
        repo.mkdir()
        (repo / ".git").mkdir()
        (repo / ".git" / "HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
        self.root.symlink_to(repo, target_is_directory=True)
        with self.assertRaisesRegex(WorkspaceError, "outside Git worktrees"):
            self.prepare()
        self.assertFalse((repo / "duong_den_binh_yen").exists())

    def test_stage_symlink_cannot_redirect_an_index_write(self):
        result = self.prepare()
        stage = Path(result["path"]) / "raw"
        (stage / "index.md").unlink()
        stage.rmdir()
        other = Path(self.temp.name) / "unrelated"
        other.mkdir()
        stage.symlink_to(other, target_is_directory=True)
        with self.assertRaisesRegex(WorkspaceError, "refusing a symlink"):
            self.prepare()
        self.assertEqual(list(other.iterdir()), [])

    def test_index_symlink_cannot_redirect_a_write(self):
        result = self.prepare()
        index = Path(result["path"]) / "score" / "index.md"
        index.unlink()
        other = Path(self.temp.name) / "unrelated.md"
        index.symlink_to(other)
        with self.assertRaisesRegex(WorkspaceError, "refusing a symlink"):
            self.prepare()
        self.assertFalse(other.exists())

    def test_user_title_cannot_escape_the_data_root(self):
        result = self.prepare(title="../../Đêm / Khuya")
        self.assertEqual(Path(result["path"]).parent, self.root)
        self.assertEqual(Path(result["path"]).name, "dem_khuya")


if __name__ == "__main__":
    unittest.main()
