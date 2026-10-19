"""Synthetic pre-shrink boundaries; no Gradle, SDK, provider or device evidence."""
from pathlib import Path
import tempfile
import unittest
import zipfile

import inspect_android_inputs as inputs
from inspect_android_release import InspectionError


class InputBoundaryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.mobile = Path(self.temporary.name)
        sandbox = self.mobile / "preview-sandbox/src/commonMain"
        (sandbox / "composeResources/files").mkdir(parents=True)
        (sandbox / "composeResources/files/story.json").write_bytes(b'private fixture corpus')
        (sandbox / "Preview.kt").write_text("package com.cantos.theatre.app\nclass PreviewOwner")
        self.sources = self.mobile / "shared/src/commonMain"
        self.sources.mkdir(parents=True)
        (self.sources / "Real.kt").write_text("package com.cantos.theatre.app\nclass RealOwner")
        self.classes = self.mobile / "compiled/com/cantos/theatre/app"
        self.classes.mkdir(parents=True)
        (self.classes / "RealOwner.class").write_bytes(b"compiled synthetic class")

    def inspect(self, extra=(), variant="release"):
        return inputs.inspect(self.mobile, variant, [self.mobile / "compiled", *extra])

    def test_same_release_class_bytes_and_sources_have_stable_fingerprints(self):
        baseline = self.inspect()
        self.assertEqual(baseline, self.inspect())
        self.assertEqual(self.inspect()["status"], "PASS")
        (self.classes / "RealOwner.class").write_bytes(b"different pre-shrink input")
        changed = self.inspect()
        self.assertNotEqual(changed["classes_sha256"], baseline["classes_sha256"])
        self.assertEqual(changed["status"], "PASS")

    def test_transitive_test_or_sandbox_jar_fails_before_r8_in_both_variants(self):
        jar = self.mobile / "transitive.jar"
        for name in ("org/junit/Test.class", "com/cantos/theatre/app/PreviewOwner.class",
                     "com/cantos/theatre/adapters/mock/Quiet.class"):
            with zipfile.ZipFile(jar, "w") as archive:
                archive.writestr(name, b"synthetic input")
            for variant in ("debug", "release"):
                result = self.inspect([jar], variant)
                self.assertEqual(result["status"], "FAIL", (name, variant))
                self.assertTrue(result["findings"])

    def test_verbatim_fixture_resource_or_source_copied_under_innocent_name_fails(self):
        for name, content in [("assets/catalog.bin", b'private fixture corpus'),
                              ("Real.kt", b"package com.cantos.theatre.app\nclass PreviewOwner")]:
            target = self.sources / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(content)
            self.assertTrue(any("copied sandbox" in f for f in self.inspect()["findings"]))
            target.unlink()

    def test_renamed_fixture_bytes_inside_dependency_jar_fail(self):
        jar = self.mobile / "transitive-resource.jar"
        with zipfile.ZipFile(jar, "w") as archive:
            archive.writestr("assets/catalog.bin", b"private fixture corpus")
        self.assertEqual(self.inspect([jar])["status"], "FAIL")

    def test_production_safe_ui_catalog_metadata_is_not_a_sandbox_adapter(self):
        jar = self.mobile / "ui.jar"
        with zipfile.ZipFile(jar, "w") as archive:
            archive.writestr("com/cantos/ui/PreviewModelKt.class", b"pure catalog metadata")
        self.assertEqual(self.inspect([jar])["status"], "PASS")

    def test_missing_classes_and_empty_inventory_never_pass(self):
        with self.assertRaisesRegex(InspectionError, "no Cantos"):
            inputs.inspect(self.mobile, "release", [])
        (self.mobile / "preview-sandbox/src/commonMain/Preview.kt").unlink()
        with self.assertRaisesRegex(InspectionError, "empty"):
            self.inspect()
