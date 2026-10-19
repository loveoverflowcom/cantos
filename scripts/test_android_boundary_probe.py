"""Negative probes cannot claim PASS from unrelated Gradle/environment failures."""
import unittest
import android_boundary_probe as probe


class ProbeOracleTests(unittest.TestCase):
    def test_probe_observes_transitive_junit_and_never_generic_gradle_failure(self):
        output = "Forbidden production release dependencies: releaseRuntimeClasspath: junit:junit:4.13.2"
        self.assertTrue(probe.admitted_failure("release", "transitive", 1, output))
        for code, text in [(0, output), (1, "network unavailable"), (1, "Forbidden production release dependencies: kotlin-test-junit"), (1, "junit:junit:4.13.2")]:
            self.assertFalse(probe.admitted_failure("release", "transitive", code, text))

    def test_native_incompatible_sandbox_edge_is_refused_without_requiring_native_target(self):
        self.assertTrue(probe.admitted_failure("debug", "sandbox", 1, "Incomplete debug graph: preview-sandbox has no matching Android variant"))
        self.assertFalse(probe.admitted_failure("debug", "sandbox", 1, "preview-sandbox network failed"))
        for variant in ("debug", "release"):
            self.assertIn(f"{variant}Implementation", probe.probe_script(variant, "sandbox"))
            self.assertIn("kotlin-test-junit:2.2.20", probe.probe_script(variant, "transitive"))
