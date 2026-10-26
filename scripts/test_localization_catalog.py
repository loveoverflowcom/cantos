"""Independent fixture oracles and negative controls; no application runtime claims."""
from copy import deepcopy
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import check_localization
import localization_catalog as catalog


class CatalogTest(unittest.TestCase):
    def setUp(self):
        self.source = catalog.load(catalog.ROOT / catalog.FIXTURE / "catalog.json")

    def refuses(self, code):
        with self.assertRaisesRegex(catalog.CatalogError, code):
            catalog.validate(self.source)

    def test_actual_catalog_and_checked_projections_have_no_drift(self):
        catalog.validate(self.source)
        self.assertEqual(catalog.repository_failures(), [])

    def test_hand_authored_cross_renderer_vectors(self):
        vectors = catalog.load(catalog.ROOT / catalog.FIXTURE / "vectors.json")
        self.assertGreaterEqual(len(vectors), 22)
        for row in vectors:
            with self.subTest(row=row):
                self.assertEqual(catalog.render(self.source, row["locale"], row["key"], row["args"],
                                                row.get("includeTest", False)), row["expected"])

    def test_generation_is_independent_of_object_order(self):
        reordered = json.loads(json.dumps(self.source, sort_keys=True))
        self.assertEqual(catalog.projections(self.source), catalog.projections(reordered))

    def test_unknown_fields_and_versions_fail_closed(self):
        self.source["version"] = True
        self.refuses("catalog version")
        self.source["version"] = 2
        self.refuses("catalog version")
        self.source["version"] = 1
        self.source["runtime"] = "unreviewed"
        self.refuses("catalog: fields")

    def test_duplicate_json_keys_are_not_silently_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicate.json"
            path.write_text('{"a":1,"a":2}')
            with self.assertRaisesRegex(catalog.CatalogError, "duplicate key: a"):
                catalog.load(path)

    def test_missing_locale_or_key_and_blank_translation_are_rejected(self):
        original = deepcopy(self.source)
        del self.source["messages"]["common.cancel"]["translations"]["en"]
        self.refuses("locale/key coverage")
        self.source = original
        self.source["messages"]["common.cancel"]["translations"]["en"]["text"] = " "
        self.refuses("blank translation")

    def test_semantic_names_and_generated_symbol_collisions_are_checked(self):
        self.source["messages"]["foreign.cancel"] = self.source["messages"].pop("common.cancel")
        self.refuses("invalid semantic key")
        self.source["messages"].pop("foreign.cancel")
        message = deepcopy(self.source["messages"]["common.error.unavailable"])
        self.source["messages"]["common.error_unavailable"] = message
        self.refuses("generated symbol collision")

    def test_named_arguments_and_multiplicity_are_exact(self):
        message = self.source["messages"]["theatre.player.bookmark.add"]
        message["translations"]["en"]["text"] = "{title} {unknown}"
        self.refuses("argument coverage")
        message["translations"]["en"]["text"] = "{title} {time} {title}"
        self.refuses("placeholder multiplicity")

    def test_plural_coverage_and_selector_drift_are_rejected(self):
        message = self.source["messages"]["theatre.library.episode_count"]
        del message["translations"]["en"]["forms"]["one"]
        self.refuses("plural coverage")
        message["translations"]["en"] = {"text": "{count} episodes"}
        self.refuses("selector drift")

    def test_select_coverage_and_type_are_rejected(self):
        message = self.source["messages"]["studio.publication.state"]
        message["translations"]["en"]["forms"]["new"] = "New"
        self.refuses("select coverage")
        del message["translations"]["en"]["forms"]["new"]
        message["args"]["state"] = "text"
        self.refuses("select requires closed enum")

    def test_validated_select_values_cannot_generate_invalid_or_colliding_symbols(self):
        message = self.source["messages"]["studio.publication.state"]
        for values in (["self"], ["state_1", "state1"]):
            message["args"]["state"] = values
            self.refuses("generated enum collision")

    def test_enum_arguments_require_select_and_distinct_generated_type_names(self):
        message = self.source["messages"]["studio.publication.state"]
        original = deepcopy(self.source)
        for tag in message["translations"]:
            message["translations"][tag] = {"text": "{state}"}
        self.refuses("enum requires select")
        self.source = original
        message = self.source["messages"].pop("studio.publication.state")
        self.source["messages"]["common.foo"] = deepcopy(message)
        self.source["messages"]["common.foo_state"] = deepcopy(message)
        for key, arg in (("common.foo", "state_state"), ("common.foo_state", "state")):
            item = self.source["messages"][key]
            item["args"] = {arg: ["draft", "ready"]}
            for pattern in item["translations"].values():
                pattern["select"] = arg
        self.refuses("generated enum type collision")

    def test_locale_metadata_must_be_unambiguous_and_fallback_supported(self):
        self.source["locales"][1]["aliases"] = ["vi"]
        self.refuses("ambiguous UI alias")
        self.source["locales"][1]["aliases"] = []
        self.source["fallback"] = "ja"
        self.refuses("fallback must be a supported non-test UI locale")

    def test_malformed_locale_and_integer_rule_are_rejected(self):
        self.source["locales"][0]["tag"] = "vi_vn"
        self.refuses("noncanonical UI tag")
        self.source["locales"][0]["tag"] = "vi-VN"
        self.source["locales"][1]["integerOne"] = [True]
        self.refuses("invalid integer plural rule")

    def test_regional_lookup_alias_case_and_extension_truncation_are_explicit(self):
        for requested, expected, exact in [("en-GB", "en", False), ("EN", "en", True),
                                           ("vi", "vi-VN", False), ("vi-VN", "vi-VN", True),
                                           ("en-GB-u-ca-gregory", "en", False)]:
            self.assertEqual(catalog.resolve(self.source, requested),
                             catalog.Resolution(requested, expected, exact))
        for requested in ("", "en--GB", "en_GB"):
            with self.assertRaisesRegex(catalog.CatalogError, "invalid UI locale request"):
                catalog.resolve(self.source, requested)

    def test_unsupported_ui_locale_never_returns_content_language_or_silent_default(self):
        for requested in ("zh-Hant", "fr", "ja"):
            with self.assertRaisesRegex(catalog.CatalogError, "unsupported UI locale: .*explicit fallback: vi-VN"):
                catalog.resolve(self.source, requested)
        self.assertEqual(catalog.resolve(self.source, "ja-JP", include_test=True).tag, "ja")

    def test_additional_locale_needs_only_metadata_and_translations(self):
        # A fourth fixture language exercises generic generation, not an app switch arm.
        extra = dict(tag="ko", autonym="한국어", aliases=[], testOnly=True, integerOne=[])
        self.source["locales"].append(extra)
        for message in self.source["messages"].values():
            message["translations"]["ko"] = deepcopy(message["translations"]["ja"])
        catalog.validate(self.source)
        self.assertEqual(catalog.resolve(self.source, "ko-KR", include_test=True).tag, "ko")
        outputs = catalog.projections(self.source)
        self.assertIn('Locale::L3 => "ko"', outputs["catalog.rs"])
        self.assertIn('Locale.L3 -> "ko"', outputs["Catalog.kt"])

    def test_missing_key_and_missing_extra_or_wrong_typed_arguments_are_visible(self):
        with self.assertRaisesRegex(catalog.CatalogError, "missing message: common.missing"):
            catalog.render(self.source, "en", "common.missing", {})
        for args in ({}, {"count": 1, "extra": "no"}):
            with self.assertRaisesRegex(catalog.CatalogError, "named arguments mismatch"):
                catalog.render(self.source, "en", "theatre.library.episode_count", args)
        for count in ("1", True, -1, 1.5, 2**64):
            with self.assertRaisesRegex(catalog.CatalogError, "argument type: count"):
                catalog.render(self.source, "en", "theatre.library.episode_count", {"count": count})
        with self.assertRaisesRegex(catalog.CatalogError, "argument type: state"):
            catalog.render(self.source, "en", "studio.publication.state", {"state": "unknown"})

    def test_original_template_is_scanned_once_and_argument_order_is_irrelevant(self):
        args = {"title": 'Lời hẹn {time} <b> & " %1$s', "time": "12:05"}
        expected = 'At 12:05, add bookmark for Lời hẹn {time} <b> & " %1$s'
        for ordered in (args, dict(reversed(list(args.items())))):
            self.assertEqual(catalog.render(self.source, "en", "theatre.player.bookmark.add", ordered), expected)
        self.assertEqual(catalog.segments("{{time}} {title} {title}"),
                         [("text", "{"), ("text", "time}"), ("text", " "), ("arg", "title"),
                          ("text", " "), ("arg", "title")])

    def test_malformed_or_executable_placeholder_syntax_is_rejected(self):
        for template in ("{title", "{title.upper}", "{title[0]}", "{title!r}", "{count:02}"):
            with self.subTest(template=template), self.assertRaisesRegex(catalog.CatalogError, "invalid placeholder"):
                catalog.segments(template)

    def test_source_escaping_cannot_emit_code_or_rescan_inserted_unicode(self):
        text = 'Đe\u0300n 😀 " \\ $name\n\t\b\f {{time}}'
        self.assertIn('\\$', catalog.quoted(text, True))
        self.assertIn('\\u{8}', catalog.quoted(text))
        args = {"title": text, "time": "12:05"}
        self.assertEqual(catalog.render(self.source, "en", "theatre.player.bookmark.add", args),
                         'At 12:05, add bookmark for ' + text)

    def test_whole_fixture_validation_detects_drift_missing_and_extra_generated_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shutil.copytree(catalog.ROOT / catalog.FIXTURE, root / catalog.FIXTURE)
            self.assertEqual(catalog.repository_failures(root), [])
            generated = root / catalog.FIXTURE / "generated"
            (generated / "catalog.rs").write_text("edited")
            self.assertIn("localization fixture: generated drift: catalog.rs", catalog.repository_failures(root))
            (generated / "unexpected.kt").write_text("extra")
            self.assertIn("localization fixture: generated inventory drift", catalog.repository_failures(root))
            (generated / "Catalog.kt").unlink()
            self.assertIn("localization fixture: generated drift: Catalog.kt", catalog.repository_failures(root))

    def test_existing_localization_gate_includes_whole_catalog_guard(self):
        with patch.object(check_localization, "changed_sources", return_value={}), \
                patch.object(catalog, "repository_failures", return_value=["catalog negative control"]):
            self.assertEqual(check_localization.repository_failures(), ["catalog negative control"])

    def test_hand_edited_expected_output_fails_the_whole_catalog_check(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shutil.copytree(catalog.ROOT / catalog.FIXTURE, root / catalog.FIXTURE)
            path = root / catalog.FIXTURE / "vectors.json"
            vectors = catalog.load(path)
            vectors[0]["expected"] = "wrong"
            path.write_text(json.dumps(vectors))
            self.assertEqual(catalog.repository_failures(root),
                             ["localization fixture: vector output: common.cancel/vi-VN"])

    def test_api_content_language_schema_remains_independent_of_ui_registry(self):
        path = catalog.ROOT / "services/cantos-server/contracts/theatre/discovery/v1/discovery.schema.json"
        schema = catalog.load(path)
        languages = [value["properties"]["language"] for value in schema["$defs"].values()
                     if isinstance(value, dict) and "language" in value.get("properties", {})]
        self.assertGreaterEqual(len(languages), 3)
        for language in languages:
            self.assertEqual(language["type"], "string")
            self.assertNotIn("enum", language)
        content = {"title": "Lời hẹn {time}", "language": "zh-Hant"}
        original = dict(content)
        for locale in ("vi-VN", "en"):
            catalog.render(self.source, locale, "theatre.player.bookmark.add", {"title": content["title"], "time": "12:05"})
        self.assertEqual(content, original)


def native_harness(kotlin):
    source = catalog.validate(catalog.load(catalog.ROOT / catalog.FIXTURE / "catalog.json"))
    vectors = catalog.load(catalog.ROOT / catalog.FIXTURE / "vectors.json")
    lines = ["import cantos.localization.fixture.*", "fun main() {"] if kotlin else ["mod catalog;", "use catalog::*;", "fn main() {"]
    for row in vectors:
        resolved = catalog.resolve(source, row["locale"], row.get("includeTest", False))
        n = next(n for n, item in enumerate(source["locales"]) if item["tag"] == resolved.tag)
        name = catalog.symbol(row["key"])
        fields = []
        for arg, kind in sorted(source["messages"][row["key"]]["args"].items()):
            value = row["args"][arg]
            if isinstance(kind, list):
                value = name + catalog.symbol(arg) + ("." if kotlin else "::") + catalog.symbol(value)
            elif kind == "count":
                value = str(value) + ("uL" if kotlin else "")
            else:
                value = catalog.quoted(value, kotlin)
                if not kotlin:
                    value += ".to_string()"
            fields.append(f"arg_{arg} {'=' if kotlin else ':'} {value}")
        if kotlin:
            message = "Message." + name + ("(" + ", ".join(fields) + ")" if fields else "")
            lines.append(f"    check(render(Locale.L{n}, {message}) == {catalog.quoted(row['expected'], True)})")
        else:
            message = "Message::" + name + (" { " + ", ".join(fields) + " }" if fields else "")
            lines.append(f"    assert_eq!(render(Locale::L{n}, &{message}), {catalog.quoted(row['expected'])});")
    lines.append('    check(tag(Locale.L1) == "en")' if kotlin else '    assert_eq!(tag(Locale::L1), "en");')
    lines.append("}")
    return "\n".join(lines)


class NativeProjectionTest(unittest.TestCase):
    @unittest.skipUnless(shutil.which("rustc"), "Rust compiler unavailable")
    def test_rust_projection_executes_independent_vectors_and_rejects_wrong_argument_type(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            shutil.copy(catalog.ROOT / catalog.FIXTURE / "generated/catalog.rs", root / "catalog.rs")
            (root / "main.rs").write_text(native_harness(False), encoding="utf-8")
            command = ["rustc", "--edition=2021", "-D", "warnings", str(root / "main.rs"), "-o", str(root / "fixture")]
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            subprocess.run([str(root / "fixture")], check=True)
            source = (root / "main.rs").read_text().replace("arg_count : 1", 'arg_count : "1"')
            (root / "main.rs").write_text(source)
            refused = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("mismatched types", refused.stderr)

    @unittest.skipUnless(os.environ.get("CANTOS_KOTLIN_COMPILER_CP") and os.environ.get("CANTOS_KOTLIN_STDLIB"),
                         "optional pinned Kotlin 2.2.20 compiler classpath not supplied")
    def test_kotlin_projection_executes_same_vectors_and_rejects_wrong_argument_type(self):
        compiler_cp = os.environ["CANTOS_KOTLIN_COMPILER_CP"]
        stdlib = os.environ["CANTOS_KOTLIN_STDLIB"]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Main.kt").write_text(native_harness(True), encoding="utf-8")
            command = ["java", "-cp", compiler_cp, "org.jetbrains.kotlin.cli.jvm.K2JVMCompiler",
                       "-no-stdlib", "-no-reflect", "-Werror", "-classpath", stdlib, "-d", str(root / "fixture.jar"),
                       str(catalog.ROOT / catalog.FIXTURE / "generated/Catalog.kt"), str(root / "Main.kt")]
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            subprocess.run(["java", "-cp", str(root / "fixture.jar") + os.pathsep + stdlib, "MainKt"], check=True)
            (root / "Main.kt").write_text(native_harness(True).replace("arg_count = 1uL", 'arg_count = "1"'))
            refused = subprocess.run(command, capture_output=True, text=True)
            self.assertNotEqual(refused.returncode, 0)
            self.assertIn("argument type mismatch", refused.stderr.lower())


if __name__ == "__main__":
    unittest.main()
