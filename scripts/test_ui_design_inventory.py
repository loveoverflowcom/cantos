"""Design proposals remain metadata, never API registrations or admission evidence."""

import copy
import unittest

import ui_preview_catalog as catalog
from ui_tokens import SOURCE, load_source


# Golden IDs transcribed from the independently supplied design manifest (raw source hash
# is recorded in the inventory), rather than calculated from the validator's implementation.
PROPOSED_IDS = frozenset("""
breadcrumbs navigation-rail bottom-navigation search-bar segmented-buttons overflow-menu
cursor-pagination list-row tooltip snackbar bottom-sheet command-search date-time-field
file-picker disclosure work-cover-card work-hero episode-list listening-progress mini-player
expanded-player listening-queue playback-speed sleep-timer volume-control download-item
offline-storage transcript-viewer bookmark-item save-action membership-entry genre-filters
offline-notice progress-conflict search-results continue-listening collection-list
content-unavailable media-skeleton next-episode chapter-navigation studio-workspace
studio-overview studio-work-table studio-work-form studio-unit-list studio-content-composer
studio-revisions studio-manuscript-import studio-script-validation studio-brief-builder
studio-brief-freshness studio-audio-prepare studio-audio-transfer studio-submission-status
studio-named-review studio-private-preview studio-artwork studio-publication-readiness
studio-rights-decision studio-publication-confirm access-kinds access-delegation-form
access-scope-matrix access-grants access-once-secret access-revoke access-producer-handoff
access-admin-capability access-admin-revoke
""".split())


class DesignInventory(unittest.TestCase):
    def setUp(self):
        self.source = load_source(catalog.CATALOG.read_bytes())
        self.tokens = load_source(SOURCE.read_bytes())
        self.inventory = load_source(catalog.DESIGN_INVENTORY.read_bytes())

    def validate(self):
        return catalog.validate_design_inventory(self.inventory, self.source, self.tokens)

    def test_exact_baseline_and_complete_design_mapping_remain_separate(self):
        self.validate()
        entries = self.inventory["specimens"]
        existing = [entry for entry in entries if entry["design_status"] == "existing"]
        proposed = [entry for entry in entries if entry["design_status"] == "proposed"]
        baseline = self.inventory["baseline"]["catalog_snapshot"]
        self.assertEqual(len(existing), 32)
        self.assertEqual(len(proposed), 70)
        self.assertEqual({entry["id"] for entry in proposed}, PROPOSED_IDS)
        self.assertEqual(sum(len(entry["states"]) for entry in baseline["components"]), 198)
        self.assertEqual({entry["id"] for entry in existing}, {entry["id"] for entry in baseline["components"]})
        self.assertFalse({entry["id"] for entry in proposed} & {entry["id"] for entry in self.source["components"]})
        self.assertEqual(self.source["baseline_consumers"], baseline["baseline_consumers"])

    def test_baseline_state_or_api_removal_is_rejected(self):
        for mutation, expected in (
            (lambda s: s["components"][0]["states"].pop(), "baseline states lost: ui-root"),
            (lambda s: s["components"][0].update(name="RenamedRoot"), "baseline API changed: ui-root"),
            (lambda s: s["components"].pop(0), "baseline API missing: ui-root"),
            (lambda s: s["baseline_consumers"].append(copy.deepcopy(s["baseline_consumers"][0])), "compatibility sites changed"),
        ):
            with self.subTest(expected=expected):
                self.setUp()
                mutation(self.source)
                with self.assertRaisesRegex(ValueError, expected):
                    self.validate()

    def test_inventory_refuses_missing_owners_forged_api_links_and_token_roles(self):
        for mutation, expected in (
            (lambda e: e.update(owner=""), "nonempty single-line"),
            (lambda e: e.update(consumer=""), "nonempty single-line"),
            (lambda e: e.update(component_ids=["future-empty-api"]), "unknown runtime component"),
            (lambda e: e.update(token_roles=["color.futurePalette"]), "unknown canonical token"),
            (lambda e: e.update(states=[]), "bounded nonempty"),
            (lambda e: e.update(anatomy=[]), "bounded nonempty"),
            (lambda e: e.update(design_status="qualified"), "design status is not runtime qualification"),
            (lambda e: e.update(scope="public-api"), "unknown ownership scope"),
        ):
            with self.subTest(expected=expected):
                self.setUp()
                mutation(self.inventory["specimens"][-1])
                with self.assertRaisesRegex(ValueError, expected):
                    self.validate()

    def test_studio_and_admin_design_inventory_cannot_authorize_mobile(self):
        self.inventory["specimens"][-1]["platforms"].append("cmp")
        with self.assertRaisesRegex(ValueError, "Studio and Admin proposals are Web only"):
            self.validate()

    def test_incomplete_or_duplicate_design_inventory_is_rejected(self):
        self.inventory["specimens"].pop()
        with self.assertRaisesRegex(ValueError, "102 design specimens"):
            self.validate()
        self.setUp()
        self.inventory["specimens"][-1]["id"] = "button"
        with self.assertRaisesRegex(ValueError, "duplicate design id"):
            self.validate()

    def test_proposal_generation_has_no_effect_on_public_registry_or_qualification(self):
        original = copy.deepcopy(self.source)
        with_inventory = catalog.rust_render(self.source, self.tokens, self.inventory)
        without_inventory = catalog.rust_render(self.source, self.tokens)
        registry = lambda s: s.split("pub const COMPONENTS:", 1)[1].split("pub struct TokenRegistration", 1)[0]
        self.assertEqual(registry(with_inventory), registry(without_inventory))
        self.assertIn("pub const DESIGN_SPECIMENS:", with_inventory)
        self.assertIn('id: "access-admin-revoke"', with_inventory)
        self.assertIn('design_status: "proposed"', with_inventory)
        self.assertIn("pub const DESIGN_SOURCE_SHA256:", with_inventory)
        self.assertEqual(self.source, original)
        self.assertEqual(with_inventory, catalog.rust_render(self.source, self.tokens, copy.deepcopy(self.inventory)))


if __name__ == "__main__":
    unittest.main()
