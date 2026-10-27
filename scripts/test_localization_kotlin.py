"""Generator invariants independent of Kotlin renderer and native-format acceptance."""
from copy import deepcopy
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from localization_catalog import CatalogError, load
import localization_kotlin as kotlin
import localization_resources as resources


class KotlinResourcesTest(unittest.TestCase):
    def setUp(self):
        self.fixture = resources.validate(load(resources.ROOT / "packages/localization/catalog/tests/third-locale.json"))

    def test_registry_excludes_test_locales_and_saved_tags_remain_canonical(self):
        release = kotlin.locale_source(self.fixture)
        self.assertIn('Vietnamese("vi-VN", "Tiếng Việt", "ltr")', release)
        self.assertIn('val Default: UiLocale = Vietnamese', release)
        self.assertIn('firstOrNull { it.tag == tag }', release)
        self.assertNotIn('Japanese', release)
        test = kotlin.locale_source(self.fixture, include_test=True)
        self.assertIn('Japanese("ja", "日本語", "ltr")', test)

    def test_typed_text_generation_keeps_insertions_opaque_and_escapes_kotlin_dollars(self):
        fixture = deepcopy(self.fixture)
        del fixture["messages"]["common.probe.episodes"]
        fixture["messages"]["common.probe.language"]["translations"]["en"]["text"] = "Price $1 and ${opaque}"
        # An unbound template would fail the canonical validator. Here check the final language
        # quoting primitive separately; no Kotlin interpolation should execute catalog bytes.
        self.assertEqual('"Price \\$1 and \\${opaque}"', kotlin.quoted("Price $1 and ${opaque}"))
        fixture["messages"]["common.probe.language"]["translations"]["en"]["text"] = "Price $1"
        source = kotlin.source(resources.validate(fixture), {"probe": "Probe"}, include_test=True)
        self.assertIn('data class Bookmark(val time: String, val title: String)', source)
        self.assertIn('"" + this.time + "に" + this.title + "を聴く"', source)
        self.assertNotIn('.replace(', source)
        self.assertNotIn('else ->', source)

    def test_numeric_and_plural_requests_cannot_silently_enter_the_text_projection(self):
        with self.assertRaisesRegex(CatalogError, 'Kotlin formatter unqualified: common.probe.episodes'):
            kotlin.source(self.fixture, {"probe": "Probe"}, include_test=True)

    def test_plain_keys_decode_original_template_braces(self):
        fixture = deepcopy(self.fixture)
        fixture['messages'] = {'common.probe.language': fixture['messages']['common.probe.language']}
        for pattern in fixture['messages']['common.probe.language']['translations'].values():
            pattern['text'] = 'Literal {{title}} and {{time}}'
        source = kotlin.source(resources.validate(fixture), {'probe': 'Probe'}, include_test=True)
        self.assertIn('-> "Literal {title} and {time}"', source)
        self.assertNotIn('Literal {{title}}', source)

    def test_form_feed_uses_a_valid_kotlin_unicode_escape(self):
        self.assertEqual('"A\\u000cB"', kotlin.quoted('A\fB'))

    def test_named_locale_argument_cannot_be_shadowed_by_the_renderer_locale(self):
        fixture = deepcopy(self.fixture)
        fixture['messages'] = {'common.probe.echo': {
            'args': {'locale': 'text'},
            'exposure': 'production',
            'translations': {row['tag']: {'text': 'Echo {locale}'} for row in fixture['locales']},
        }}
        source = kotlin.source(resources.validate(fixture), {'probe': 'Probe'}, include_test=True)
        self.assertIn('data class Echo(val locale: String)', source)
        self.assertIn('-> "" + "Echo " + this.locale', source)

    def test_kotlin_reserved_argument_and_locale_symbols_fail_before_generation(self):
        fixture = deepcopy(self.fixture)
        fixture["locales"][0]["symbol"] = "Companion"
        with self.assertRaisesRegex(CatalogError, 'Kotlin reserved locale symbol'):
            kotlin.locale_source(fixture)
        fixture = deepcopy(self.fixture)
        fixture["messages"] = {"common.probe.bookmark": fixture["messages"]["common.probe.bookmark"]}
        fixture["messages"]["common.probe.bookmark"]["args"]["when"] = "text"
        with self.assertRaisesRegex(CatalogError, 'Kotlin reserved argument'):
            kotlin.source(fixture, {"probe": "Probe"})

    def test_real_catalog_has_all_cmp_owners_and_each_generated_file_is_drift_guarded(self):
        catalog = resources.validate(load(resources.ROOT / resources.SOURCE))
        self.assertEqual(set(kotlin.GROUPS), {key.split('.')[1] for key in catalog["messages"] if '.cmp_' in key})
        outputs = resources.expected_outputs()
        self.assertTrue(set(kotlin.projections(catalog)) <= outputs.keys())
        for path in kotlin.projections(catalog):
            self.assertEqual(outputs[path], (resources.ROOT / path).read_text())
        for path in resources.TARGETS.values():
            self.assertNotIn('pub mod cmp_', outputs[path])

    def test_cmp_namespace_cannot_be_rebound_to_another_app_or_an_unknown_group(self):
        for key in ['studio.cmp_shell.label', 'theatre.cmp_unknown.label']:
            fixture = deepcopy(self.fixture)
            fixture['messages'] = {key: fixture['messages']['common.probe.language']}
            with self.assertRaisesRegex(CatalogError, 'Kotlin unbound namespace'):
                kotlin.source(fixture)

    @unittest.skipUnless(os.environ.get('CANTOS_KOTLIN_COMPILER_CP') and os.environ.get('CANTOS_KOTLIN_STDLIB'),
                         'optional pinned Kotlin 2.2.20 compiler classpath not supplied')
    def test_generated_text_executes_braces_controls_and_shadowed_arguments(self):
        fixture = deepcopy(self.fixture)
        fixture['messages'] = {
            'common.probe.plain': {
                'args': {},
                'exposure': 'production',
                'translations': {row['tag']: {'text': 'Plain {{title}}'} for row in fixture['locales']},
            },
            'common.probe.echo': {
                'args': {'locale': 'text'},
                'exposure': 'production',
                'translations': {row['tag']: {'text': 'Echo {locale} \f {{time}}'} for row in fixture['locales']},
            },
        }
        resources.validate(fixture)
        package = 'com.cantos.theatre.localization.fixture'
        compiler_cp = os.environ['CANTOS_KOTLIN_COMPILER_CP']
        stdlib = os.environ['CANTOS_KOTLIN_STDLIB']
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'UiLocale.kt').write_text(kotlin.locale_source(fixture, package, include_test=True), encoding='utf-8')
            (root / 'Resources.kt').write_text(kotlin.source(fixture, {'probe': 'Probe'}, package, package,
                                                           include_test=True, locale_type='UiLocale'), encoding='utf-8')
            (root / 'Main.kt').write_text(r'''package com.cantos.theatre.localization.fixture
fun main() {
    for (locale in UiLocale.entries) {
        check(ProbeKey.Plain.text(locale) == "Plain {title}")
        check(ProbeMessage.Echo(locale = "opaque {time} <b> %1").text(locale) ==
            "Echo opaque {time} <b> %1 \u000c {time}")
    }
}
''', encoding='utf-8')
            command = ['java', '-cp', compiler_cp, 'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler',
                       '-no-stdlib', '-no-reflect', '-Werror', '-classpath', stdlib, '-d', str(root / 'fixture.jar'),
                       str(root / 'UiLocale.kt'), str(root / 'Resources.kt'), str(root / 'Main.kt')]
            result = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            subprocess.run(['java', '-cp', str(root / 'fixture.jar') + os.pathsep + stdlib,
                            package + '.MainKt'], check=True)


if __name__ == '__main__':
    unittest.main()
