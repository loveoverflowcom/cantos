"""Static contract of the optimized demo/QA Android variant (#245 part C).

Bytes, signatures and R8 behavior are checked from the built APK by `cargo xtask distribute
firebase --channel demo-qa` and its installed-tool tests; this guards the source declarations that
make that variant release-like without weakening the Production unshrunk guard (#122).
"""

from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[1]
HOST = ROOT / "apps/cantos-theatre/mobile/androidApp"
BUILD = (HOST / "build.gradle.kts").read_text()
RULES = (HOST / "proguard-rules.pro").read_text()
REQUEST = (ROOT / "xtask/src/distribute/request.rs").read_text()


def build_type(name: str) -> str:
    start = BUILD.index(f'create("{name}")', BUILD.index("buildTypes {"))
    depth, index = 0, BUILD.index("{", start)
    for cursor in range(index, len(BUILD)):
        depth += {"{": 1, "}": -1}.get(BUILD[cursor], 0)
        if depth == 0:
            return BUILD[index:cursor + 1]
    raise AssertionError(name)


class TheatreAndroidQaVariantTest(unittest.TestCase):
    def test_qa_is_release_like_not_debuggable_and_shrunk(self):
        qa = build_type("qa")
        self.assertIn("isDebuggable = false", qa)
        self.assertIn("isMinifyEnabled = true", qa)
        self.assertIn("isShrinkResources = true", qa)
        self.assertIn('versionNameSuffix = "-qa"', qa)
        self.assertIn('proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")', qa)
        self.assertIn('matchingFallbacks += listOf("release")', qa)

    def test_qa_audit_is_the_unshrunk_unsigned_twin_that_cannot_be_distributed(self):
        audit = build_type("qaAudit")
        self.assertIn("isMinifyEnabled = false", audit)
        self.assertIn("isShrinkResources = false", audit)
        self.assertIn("signingConfig = null", audit)
        # Its label cannot equal the QA policy's `-demo-qa` suffix.
        self.assertIn('versionNameSuffix = "-qa-audit"', audit)

    def test_signing_key_is_operator_provided_through_the_environment_only(self):
        for name in ("CANTOS_QA_KEYSTORE", "CANTOS_QA_KEYSTORE_PASSWORD", "CANTOS_QA_KEY_ALIAS", "CANTOS_QA_KEY_PASSWORD"):
            self.assertIn(f'"{name}"', BUILD)
            self.assertIn(f'"{name}"', REQUEST)
        self.assertEqual(len(re.findall(r"signingConfigs\.create\(", BUILD)), 1)
        # Never a Gradle property (process lists), a literal secret, the debug key or a generator.
        self.assertNotIn("gradleProperty(\"cantos.qa", BUILD)
        self.assertNotIn('getByName("debug")', BUILD)
        self.assertNotRegex(BUILD, r'(storePassword|keyPassword)\s*=\s*"')
        self.assertNotIn("keytool", BUILD)
        self.assertIn('it.name == "packageQa"', BUILD)

    def test_tooling_runtime_is_debug_only_and_the_qa_graph_is_checked_before_building(self):
        self.assertEqual(re.findall(r'(\w*[iI]mplementation)\("androidx\.compose\.ui:ui-tooling:', BUILD),
                         ["debugImplementation"])
        self.assertIn("assertQaDependencyBoundary", BUILD)
        self.assertIn('"qaRuntimeClasspath"', BUILD)
        self.assertIn('it.name == "preQaBuild"', BUILD)
        # The demo host intentionally composes the mock sandbox; nothing else local is allowed.
        self.assertIn('setOf(hostPath, ":shared", ":preview-sandbox")', BUILD)

    def test_keep_rules_are_narrow_and_warnings_are_never_silenced_globally(self):
        rules = [line.strip() for line in RULES.splitlines() if line.strip() and not line.strip().startswith("#")]
        for rule in rules:
            self.assertNotRegex(rule, r"^-keep(classmembers)?\s+(public\s+)?class\s+\*\*", rule)
            self.assertNotRegex(rule, r"^-dontwarn\s+\*\*", rule)
        for forbidden in ("-ignorewarnings", "-dontshrink", "-dontoptimize", "-dontobfuscate"):
            self.assertNotIn(forbidden, rules)

    def test_policy_maps_the_qa_channel_to_this_build_type_without_touching_demo(self):
        self.assertIn('pub const DEMO_QA_VARIANT: &str = "qa";', REQUEST)
        self.assertIn('pub const DEMO_QA_GRADLE_TASK: &str = ":androidApp:assembleQa";', REQUEST)
        self.assertIn('pub const DEMO_VARIANT: &str = "debug";', REQUEST)
        self.assertIn('pub const DEMO_GRADLE_TASK: &str = ":androidApp:assembleDebug";', REQUEST)
        self.assertIn('pub const DEMO_QA_VERSION_SUFFIX: &str = "-demo-qa";', REQUEST)
        self.assertIn('applicationId = "com.cantos.theatre.demo"', BUILD)

    def test_production_host_still_guards_its_unshrunk_unsigned_release(self):
        production = (ROOT / "apps/cantos-theatre/mobile/androidProductionApp/build.gradle.kts").read_text()
        self.assertIn("isMinifyEnabled = false", production)
        self.assertIn("isShrinkResources = false", production)
        self.assertNotIn("signingConfig", production)


if __name__ == "__main__":
    unittest.main()
