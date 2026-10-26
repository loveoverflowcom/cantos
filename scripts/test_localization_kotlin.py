"""Generator invariants independent of Kotlin renderer and native-format acceptance."""
from copy import deepcopy
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
        self.assertIn('"" + time + "に" + title + "を聴く"', source)
        self.assertNotIn('.replace(', source)
        self.assertNotIn('else ->', source)

    def test_numeric_and_plural_requests_cannot_silently_enter_the_text_projection(self):
        with self.assertRaisesRegex(CatalogError, 'Kotlin formatter unqualified: common.probe.episodes'):
            kotlin.source(self.fixture, {"probe": "Probe"}, include_test=True)

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


if __name__ == '__main__':
    unittest.main()
