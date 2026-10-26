"""Production schema controls; actual generated Rust compilation is a separate integration test."""
from copy import deepcopy
import json
from pathlib import Path
import tempfile
import unittest
import localization_resources as resources
from localization_catalog import CatalogError, load

class ResourceTests(unittest.TestCase):
    def setUp(self):
        self.source = load(resources.ROOT / 'packages/localization/catalog/tests/third-locale.json')
    def rejects(self, source):
        with self.assertRaises(CatalogError): resources.validate(source)
    def test_actual_projection_and_order_are_deterministic(self):
        actual=resources.validate(load(resources.ROOT / resources.SOURCE))
        self.assertEqual(resources.repository_failures(), [])
        self.assertEqual(resources.projections(actual), resources.projections(json.loads(json.dumps(actual,sort_keys=True))))
    def test_third_locale_exposure_is_resource_driven(self):
        source=resources.validate(self.source)
        normal=next(iter(resources.projections(source).values()))
        test=next(iter(resources.projections(source,include_test=True).values()))
        self.assertNotIn('Japanese',normal)
        self.assertIn('Self::Japanese].into_iter()',test)
        self.assertIn('日本語',test)
        self.assertIn('言語',test)
    def test_missing_translation_argument_or_plural_form_fails(self):
        for mutate in [lambda s:s['messages']['common.probe.language']['translations'].pop('en'),
                       lambda s:s['messages']['common.probe.bookmark']['args'].pop('time'),
                       lambda s:s['messages']['common.probe.episodes']['translations']['en']['forms'].pop('one')]:
            source=deepcopy(self.source);mutate(source);self.rejects(source)
    def test_unknown_fields_blank_and_selector_drift_fail(self):
        for pattern in [{'text':''},{'text':' '},{'text':'{unknown}'},{'unknown':'word'},{'text':'{count} episodes'}]:
            source=deepcopy(self.source);source['messages']['common.probe.episodes']['translations']['en']=pattern;self.rejects(source)
    def test_malformed_types_fail_with_catalog_diagnostics(self):
        for value in [None, [], {}, True, 2]:
            for section,field in [('locale','dir'),('locale','cardinalCategories'),('message','exposure'),('message','args')]:
                source=deepcopy(self.source)
                item=source['locales'][0] if section=='locale' else source['messages']['common.probe.language']

                if field=='args' and value=={}: continue
                item[field]=value;self.rejects(source)
    def test_generated_consumer_collisions_fail(self):
        self.source['messages']['studio.probe.language']=deepcopy(self.source['messages']['common.probe.language']);self.rejects(self.source)
    def test_extra_argument_type_and_test_fallback_fail(self):
        for kind in [[],None,{},'float']:
            source=deepcopy(self.source);source['messages']['common.probe.bookmark']['args']['title']=kind;self.rejects(source)
        self.source['fallback']='ja';self.rejects(self.source)
    def test_count_only_catalog_does_not_emit_an_unused_lifetime(self):
        self.source['messages'] = {'common.probe.episodes': self.source['messages']['common.probe.episodes']}
        output=next(iter(resources.projections(resources.validate(self.source)).values()))
        self.assertIn('pub enum Message {',output)
        self.assertNotIn("Message<'a>",output)
    def test_reserved_symbols_and_unbound_generated_owners_fail(self):
        import check_localization
        source=deepcopy(self.source);source['locales'][0]['symbol']='Self';self.rejects(source)
        source=deepcopy(self.source);source['messages']['common.type.label']=source['messages'].pop('common.probe.language');self.rejects(source)
        policy={'owners':[{'path':'apps/cantos-studio/web/src/view.rs','mode':'generated','reason':'test'}],'exceptions':[]}
        self.assertTrue(any('no drift guard' in error for error in check_localization.validate_policy(policy)))
    def test_whole_catalog_drift_guard_rejects_missing_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);source=root/resources.SOURCE;source.parent.mkdir(parents=True);source.write_text(json.dumps(self.source))
            fixture=root/'packages/localization/catalog/tests/third-locale.json';fixture.parent.mkdir(parents=True);fixture.write_text(json.dumps(self.source))
            self.assertTrue(any('generated drift' in item for item in resources.repository_failures(root)))

if __name__=='__main__': unittest.main()
