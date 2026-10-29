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

    def test_public_composable_modifiers_cannot_hide_unregistered_native_apis(self):
        for declaration in (
            'inline fun NewControl(content: @Composable () -> Unit) { content() }',
            'public inline fun NewControl(content: @Composable () -> Unit) { content() }',
            'inline public fun <T : List<String>> NewControl(value: T) {}',
        ):
            with self.subTest(declaration=declaration):
                self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/NewControl.kt", "@Composable\n" + declaration)
                self.write("apps/example/mobile/View.kt", "import com.cantos.ui.NewControl\n")
                self.assertEqual(self.failures(), ["CMP component inventory mismatch: missing [], undeclared ['NewControl']"])
        for declaration in (
            'private inline fun NewControl() {}',
            'internal inline fun NewControl() {}',
            'inline fun NewControl() {}',
        ):
            with self.subTest(non_api=declaration):
                annotation = "" if declaration.startswith("inline fun") else "@Composable\n"
                self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/NewControl.kt", annotation + declaration)
                self.write("apps/example/mobile/View.kt", "// no shared components\n")
                self.assertEqual(self.failures(), [])

    def test_composable_annotation_names_cannot_hide_unregistered_native_apis(self):
        for source in (
            'import androidx.compose.runtime.Composable as UiComponent\n@UiComponent fun NewControl() {}',
            '@androidx.compose.runtime.Composable fun NewControl() {}',
            '@`androidx` . `compose` . `runtime` . `Composable` fun NewControl() {}',
            'import androidx.compose.runtime.Composable as UiComponent\ntypealias UiAlias = UiComponent\n@UiAlias fun NewControl() {}',
            'import androidx.compose.runtime.Composable as UiComponent\ntypealias UiAlias = OtherAlias\ntypealias OtherAlias = UiComponent\n@UiAlias fun NewControl() {}',
            '@Composable() fun NewControl() {}',
            'import androidx.compose.runtime.Composable as UiComponent\n@UiComponent() fun NewControl() {}',
        ):
            with self.subTest(source=source):
                self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/NewControl.kt", source)
                self.write("apps/example/mobile/View.kt", "import com.cantos.ui.NewControl\n")
                self.assertEqual(self.failures(), ["CMP component inventory mismatch: missing [], undeclared ['NewControl']"])
        for source in (
            'import androidx.compose.runtime.Composable as UiComponent\n@UiComponent private fun NewControl() {}',
            '/* @androidx.compose.runtime.Composable fun NewControl() {} */',
            'val note = "@androidx.compose.runtime.Composable fun NewControl() {}"',
        ):
            with self.subTest(non_api=source):
                self.write("packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/NewControl.kt", source)
                self.write("apps/example/mobile/View.kt", "// no shared components\n")
                self.assertEqual(self.failures(), [])

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

    def test_source_directories_and_same_named_files_are_not_digest_exemptions(self):
        for renderer, path in (
            ("leptos", "packages/ui-system/leptos/src/build/control.rs"),
            ("leptos", "packages/ui-system/leptos/src/nested/preview_catalog.rs"),
            ("cmp", "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/build/Control.kt"),
            ("cmp", "packages/ui-system/cmp/src/desktopMain/kotlin/com/cantos/ui/PreviewCatalog.kt"),
            ("cmp", "packages/ui-system/cmp/gradle/libs.versions.toml"),
            ("cmp", "packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/build/Control.kt"),
            ("cmp", "packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/.gradle/Control.kt"),
            ("cmp", "packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/.idea/Control.kt"),
            ("cmp", "packages/ui-system/cmp/android-preview/src/androidMain/kotlin/com/cantos/ui/.kotlin/Control.kt"),
            ("cmp", "packages/ui-system/cmp/android-preview/nested/build/Control.kt"),
            ("cmp", "packages/ui-system/cmp/other-project/build/Control.kt"),
            ("cmp", "packages/ui-system/cmp/android-preview/src/local.properties"),
        ):
            with self.subTest(renderer=renderer, path=path):
                self.qualify()
                self.write(path, "// changed actual source input\n")
                self.assertIn(f"button/{renderer}: stale source or token qualification", self.failures())

    def test_repository_ancestor_named_build_does_not_hide_renderer_sources(self):
        root = self.root / "build" / "repository"
        for renderer, relative in (
            ("leptos", "packages/ui-system/leptos/src/control.rs"),
            ("cmp", "packages/ui-system/cmp/src/commonMain/kotlin/com/cantos/ui/Control.kt"),
        ):
            with self.subTest(renderer=renderer):
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("// original source\n")
                before = catalog.renderer_digest(renderer, root, self.source["components"])
                path.write_text("// changed source\n")
                self.assertNotEqual(before, catalog.renderer_digest(renderer, root, self.source["components"]))

    def test_avif_byte_changes_invalidate_both_renderer_qualifications(self):
        original = (Path(__file__).parent / "fixtures/runtime-images/red-2x3.avif").read_bytes()
        for renderer in ("leptos", "cmp"):
            with self.subTest(renderer=renderer):
                path = self.root / f"packages/ui-system/{renderer}/src/artwork.avif"
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(original)
                before = catalog.renderer_digest(renderer, self.root, self.source["components"])
                path.write_bytes(original + b"changed")
                self.assertNotEqual(before, catalog.renderer_digest(renderer, self.root, self.source["components"]))

    def test_only_exact_generated_catalogs_and_package_build_outputs_are_excluded(self):
        for renderer, paths in (
            ("leptos", (catalog.RUST_OUTPUT.relative_to(catalog.ROOT).as_posix(),)),
            ("cmp", (catalog.KOTLIN_OUTPUT.relative_to(catalog.ROOT).as_posix(),
                     "packages/ui-system/cmp/build/generated/Synthetic.kt",
                     "packages/ui-system/cmp/.gradle/cache/metadata.json")),
        ):
            with self.subTest(renderer=renderer):
                before = catalog.renderer_digest(renderer, self.root, self.source["components"])
                for path in paths:
                    self.write(path, "excluded output\n")
                self.assertEqual(before, catalog.renderer_digest(renderer, self.root, self.source["components"]))

    def test_android_preview_generated_outputs_do_not_invalidate_cmp_qualification(self):
        self.qualify()
        before = catalog.renderer_digest("cmp", self.root, self.source["components"])
        for generated_directory in ("build", ".gradle", ".idea", ".kotlin"):
            for relative in ("intermediates/AndroidManifest.xml", "metadata/dependencies.json",
                             "generated/Synthetic.kt", "reports/preview.png"):
                with self.subTest(generated_directory=generated_directory, relative=relative):
                    path = self.write(f"packages/ui-system/cmp/android-preview/{generated_directory}/{relative}",
                                      "generated output\n")
                    self.assertEqual(before, catalog.renderer_digest("cmp", self.root, self.source["components"]))
                    path.write_text("regenerated output\n")
                    self.assertEqual(before, catalog.renderer_digest("cmp", self.root, self.source["components"]))
                    self.assertEqual(self.failures(), [])

    def test_android_preview_build_and_preview_sources_invalidate_cmp_qualification(self):
        for relative in ("build.gradle.kts", "settings.gradle.kts", "gradle.properties",
                         "gradle/wrapper/gradle-wrapper.properties",
                         "src/androidMain/AndroidManifest.xml",
                         "src/androidMain/kotlin/com/cantos/ui/androidpreview/AndroidPreviews.kt"):
            with self.subTest(relative=relative):
                path = self.write(f"packages/ui-system/cmp/android-preview/{relative}", "original source\n")
                self.qualify()
                before = catalog.renderer_digest("cmp", self.root, self.source["components"])
                path.write_text("changed source\n")
                self.assertNotEqual(before, catalog.renderer_digest("cmp", self.root, self.source["components"]))
                self.assertIn("button/cmp: stale source or token qualification", self.failures())

    def test_android_preview_local_sdk_path_does_not_invalidate_cmp_qualification(self):
        self.qualify()
        before = catalog.renderer_digest("cmp", self.root, self.source["components"])
        path = self.write("packages/ui-system/cmp/android-preview/local.properties", "sdk.dir=/tmp/first-sdk\n")
        self.assertEqual(before, catalog.renderer_digest("cmp", self.root, self.source["components"]))
        path.write_text("sdk.dir=/tmp/second-sdk\n")
        self.assertEqual(before, catalog.renderer_digest("cmp", self.root, self.source["components"]))
        self.assertEqual(self.failures(), [])

    def test_shared_web_fonts_and_design_inventory_invalidate_only_leptos(self):
        for relative in (
            "packages/ui-system/shared/assets/noto-fonts/NotoSans-Regular.ttf",
            "packages/ui-system/shared/assets/noto-fonts/NotoSerif-Bold.ttf",
            "packages/ui-system/shared/assets/noto-fonts/provenance.json",
            "packages/ui-system/shared/assets/noto-fonts/README.md",
            "packages/ui-system/shared/assets/noto-fonts/OFL.txt",
            "packages/ui-system/shared/design-inventory-v3.json",
        ):
            with self.subTest(relative=relative):
                path = self.write(relative, "original rendering input\n")
                self.qualify()
                native_before = catalog.renderer_digest("cmp", self.root, self.source["components"])
                path.write_text("changed rendering input\n")
                self.assertIn("button/leptos: stale source or token qualification", self.failures())
                self.assertEqual(native_before, catalog.renderer_digest("cmp", self.root, self.source["components"]))

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

    def test_valid_alternate_import_and_reference_syntax_still_requires_qualification(self):
        cases = (
            ("leptos", "use cantos_ui :: Button;"),
            ("leptos", "use :: cantos_ui :: { r#Button as Save };"),
            ("leptos", "use r#cantos_ui::{r#Button};"),
            ("leptos", "fn example() { :: cantos_ui :: r#Button(); }"),
            ("cmp", "import com . cantos . ui . Button as Save"),
            ("cmp", "import `com`.`cantos`.`ui`.`Button` as `Save`"),
            ("cmp", "fun example() { com . cantos . ui . `Button`(onClick = {}) }"),
        )
        for renderer, content in cases:
            with self.subTest(renderer=renderer, content=content):
                self.write("apps/example/web/src/view.rs", "// no shared components\n")
                self.write("apps/example/mobile/View.kt", "// no shared components\n")
                path = "apps/example/web/src/view.rs" if renderer == "leptos" else "apps/example/mobile/View.kt"
                self.write(path, content)
                self.assertEqual(self.failures(), [f"{path}: new use requires both preview qualifications: Button"])

    def test_shared_kotlin_package_cannot_hide_application_component_adoption(self):
        for namespace in ("com.cantos.ui", "`com` . `cantos` . `ui`"):
            with self.subTest(namespace=namespace):
                self.write("apps/example/mobile/View.kt", f'package {namespace}\n@Composable fun ProductScreen() {{ Button("Lưu", {{}}) }}\n')
                self.assertEqual(self.failures(), ["apps/example/mobile/View.kt: explicit CMP component imports required for shared-package adoption checks"])
        for source in (
            'package com.cantos.ui.application\nval label = "Button"',
            'package com.cantos.uinterface\nval label = "Button"',
            'val note = "package com.cantos.ui"',
            '/* package com.cantos.ui */\nval note = "Button"',
        ):
            with self.subTest(source=source):
                self.write("apps/example/mobile/View.kt", source)
                self.assertEqual(self.failures(), [])

    def test_character_literals_cannot_swallow_later_shared_component_use(self):
        cases = (
            ("leptos", "const QUOTE: char = '\"';\nuse cantos_ui::Button;\nconst LABEL: &str = \"Cantos\";"),
            ("leptos", "const QUOTE: u8 = b'\\\"';\nuse cantos_ui::Button;\nconst LABEL: &str = \"Cantos\";"),
            ("cmp", "fun example() { val quote = '\"'; com.cantos.ui.Button(onClick = {}); val label = \"Cantos\" }"),
        )
        for renderer, content in cases:
            with self.subTest(renderer=renderer, content=content):
                self.assertEqual(catalog.imported_components(content, renderer, {"Button"}), {"Button"})
        self.assertEqual(catalog.source_text("fn borrow<'a>(value: &'a str) {}", False), "fn borrow<'a>(value: &'a str) {}")

    def test_alternate_wildcards_and_namespace_aliases_fail_closed(self):
        for renderer, content in (
            ("leptos", "use :: cantos_ui :: *;"),
            ("leptos", "use r#cantos_ui :: {self as r#ui};"),
            ("leptos", "extern crate r#cantos_ui as r#ui;"),
            ("leptos", "use cantos_ui::{{Button}};"),
            ("cmp", "import `com` . `cantos` . `ui` . *"),
        ):
            with self.subTest(renderer=renderer, content=content):
                with self.assertRaisesRegex(ValueError, "explicit"):
                    catalog.imported_components(content, renderer, {"Button"})

    def test_kotlin_templates_cannot_hide_fully_qualified_component_calls(self):
        for content in (
            'fun example() { val label = "${com.cantos.ui.Button(onClick = {})}" }',
            'fun example() { val label = "${run { val prefix = "text"; com.cantos.ui.Button(onClick = {}) }}" }',
            'fun example() { val label = """${com . cantos . ui . `Button`(onClick = {})}""" }',
        ):
            with self.subTest(content=content):
                self.write("apps/example/mobile/View.kt", content)
                self.assertEqual(self.failures(), ["apps/example/mobile/View.kt: explicit CMP component imports required for template adoption checks"])
        self.assertEqual(catalog.imported_components('import com.cantos.ui.Button\nfun example() { val label = "${Button(onClick = {})}" }', "cmp", {"Button"}), {"Button"})
        self.assertEqual(catalog.imported_components('val note = "com.cantos.ui.Button"', "cmp", {"Button"}), set())
        self.assertEqual(catalog.imported_components(r'val note = "\${com.cantos.ui.Button}"', "cmp", {"Button"}), set())

    def test_inert_namespaces_and_comments_do_not_become_template_adoption(self):
        for content in (
            'val namespace = "com.cantos.ui.Button"\nval value = "${1}"',
            '/* ${com.cantos.ui.Button(onClick = {})} */\nval value = "${1}"',
            'val value = "${run { /* nested /* comment */ com.cantos.ui.Button() */ 1 }}"',
            'val value = "${\"com.cantos.ui.Button\"}"',
        ):
            with self.subTest(content=content):
                self.write("apps/example/mobile/View.kt", content)
                self.assertEqual(self.failures(), [])

    def test_kotlin_triple_quote_closing_runs_do_not_swallow_following_code(self):
        for closing_count in (4, 5):
            prefix = 'val quoted = """x' + '"' * closing_count + '\n'
            with self.subTest(closing_count=closing_count, region="ordinary"):
                content = prefix + '@Composable fun Example() { com.cantos.ui.Button("Save", {}) }'
                self.write("apps/example/mobile/View.kt", content)
                self.assertEqual(self.failures(), ["apps/example/mobile/View.kt: new use requires both preview qualifications: Button"])
            with self.subTest(closing_count=closing_count, region="template"):
                content = prefix + 'val following = "${com.cantos.ui.Button(\"Save\", {})}"'
                self.write("apps/example/mobile/View.kt", content)
                self.assertEqual(self.failures(), ["apps/example/mobile/View.kt: explicit CMP component imports required for template adoption checks"])
            with self.subTest(closing_count=closing_count, region="inventory"):
                self.assertIn("@Composable fun Example", catalog.source_text(prefix + '@Composable fun Example() { Text("Save") }', False))

    def test_cargo_dependency_renames_cannot_hide_the_shared_crate(self):
        cases = (
            ("apps/example/web/Cargo.toml", '[dependencies]\nui = { package = "cantos-ui", path = "../../../../packages/ui-system/leptos" }\n'),
            ("apps/example/web/Cargo.toml", "[dependencies.ui]\npackage = 'cantos-ui'\npath = '../../../../packages/ui-system/leptos'\n"),
            ("Cargo.toml", '[workspace.dependencies]\nui = { package = "\\u0063antos-ui", path = "packages/ui-system/leptos" }\n'),
        )
        self.write("apps/example/web/src/view.rs", "use ui::Button;\n")
        for path, content in cases:
            with self.subTest(path=path, content=content):
                for old_path in ("Cargo.toml", "apps/example/web/Cargo.toml"):
                    (self.root / old_path).unlink(missing_ok=True)
                self.write(path, content)
                self.assertEqual(self.failures(), [f"{path}: cantos-ui dependency rename hides adoption; use the canonical cantos_ui namespace"])
        self.write("Cargo.toml", "[workspace]\nmembers = []\n")
        self.write("apps/example/web/Cargo.toml", '[dependencies]\ncantos-ui = { package = "cantos-ui", path = "../../../../packages/ui-system/leptos" }\n')
        self.write("apps/example/web/src/view.rs", "use cantos_ui::Button;\n")
        self.assertEqual(self.failures(), ["apps/example/web/src/view.rs: new use requires both preview qualifications: Button"])

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
