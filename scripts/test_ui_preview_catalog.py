"""Independent catalog/parity/adoption examples; no renderer or visual claim."""

import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

import ui_preview_catalog as catalog
from ui_tokens import SOURCE, load_source


def fixture():
    return {
        "version": 1,
        "components": [{"id": "button", "name": "Button", "family": "actions",
                        "states": ["enabled", "disabled", "loading", "focus"],
                        "leptos": {"status": "registered", "reason": "Runtime review pending."},
                        "cmp": {"status": "implemented-unverified", "reason": "Native runtime pending."}}],
        "baseline_consumers": [], "qualifications": [],
    }


class PreviewCatalog(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.source = fixture()
        self.tokens = load_source(SOURCE.read_bytes())
        self.write("packages/ui-system/leptos/src/button.rs", "#[component]\npub fn Button() {}\n")
        self.write("packages/ui-system/leptos/preview/main.rs", "<Button/>\n")
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/Components.kt", "@Composable\nfun Button() {}\n")
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/PreviewModel.kt", 'enum class PreviewExample(val id: String) { Button("button") }\n')
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/PreviewApp.kt", 'when (example) {\n PreviewExample.Button -> Button()\n}\n')
        self.write("apps/example/web/src/view.rs", "// no shared components\n")

    def write(self, relative, content):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)
        return path

    def failures(self):
        return catalog.repository_failures(self.source, self.tokens, self.root)

    def evidence(self, renderer):
        checks = {}
        for check in ("structural", "runtime", "accessibility", "visual"):
            path = f"artifacts/{renderer}/{check}.txt"
            self.write(path, f"{check}: actual named verifier output fixture\n")
            checks[check] = {"status": "PASS", "command": f"fixture {check}", "artifact": path}
        record = {
            "component_id": "button", "renderer": renderer, "status": "PASS",
            "reviewer": "Named reviewer fixture", "reference": "Named inspected reference fixture",
            "tokens_sha256": catalog.digest(self.tokens),
            "source_sha256": catalog.renderer_digest(renderer, self.root, self.source["components"]), "checks": checks,
        }
        path = f"docs/evidence/button-{renderer}.json"
        self.write(path, json.dumps(record))
        return path

    def qualify(self):
        paths = {renderer: self.evidence(renderer) for renderer in ("leptos", "cmp")}
        self.source["qualifications"] = [{"component_id": "button", "evidence": paths}]
        return paths

    def test_structural_registration_does_not_grant_application_use(self):
        self.assertEqual(self.failures(), [])
        self.write("apps/example/web/src/view.rs", "use cantos_ui::Button;\n")
        self.assertEqual(self.failures(), ["apps/example/web/src/view.rs: new use requires both preview qualifications: Button"])

    def test_baseline_is_exact_component_and_site_not_a_global_exemption(self):
        self.source["baseline_consumers"] = [{"renderer": "leptos", "path": "apps/example/web/src/view.rs", "components": ["Button"]}]
        self.write("apps/example/web/src/view.rs", "use cantos_ui::Button;\n")
        self.assertEqual(self.failures(), [])
        self.write("apps/example/web/src/new.rs", "use cantos_ui::{Button as Action};\n")
        self.assertEqual(self.failures(), ["apps/example/web/src/new.rs: new use requires both preview qualifications: Button"])

    def test_inventory_and_actual_sample_are_required_on_both_renderers(self):
        self.write("packages/ui-system/leptos/src/nested/new.rs", "#[component(transparent)]\n#[allow(unused)]\npub fn NewControl() {}\n")
        self.write("packages/ui-system/leptos/preview/main.rs", "// metadata only\n")
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/Components.kt", "// metadata only\n")
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/nested/NewControl.kt", "@Composable\nfun NewControl() {}\n")
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/PreviewApp.kt", "// metadata only\n")
        failures = self.failures()
        self.assertEqual(len(failures), 4)
        self.assertIn("missing ['NewControl']", failures[0])
        self.assertIn("Leptos preview omits actual Button sample", failures)
        self.assertIn("CMP preview omits button dispatch", failures)

    def test_comments_strings_and_native_metadata_are_not_real_samples(self):
        self.write("packages/ui-system/leptos/preview/main.rs", '// <Button/>\n/* outer /* nested */ <Button/> */\nlet text = r#"<Button/>"#;\n')
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/PreviewApp.kt", 'val note = "button"\n// PreviewExample.Button -> Button()\n')
        self.assertIn("Leptos preview omits actual Button sample", self.failures())
        self.assertIn("CMP preview omits button dispatch", self.failures())
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/preview/PreviewApp.kt", 'when(example) {\n PreviewExample.Button -> Text("Button()")\n}\n')
        self.assertIn("CMP preview omits button dispatch", self.failures())

    def test_both_fresh_records_allow_new_use_and_source_or_tokens_invalidate_it(self):
        self.write("apps/example/web/src/view.rs", "use cantos_ui::Button;\n")
        self.qualify()
        self.assertEqual(self.failures(), [])
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/Components.kt", "@Composable\nfun Button() { /* source changed */ }\n")
        self.assertIn("button/cmp: stale source or token qualification", self.failures())
        self.qualify()
        self.tokens["tokens"]["space.md"]["value"] += 1
        self.assertIn("button/leptos: stale source or token qualification", self.failures())

    def test_unrun_runtime_and_missing_artifacts_cannot_qualify(self):
        paths = self.qualify()
        path = self.root / paths["cmp"]
        record = json.loads(path.read_text())
        record["checks"]["runtime"]["status"] = "NOT_RUN"
        path.write_text(json.dumps(record))
        self.assertEqual(self.failures(), ["button/cmp: runtime is not PASS"])
        record["checks"]["runtime"]["status"] = "PASS"
        record["checks"]["visual"]["artifact"] = "artifacts/missing.png"
        path.write_text(json.dumps(record))
        self.assertEqual(self.failures(), ["button/cmp: missing visual artifact"])

    def test_state_contract_changes_invalidate_both_renderer_qualifications(self):
        self.qualify()
        self.assertEqual(self.failures(), [])
        self.source["components"][0]["states"].append("pressed")
        self.assertIn("button/leptos: stale source or token qualification", self.failures())
        self.qualify()
        self.assertEqual(self.failures(), [])
        self.source["components"][0]["cmp"]["reason"] = "A different platform contract requires review."
        self.assertIn("button/leptos: stale source or token qualification", self.failures())

    def test_runtime_manifest_and_lock_changes_invalidate_renderer_qualification(self):
        self.qualify()
        self.write("packages/ui-system/leptos/Cargo.toml", '[dependencies]\nleptos = "different-runtime"\n')
        self.assertIn("button/leptos: stale source or token qualification", self.failures())
        self.qualify()
        self.write("Cargo.lock", "# changed resolved dependencies\n")
        self.assertIn("button/leptos: stale source or token qualification", self.failures())
        self.qualify()
        self.write("packages/ui-system/cmp/gradle.properties", "kotlin.native.binary.memoryModel=changed\n")
        self.assertIn("button/cmp: stale source or token qualification", self.failures())

    def test_unsupported_is_declared_but_never_silently_approved(self):
        self.source["components"][0]["cmp"]["status"] = "unsupported"
        self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/Components.kt", "// Unsupported: explicit native reason\n")
        self.assertEqual(self.failures(), [])
        self.qualify()
        self.assertEqual(self.failures(), ["button/cmp: unsupported cannot be qualified"])

    def test_schema_rejects_duplicate_identity_missing_renderer_and_fake_approved_status(self):
        for mutate, expected in (
            (lambda s: s["components"].append(copy.deepcopy(s["components"][0])), "duplicate id"),
            (lambda s: s["components"][0].pop("cmp"), "expected keys"),
            (lambda s: s["components"][0]["cmp"].update(status="approved"), "registration is not qualification"),
            (lambda s: s["components"][0]["cmp"].update(reason=""), "nonempty"),
            (lambda s: s["components"][0]["states"].append("focus"), "duplicate states"),
        ):
            with self.subTest(expected=expected):
                source = fixture()
                mutate(source)
                with self.assertRaisesRegex(ValueError, expected):
                    catalog.validate(source)

    def test_qualification_paths_are_local_records_and_named_components_only(self):
        self.source["qualifications"] = [{"component_id": "button", "evidence": {"leptos": "../outside.json", "cmp": "docs/evidence/cmp.json"}}]
        with self.assertRaisesRegex(ValueError, "repository-relative"):
            catalog.validate(self.source)
        self.source["qualifications"][0]["component_id"] = "unknown"
        with self.assertRaisesRegex(ValueError, "unknown or duplicate component"):
            catalog.validate(self.source)

    def test_aliases_and_wildcards_cannot_hide_component_adoption(self):
        self.assertEqual(catalog.imported_components("use cantos_ui::{Button as Save};", "leptos", {"Button"}), {"Button"})
        self.assertEqual(catalog.imported_components("use cantos_ui::{Button as\n Save};", "leptos", {"Button"}), {"Button"})
        self.assertEqual(catalog.imported_components("use cantos_ui::{Button /* caller label */ as Save};", "leptos", {"Button"}), {"Button"})
        self.assertEqual(catalog.imported_components('// use cantos_ui::Button;\nlet note = "use cantos_ui::Button;";', "leptos", {"Button"}), set())
        self.assertEqual(catalog.imported_components("cantos_ui::Button()", "leptos", {"Button"}), {"Button"})
        self.assertEqual(catalog.imported_components("import com.cantos.ui.Button as Save", "cmp", {"Button"}), {"Button"})
        self.assertEqual(catalog.imported_components("com.cantos.ui.Button(onClick = {});", "cmp", {"Button"}), {"Button"})
        for renderer, source in (
            ("leptos", "use cantos_ui::*;"),
            ("leptos", "use cantos_ui as cui;"),
            ("leptos", "use cantos_ui::{self as cui}; cui::Button();"),
            ("leptos", "use {cantos_ui as cui}; cui::Button();"),
            ("leptos", "extern crate cantos_ui as cui; cui::Button();"),
            ("leptos", "use cantos_ui;"),
            ("cmp", "import com.cantos.ui.*"),
        ):
            with self.assertRaisesRegex(ValueError, "explicit"):
                catalog.imported_components(source, renderer, {"Button"})

    def test_evidence_and_artifact_symlinks_cannot_escape_repository(self):
        outside = Path(self.directory.name).parent / (Path(self.directory.name).name + "-outside")
        outside.mkdir()
        self.addCleanup(lambda: __import__("shutil").rmtree(outside))
        external = outside / "record.json"
        external.write_text("{}")
        link = self.root / "docs/evidence/link.json"
        link.parent.mkdir(parents=True, exist_ok=True)
        link.symlink_to(external)
        with self.assertRaisesRegex(ValueError, "symlink escapes"):
            catalog.safe_location(self.root, "docs/evidence/link.json", "evidence")

    def test_outputs_are_deterministic_and_every_named_token_has_both_values(self):
        expected = catalog.outputs(self.source, self.tokens, self.root)
        self.assertEqual(expected, catalog.outputs(copy.deepcopy(self.source), copy.deepcopy(self.tokens), self.root))
        rust = expected[self.root / catalog.RUST_OUTPUT.relative_to(catalog.ROOT)]
        kotlin = expected[self.root / catalog.KOTLIN_OUTPUT.relative_to(catalog.ROOT)]
        roles = set(self.tokens["tokens"]) | set(self.tokens["themes"]["light"]) | set(self.tokens["aliases"])
        for role in roles:
            self.assertIn(f'name: "{role}"', rust)
        self.assertIn('css_variable: "--cantos-color-primary"', rust)
        self.assertIn('alias_of: Some("shape.reading")', rust)
        self.assertIn('PreviewComponent("button", "Button", "actions"', kotlin)
        self.assertNotIn("approved", kotlin)

    def test_rust_reason_strings_preserve_valid_unicode_and_escape_quotes(self):
        self.source["components"][0]["cmp"]["reason"] = 'Chưa kiểm tra 🎭 "runtime".'
        output = catalog.rust_render(self.source, self.tokens)
        self.assertIn('Chưa kiểm tra 🎭 \\"runtime\\".', output)
        self.assertNotIn('\\u01', output)


if __name__ == "__main__":
    unittest.main()
