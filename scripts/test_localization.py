#!/usr/bin/env python3
"""Focused regression oracles for the changed-file localization primitive."""
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

import check_localization as checks

WEB = "apps/cantos-theatre/web/src/features/example.rs"
CMP = "apps/cantos-theatre/mobile/shared/src/commonMain/kotlin/com/cantos/theatre/Example.kt"
OWNER = "apps/cantos-theatre/web/src/messages.rs"


class LocalizationRegressionTest(unittest.TestCase):
    def failures(self, source, path=WEB, lines=None, allowlist=None):
        return checks.check_source(path, source, lines, allowlist or checks.empty_policy())

    def test_new_view_text_and_multiline_accessible_name_are_rejected(self):
        source = 'view! { <button aria-label=\n"Retry the request">"Retry"</button> }'
        failures = self.failures(source)
        self.assertEqual(len(failures), 2, failures)
        self.assertTrue(all("displayed literal" in item for item in failures))

    def test_comparison_operators_in_child_expressions_do_not_hide_literal_copy(self):
        source = 'view! { <p>{if count < 1 { "Few items" } else { "Many items" }}</p> }'
        self.assertEqual(len(self.failures(source)), 2)
        source = 'view! { <p>{if count > 1 { "Many items" } else { "Few items" }}</p> }'
        self.assertEqual(len(self.failures(source)), 2)

    def test_nested_views_keep_technical_attributes_out_of_child_copy(self):
        source = 'view! { <div>{items.map(|item| view! { <p class="fixture">{text(Key::Title,locale)}</p> })}</div> }'
        self.assertEqual(self.failures(source), [])

    def test_view_component_label_and_formatter_copy_are_rejected(self):
        source = 'view! { <TextField label="Title"/> <p>{format!("Request failed: {code}")}</p> }'
        self.assertEqual(len(self.failures(source)), 2)

    def test_native_form_name_is_protocol_but_component_name_is_accessible(self):
        self.assertEqual(self.failures('view! { <input id="email" name="email" type="email"/> }'), [])
        self.assertEqual(self.failures('<input id="email" name="email" type="email"/>', 'apps/cantos-theatre/web/index.html'), [])
        self.assertEqual(len(self.failures('view! { <BackButton name="Go back"/> }')), 1)

    def test_unquoted_html_text_and_accessibility_attribute_are_rejected(self):
        failures = self.failures('<html><button aria-label="Retry request">Try again</button></html>',
                                 "apps/cantos-theatre/web/index.html")
        self.assertEqual(len(failures), 2, failures)

    def test_kotlin_text_and_semantics_are_rejected(self):
        source = '@Composable fun Screen() { Text(\n"Retry"); Icon(contentDescription = "Back"); semantics { contentDescription = "Error" } }'
        self.assertEqual(len(self.failures(source, CMP)), 3)

    def test_existing_cmp_wrappers_have_reviewed_label_positions(self):
        source = '@Composable fun Screen() { TextField("New label", value, onInput); Button("Save draft", {}); Select(label="Theme", options=options); Dialog(true,"Title","Close",{}); SelectOption("protocol-id","Display label") }'
        self.assertEqual(len(self.failures(source, CMP)), 6)
        source = '@Composable fun Screen() { TextField(copy.text(Key.Title), "raw content", onInput); Select(label=copy.text(Key.Theme), options=options, value="protocol-id"); SelectOption("protocol-id", copy.text(Key.Theme)) }'
        self.assertEqual(self.failures(source, CMP), [])

    def test_cmp_accessibility_constructor_of_and_tooltip_are_ui_sinks(self):
        source = '@Composable fun Screen() { val name = AccessibleName.of("Open"); val reason = DisabledReason.of("Unavailable"); Tooltip("Help") }'
        self.assertEqual(len(self.failures(source, CMP)), 3)

    def test_resource_calls_and_technical_attributes_are_accepted(self):
        source = 'view! { <button class="primary" id="retry" data-testid="retry" on:click=move |_| log::info!("retry")>{text(Key::Retry, locale)}</button> }'
        self.assertEqual(self.failures(source), [])
        source = '@Composable fun Screen() { Text(copy.text(Key.Retry)); println("diagnostic"); val request = "application/json" }'
        self.assertEqual(self.failures(source, CMP), [])

    def test_comments_raw_strings_and_unrelated_diagnostics_do_not_confuse_context(self):
        source = '// view! { <p>"comment"</p> }\nfn run() { log::error!(r#"request \"failed\""#); }\nview! { <p>{r#"Retry \"now\""#}</p> }'
        self.assertEqual(len(self.failures(source)), 1)

    def test_reviewed_fixture_exception_is_exact_and_requires_reason(self):
        source = 'view! { <p lang="vi-VN">"Ánh đèn cuối sân khấu"</p> }'
        policy = checks.empty_policy()
        policy["exceptions"] = [{"path": WEB, "literal": "Ánh đèn cuối sân khấu", "context": source,
                                 "reason": "Vietnamese typography specimen; content retains its language."}]
        self.assertEqual(self.failures(source, allowlist=policy), [])
        self.assertEqual(len(self.failures(source.replace("Ánh đèn", "Những ngọn đèn"), allowlist=policy)), 1)
        policy["exceptions"][0]["reason"] = ""
        self.assertTrue(checks.validate_policy(policy))

    def test_changed_interior_lines_of_multiline_literals_are_checked(self):
        self.assertEqual(len(self.failures('<button>\nNew visible copy\n</button>', 'apps/cantos-theatre/web/index.html', {2})), 1)
        source = 'view! { <p>{r#"Existing line\nNew visible copy\n"#}</p> }'
        self.assertEqual(len(self.failures(source, lines={2})), 1)
        source = '<button aria-label="Existing line\nnew accessible copy">{label}</button>'
        self.assertTrue(any('accessible copy' in item for item in self.failures(source, 'apps/cantos-theatre/web/index.html', {2})))

    def test_existing_boolean_en_alias_cannot_create_new_feature_map(self):
        source = 'fn text(key: Copy, en: bool) -> &str { if en { "Retry" } else { "Thử lại" } }'
        self.assertTrue(any('translation owner' in item for item in self.failures(source)))

    def test_only_added_lines_are_linted_for_displayed_copy(self):
        source = 'view! { <p>"Legacy copy"</p>\n<button>"New copy"</button> }'
        failures = self.failures(source, lines={2})
        self.assertEqual(len(failures), 1)
        self.assertIn("New copy", failures[0])

    def test_feature_translation_maps_are_rejected_outside_reviewed_owners(self):
        source = 'enum Copy { Retry }\nfn text(key: Copy, locale: Locale) -> &\'static str { match (key, locale) { (Copy::Retry, Locale::Vietnamese) => "Thử lại", (Copy::Retry, Locale::English) => "Retry" } }'
        self.assertTrue(any("translation owner" in failure for failure in self.failures(source)))
        source = 'fun label(locale: Locale) = when (locale) { Locale.English -> "Retry"; else -> "Thử lại" }'
        self.assertTrue(any("translation owner" in failure for failure in self.failures(source, CMP)))

    def test_existing_owner_is_not_permission_to_add_a_macro_in_a_feature(self):
        source = 'macro_rules! resources { ($key:ident) => {} }\nresources! { Retry => ("Thử lại", "Retry") }'
        self.assertTrue(any("translation owner" in failure for failure in self.failures(source)))

    def test_macro_resource_pair_presence_and_placeholders(self):
        source = 'resources! { Retry => ("Thử lại {title}", "Retry {name}"), Empty => ("", "Empty"), Missing => ("Chưa đủ") }'
        failures = checks.check_catalog(OWNER, source)
        self.assertEqual(len(failures), 3, failures)
        self.assertTrue(any("placeholder" in failure for failure in failures))
        self.assertTrue(any("blank" in failure for failure in failures))
        self.assertTrue(any("two" in failure for failure in failures))

    def test_named_placeholder_multiplicity_must_match(self):
        source = 'resources! { Title => ("{title} {title}", "{title}") }'
        self.assertEqual(len(checks.check_catalog(OWNER, source)), 1)

    def test_macro_resource_parameters_are_order_independent_and_literal_title_is_not_expanded(self):
        source = 'resources! { Resume => ("{title} lúc {time}", "At {time}: {title}"), Title => ("Lời hẹn {{time}}", "Lời hẹn {{time}}") }'
        self.assertEqual(checks.check_catalog(OWNER, source), [])

    def test_match_catalog_requires_both_locales_even_when_whole_file_has_them(self):
        source = 'fn text() { match (key, locale) { (Key::Retry, false) => "Thử lại", (Key::Retry, true) => "Retry", (Key::Wait, false) => "Đợi" } }'
        failures = checks.check_catalog(OWNER, source)
        self.assertEqual(len(failures), 1, failures)
        self.assertIn("Wait", failures[0])

    def test_kotlin_enum_requires_paired_nonblank_strings_and_matching_parameters(self):
        source = 'enum class CopyKey(val vi: String, val en: String) { Retry("Thử lại", "Retry"), Missing("Chưa đủ"), Bad("{title}", "{name}") }'
        failures = checks.check_catalog(CMP, source)
        self.assertEqual(len(failures), 2, failures)

    def test_legacy_struct_branch_fields_and_parameters_are_checked(self):
        source = 'if english { Copy { save: "Save {title}", retry: "Retry" } } else { Copy { save: "Lưu {name}" } }'
        self.assertEqual(len(checks.check_catalog(OWNER, source)), 2)

    def test_reverse_specimen_match_and_shelltext_keep_locale_pairs(self):
        source = 'match (locale, key) { (Locale::Vi, SpecimenKey::Save) => "Lưu", (Locale::En, SpecimenKey::Save) => "Save", (Locale::Vi, SpecimenKey::Retry) => "Thử lại" }'
        self.assertEqual(len(checks.check_catalog(OWNER, source)), 1)
        self.assertEqual(len(checks.check_catalog(CMP, 'val overrides = mapOf(Key.Retry to ShellText("Thử lại", ""))')), 1)

    def test_compose_resource_xml_requires_matching_explicit_en_keys(self):
        source = '<resources><string name="retry">Thử lại {title}</string><string name="retry_en">Retry {name}</string><string name="wait">Đợi</string></resources>'
        failures = checks.check_catalog("apps/cantos-theatre/mobile/shared/src/commonMain/composeResources/values/strings.xml", source)
        self.assertEqual(len(failures), 2, failures)

    def test_raw_error_is_rejected_at_ui_sinks_but_not_in_logs(self):
        source = 'fn log(error: Error) { log::error!("{error}"); }\nview! { <p>{error.to_string()}</p><p>{api_error.message}</p> }'
        failures = self.failures(source)
        self.assertEqual(len(failures), 2, failures)
        self.assertTrue(all("raw error" in failure for failure in failures))
        self.assertEqual(len(self.failures('@Composable fun S() { Text(apiError.toString()) }', CMP)), 1)

    def test_typed_presentation_mappings_cannot_add_catch_all(self):
        source = 'fn error_key(error: ApiError) -> Key { match error { ApiError::Network => Key::Network, _ => Key::Unknown } }'
        self.assertTrue(any("exhaustive" in failure for failure in self.failures(source)))
        source = 'fun errorKey(error: ApiError): CopyKey = when (error) { ApiError.Network -> CopyKey.Network; else -> CopyKey.Unknown }'
        self.assertTrue(any("exhaustive" in failure for failure in self.failures(source, CMP)))

    def test_nested_http_status_fallback_does_not_hide_closed_error_variants(self):
        source = 'fn error_key(error: ApiError) -> Key { match error { ApiError::Http { status } => match status { 401 => Key::SignIn, _ => Key::Unavailable }, ApiError::Network => Key::Network } }'
        self.assertEqual(self.failures(source), [])

    def test_rust_test_modules_and_integration_fixtures_are_excluded(self):
        source = '#[cfg(test)] mod tests { fn fixture() { let label = if english { "Test" } else { "Mẫu" }; view! { <p>"Fixture"</p> }; } }'
        self.assertEqual(self.failures(source), [])
        self.assertEqual(self.failures('view! { <p>"Fixture"</p> }', 'packages/ui-system/leptos/tests/fixture.rs'), [])

    def test_boolean_locale_alias_has_a_word_boundary(self):
        source = '@Composable fun Tokens() { if (entries.isEmpty()) EmptyState(copy.Empty, copy.Help) }'
        self.assertEqual(self.failures(source, CMP), [])

    def test_technical_state_matches_are_not_treated_as_error_presentation(self):
        source = 'fn decode(code: &str) -> Option<State> { match code { "ok" => Some(State::Ready), _ => None } }'
        self.assertEqual(self.failures(source), [])

    def test_branch_committed_staged_and_unstaged_lines_are_all_selected(self):
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            path = root / WEB
            path.parent.mkdir(parents=True)
            path.write_text("legacy\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            commit = ["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "-qm"]
            subprocess.run(commit + ["base"], cwd=root, check=True)
            base = checks.git(root, "rev-parse", "HEAD").strip()
            subprocess.run(["git", "update-ref", "refs/remotes/origin/develop", base], cwd=root, check=True)
            path.write_text("legacy\ncommitted\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            subprocess.run(commit + ["feature"], cwd=root, check=True)
            path.write_text("legacy\ncommitted\nstaged\n", encoding="utf-8")
            subprocess.run(["git", "add", "."], cwd=root, check=True)
            path.write_text("legacy\ncommitted\nstaged\nunstaged\n", encoding="utf-8")
            self.assertEqual(checks.default_base(root), base)
            self.assertEqual(checks.changed_sources(root)[WEB], {2, 3, 4})

    def test_untracked_source_is_included_in_changed_file_selection(self):
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-qm", "base"], cwd=root, check=True)
            path = root / WEB
            path.parent.mkdir(parents=True)
            path.write_text('view! { <button>"Retry"</button> }\n', encoding="utf-8")
            self.assertEqual(checks.changed_sources(root, "HEAD")[WEB], {1})

    def test_named_interpolation_keys_are_not_displayed_literals(self):
        self.assertEqual(self.failures('view! { <p>{messages::format(Key::Title, locale, &[("title", title)])}</p> }'), [])

    def test_resource_arm_blocks_are_checked_without_false_missing_locale(self):
        source = 'match (key, locale) { (Key::Retry, false) => "Thử lại", (Key::Retry, true) => { "Retry" } }'
        self.assertEqual(checks.check_catalog(OWNER, source), [])

    def test_test_fixtures_are_not_production_ui(self):
        self.assertEqual(self.failures('Text("Fixture")', CMP.replace('/commonMain/', '/desktopTest/')), [])

    def test_no_diff_base_fails_actionably_instead_of_silently_skipping(self):
        import subprocess
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(["git", "init", "-q"], cwd=root, check=True)
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-qm", "base"], cwd=root, check=True)
            with self.assertRaisesRegex(ValueError, "fetch-depth: 2"):
                checks.default_base(root)
            first = checks.git(root, "rev-parse", "HEAD").strip()
            subprocess.run(["git", "-c", "user.name=Test", "-c", "user.email=test@example.invalid", "commit", "--allow-empty", "-qm", "head"], cwd=root, check=True)
            with self.assertRaisesRegex(ValueError, "fetch develop history"):
                checks.default_base(root)
            with patch.dict("os.environ", {"GITHUB_ACTIONS": "true", "GITHUB_EVENT_NAME": "push", "GITHUB_REF": "refs/heads/develop"}):
                self.assertEqual(checks.default_base(root), first)

    def test_repository_checkout_retains_the_parent_for_changed_file_guard(self):
        workflow = (checks.ROOT / ".github/workflows/repository-checks.yml").read_text(encoding="utf-8")
        repository = workflow.split("  repository:", 1)[1].split("  rust:", 1)[0]
        self.assertIn("fetch-depth: 2", repository)

    def test_scopes_exclude_backend_and_studio_mobile(self):
        self.assertFalse(checks.in_scope("services/cantos-api/src/main.rs"))
        self.assertFalse(checks.in_scope("apps/cantos-studio/mobile/Example.kt"))
        self.assertTrue(checks.in_scope(WEB))
        self.assertTrue(checks.in_scope("packages/ui-system/leptos/preview/main.rs"))


if __name__ == "__main__":
    unittest.main()
