"""Release-artifact inspector over synthetic DEX/ZIP bytes and recorded aapt2 output.

These establish the parser and policy only. Actual exclusion evidence comes from running the
inspector on a built production APK/AAB through `cargo xtask theatre-host android-release`.
"""

from pathlib import Path
import argparse
import contextlib
import hashlib
import io
import os
import struct
import tempfile
import unittest
import zipfile

import inspect_android_release as inspector

APP = "com.cantos.theatre"
LAUNCHER = "com.cantos.theatre.app.TheatreActivity"
DYNAMIC = "com.cantos.theatre.DYNAMIC_RECEIVER_NOT_EXPORTED_PERMISSION"
ANDROID = "http://schemas.android.com/apk/res/android:"
PRODUCTION = [
    "Lcom/cantos/theatre/app/TheatreActivity;",
    "Lcom/cantos/theatre/app/AppContainer;",
    "Lcom/cantos/theatre/features/launch/presentation/CantosListeningUnavailableKt;",
    "Landroidx/activity/ComponentActivity;",
]
SANDBOX = "Lcom/cantos/theatre/app/PreviewOwnersKt;"

# Trimmed from real aapt2 35.0.0 output: binary APK form, then the AAB proto (Raw) form.
APK_MANIFEST = f"""N: android=http://schemas.android.com/apk/res/android (line=6)
  E: manifest (line=6)
    A: {ANDROID}versionCode(0x0101021b)=1
    A: package="{APP}" (Raw: "{APP}")
      E: uses-sdk (line=11)
        A: {ANDROID}minSdkVersion(0x0101020c)=23
      E: uses-permission (line=19)
        A: {ANDROID}name(0x01010003)="{DYNAMIC}" (Raw: "{DYNAMIC}")
      E: application (line=21)
        A: {ANDROID}allowBackup(0x01010280)=false
          E: activity (line=28)
            A: {ANDROID}name(0x01010003)="{LAUNCHER}" (Raw: "{LAUNCHER}")
            A: {ANDROID}exported(0x01010010)=true
              E: intent-filter (line=31)
                  E: action (line=32)
                    A: {ANDROID}name(0x01010003)="android.intent.action.MAIN" (Raw: "android.intent.action.MAIN")
                  E: category (line=34)
                    A: {ANDROID}name(0x01010003)="android.intent.category.LAUNCHER" (Raw: "android.intent.category.LAUNCHER")
          E: provider (line=38)
            A: {ANDROID}name(0x01010003)="org.jetbrains.compose.resources.AndroidContextProvider" (Raw: "org.jetbrains.compose.resources.AndroidContextProvider")
            A: {ANDROID}authorities(0x01010018)="{APP}.resources.AndroidContextProvider" (Raw: "{APP}.resources.AndroidContextProvider")
"""
AAB_MANIFEST = f"""N: android=http://schemas.android.com/apk/res/android (line=6)
  E: manifest (line=6)
    A: package="{APP}" (Raw: "{APP}")
    A: {ANDROID}versionCode(0x0101021b)=1 (Raw: "1")
      T: '\\n\\n    '
      E: uses-permission (line=19)
        A: {ANDROID}name(0x01010003)="{DYNAMIC}" (Raw: "{DYNAMIC}")
      E: permission (line=15)
        A: {ANDROID}protectionLevel(0x01010009)=0x00000002 (Raw: "signature")
      E: application (line=21)
        A: {ANDROID}allowBackup(0x01010280)=false (Raw: "false")
          E: activity (line=28)
            A: {ANDROID}name(0x01010003)="{LAUNCHER}" (Raw: "{LAUNCHER}")
              E: intent-filter (line=31)
                  E: category (line=34)
                    A: {ANDROID}name(0x01010003)="android.intent.category.LAUNCHER" (Raw: "android.intent.category.LAUNCHER")
"""


# The browser-callback entry that Identity v1 removed: a second, BROWSABLE VIEW filter whose
# `<data>` declares a custom scheme, on the launcher activity (aapt2 xmltree shape).
RETIRED_CALLBACK_FILTER = f"""              E: intent-filter (line=36)
                  E: action (line=37)
                    A: {ANDROID}name(0x01010003)="android.intent.action.VIEW" (Raw: "android.intent.action.VIEW")
                  E: category (line=38)
                    A: {ANDROID}name(0x01010003)="android.intent.category.DEFAULT" (Raw: "android.intent.category.DEFAULT")
                  E: category (line=39)
                    A: {ANDROID}name(0x01010003)="android.intent.category.BROWSABLE" (Raw: "android.intent.category.BROWSABLE")
                  E: data (line=40)
                    A: {ANDROID}scheme(0x01010027)="{APP}" (Raw: "{APP}")
"""
PROVIDER = "          E: provider (line=38)\n"
BROWSABLE_FINDING = f"{LAUNCHER} declares a BROWSABLE intent-filter; Identity v1 has no browser callback"
SCHEME_FINDING = (f"{LAUNCHER} declares an intent-filter URL scheme {APP!r}; "
                  "Identity v1 has no custom-scheme callback")


def with_filter(manifest, text=RETIRED_CALLBACK_FILTER):
    """The manifest plus one more launcher-activity filter; a provider (APK form) follows the filters."""
    changed = manifest.replace(PROVIDER, text + PROVIDER) if PROVIDER in manifest else manifest + text
    assert changed != manifest
    return changed


def make_dex(defined, referenced=()):
    """A minimal valid DEX whose class_defs name `defined`; `referenced` are types only."""
    descriptors = list(defined) + list(referenced)
    count, header = len(descriptors), 0x70
    string_ids, type_ids = header, header + 4 * len(descriptors)
    class_defs = type_ids + 4 * count
    data_start = class_defs + 32 * len(defined)
    data, offsets = bytearray(), []
    for descriptor in descriptors:
        offsets.append(data_start + len(data))
        encoded = descriptor.encode()
        data += bytes([len(encoded)]) + encoded + b"\0"
    out = bytearray(data_start) + data
    out[:8] = b"dex\n035\0"
    struct.pack_into("<I", out, 0x28, 0x12345678)
    struct.pack_into("<4I", out, 0x38, count, string_ids, count, type_ids)
    struct.pack_into("<2I", out, 0x60, len(defined), class_defs)
    for index, offset in enumerate(offsets):
        struct.pack_into("<I", out, string_ids + 4 * index, offset)
        struct.pack_into("<I", out, type_ids + 4 * index, index)
    for index in range(len(defined)):
        struct.pack_into("<I", out, class_defs + 32 * index, index)
    return bytes(out)


class DexParserTest(unittest.TestCase):
    def test_reports_defined_classes_and_ignores_merely_referenced_types(self):
        dex = make_dex(PRODUCTION, referenced=[SANDBOX, "Lkotlin/test/Asserter;"])
        self.assertEqual(inspector.dex_defined_classes(dex), set(PRODUCTION))

    def test_malformed_dex_is_an_inspection_error_not_an_empty_pass(self):
        valid = make_dex(PRODUCTION)
        for broken in (b"PK\x03\x04" + valid[4:], valid[:0x60],
                       valid[:0x38] + struct.pack("<I", 10_000) + valid[0x3C:],
                       valid[:0x28] + struct.pack("<I", 0x78563412) + valid[0x2C:]):
            with self.assertRaises(inspector.InspectionError):
                inspector.dex_defined_classes(broken)

    def test_configured_catalog_requires_exact_origin_internet_and_no_cleartext(self):
        origin = "https://api.cantos.example"
        configured = APK_MANIFEST.replace(
            '      E: application (line=21)\n',
            f'      E: uses-permission (line=20)\n        A: {ANDROID}name="android.permission.INTERNET"\n'
            '      E: application (line=21)\n'
            f'        A: {ANDROID}usesCleartextTraffic=false\n'
            '        E: meta-data (line=22)\n'
            f'          A: {ANDROID}name="CantosApiOrigin"\n'
            f'          A: {ANDROID}value="{origin}"\n',
        )
        elements = inspector.parse_xmltree(configured)
        self.assertEqual(inspector.catalog_origin_findings(elements, origin), [])
        self.assertTrue(inspector.catalog_origin_findings(elements, "https://other.example"))
        self.assertTrue(inspector.catalog_origin_findings(inspector.parse_xmltree(APK_MANIFEST), origin))
        cleartext = inspector.parse_xmltree(configured.replace("usesCleartextTraffic=false", "usesCleartextTraffic=true"))
        self.assertIn("release catalog permits cleartext traffic", inspector.catalog_origin_findings(cleartext, origin))

    def test_debug_production_does_not_waive_sandbox_testonly_or_component_policy(self):
        debug = APK_MANIFEST.replace('A: '+ANDROID+'allowBackup(0x01010280)=false',
                                     'A: '+ANDROID+'allowBackup(0x01010280)=false\n        A: '+ANDROID+'debuggable=true')
        elements = inspector.parse_xmltree(debug)
        self.assertEqual(inspector.manifest_findings(elements, APP, LAUNCHER, {DYNAMIC}, debug_apk=True), [])
        self.assertTrue(inspector.manifest_findings(elements, APP, LAUNCHER, {DYNAMIC}))
        invalid = inspector.parse_xmltree(debug.replace('debuggable=true', 'debuggable=true\n        A: '+ANDROID+'testOnly=true'))
        self.assertTrue(inspector.manifest_findings(invalid, APP, LAUNCHER, {DYNAMIC}, debug_apk=True))
        service = elements + [(10, 'service', {'name': 'com.cantos.theatre.adapters.playback.CantosPlaybackService'})]
        self.assertTrue(inspector.manifest_findings(service, APP, LAUNCHER, {DYNAMIC}, debug_apk=True))
        self.assertEqual(inspector.manifest_findings(service, APP, LAUNCHER, {DYNAMIC}, debug_apk=True,
                          allowed_components={'com.cantos.theatre.adapters.playback.CantosPlaybackService'}), [])

    def test_class_inventory_maps_compiled_files_to_descriptors(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "com/cantos/theatre/app").mkdir(parents=True)
            (root / "META-INF").mkdir()
            (root / "com/cantos/theatre/app/PreviewOwnersKt.class").write_bytes(b"x")
            (root / "com/cantos/theatre/app/Outer$Inner.class").write_bytes(b"x")
            (root / "META-INF/main.kotlin_module").write_bytes(b"x")
            self.assertEqual(inspector.class_inventory(root),
                             {SANDBOX, "Lcom/cantos/theatre/app/Outer$Inner;"})
        with self.assertRaises(inspector.InspectionError):
            inspector.class_inventory(Path(directory))


class PolicyTest(unittest.TestCase):
    def test_production_classes_pass_and_each_sandbox_route_is_rejected(self):
        self.assertEqual(inspector.class_findings(set(PRODUCTION), {SANDBOX}), [])
        rejected = [
            SANDBOX,  # exact sandbox inventory, even inside a shared package
            "Lcom/cantos/theatre/adapters/mock/MockMediaEngine;",
            "Lcom/cantos/theatre/demo/DemoWorkIds;",
            "Lcom/cantos/ui/preview/PreviewCatalog;",
            "Lcom/cantos/theatre/app/workbench/Recipe;",
            "Lcom/cantos/theatre/app/FakeStorageDatabase;",
            "Lcom/cantos/theatre/app/CantosDemoAppKt$CantosDemoApp$1;",
            "Lkotlin/test/Asserter;", "Lorg/junit/Test;", "Lkotlinx/coroutines/test/TestScope;",
            "Lokio/fakefilesystem/FakeFileSystem;", "Lio/ktor/client/engine/mock/MockEngine;",
            "Landroidx/compose/ui/tooling/preview/Preview;", "Landroidx/compose/ui/test/SemanticsMatcher;",
        ]
        for name in rejected:
            with self.subTest(name=name):
                self.assertEqual(len(inspector.class_findings({name}, {SANDBOX})), 1)

    def test_both_aapt2_forms_parse_to_the_same_manifest_facts(self):
        for text in (APK_MANIFEST, AAB_MANIFEST):
            elements = inspector.parse_xmltree(text)
            self.assertEqual(elements[0][1:], ("manifest", {"package": APP, "versionCode": "1"}))
            self.assertEqual(inspector.manifest_findings(elements, APP, LAUNCHER, {DYNAMIC}), [])
        with self.assertRaises(inspector.InspectionError):
            inspector.parse_xmltree("ERROR: no manifest")

    def test_manifest_rejects_debug_permissions_extra_launchers_and_demo_names(self):
        def findings(text, application_id=APP):
            return inspector.manifest_findings(inspector.parse_xmltree(text), application_id, LAUNCHER, {DYNAMIC})

        application = "      E: application (line=21)\n"
        cases = {
            "debuggable": APK_MANIFEST.replace(application, application + f"        A: {ANDROID}debuggable(0x0101000f)=true\n"),
            "testOnly": APK_MANIFEST.replace(application, application + f'        A: {ANDROID}testOnly(0x01010272)=true (Raw: "true")\n'),
            "permission": APK_MANIFEST.replace(DYNAMIC, "android.permission.INTERNET", 1),
            "demo": APK_MANIFEST.replace(LAUNCHER, "com.cantos.theatre.demo.MainActivity"),
        }
        self.assertTrue(any("debuggable" in item for item in findings(cases["debuggable"])))
        self.assertTrue(any("testOnly" in item for item in findings(cases["testOnly"])))
        self.assertTrue(any("INTERNET" in item for item in findings(cases["permission"])))
        demo = findings(cases["demo"])
        self.assertTrue(any("launcher components" in item for item in demo))
        self.assertTrue(any("unexpected Cantos component" in item for item in demo))
        self.assertTrue(any("mock/demo" in item for item in demo))
        self.assertTrue(any("package" in item for item in findings(APK_MANIFEST, "com.cantos.theatre.demo")))
        second = APK_MANIFEST + """          E: activity (line=90)
            A: {a}name(0x01010003)="org.example.Second" (Raw: "org.example.Second")
              E: intent-filter (line=91)
                  E: category (line=92)
                    A: {a}name(0x01010003)="android.intent.category.LAUNCHER" (Raw: "android.intent.category.LAUNCHER")
""".format(a=ANDROID)
        self.assertTrue(any("launcher components" in item for item in findings(second)))

    def test_archive_rejects_renamed_sandbox_bytes_foreign_resources_and_demo_paths(self):
        sandbox = hashlib.sha256(b"sandbox artwork").hexdigest()
        clean = {"assets/composeResources/com.cantos.theatre.resources/font/noto_sans_regular.ttf": "a" * 64}
        self.assertEqual(inspector.archive_findings(clean, {sandbox: "drawable/son_ha_moon.png"},
                                                    "com.cantos.theatre.resources"), [])
        cases = {
            "res/drawable/x.png": sandbox,
            "base/assets/composeResources/com.cantos.theatre.preview.resources/files/a.json": "b" * 64,
            "assets/composeResources/org.example.resources/files/a.json": "d" * 64,
            "assets/demo_fixture.json": "c" * 64,
        }
        for name, digest in cases.items():
            with self.subTest(name=name):
                self.assertTrue(inspector.archive_findings({name: digest}, {sandbox: "drawable/son_ha_moon.png"},
                                                           "com.cantos.theatre.resources"))


class InspectFixture(unittest.TestCase):
    """Synthetic artifacts, a sandbox inventory and a recorded-manifest `dump`; no tests of its own."""

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.root = Path(self.directory.name)
        classes = self.root / "sandbox-classes/com/cantos/theatre/app"
        classes.mkdir(parents=True)
        (classes / "PreviewOwnersKt.class").write_bytes(b"x")
        resources = self.root / "sandbox-resources/drawable"
        resources.mkdir(parents=True)
        (resources / "son_ha_moon.png").write_bytes(b"sandbox artwork")

    def tearDown(self):
        self.directory.cleanup()

    def artifacts(self, apk_classes=PRODUCTION, aab_classes=PRODUCTION, extra=None):
        apk, aab = self.root / "app.apk", self.root / "app.aab"
        with zipfile.ZipFile(apk, "w") as archive:
            archive.writestr("classes.dex", make_dex(apk_classes))
            archive.writestr("AndroidManifest.xml", b"binary")
            for name, data in (extra or {}).items():
                archive.writestr(name, data)
        with zipfile.ZipFile(aab, "w") as archive:
            archive.writestr("base/dex/classes.dex", make_dex(aab_classes[:2]))
            archive.writestr("base/dex/classes2.dex", make_dex(aab_classes[2:]))
            archive.writestr("base/manifest/AndroidManifest.xml", b"proto")
            archive.writestr("base/resources.pb", b"proto")
        return apk, aab

    def args(self, apk, aab, classes=None):
        return argparse.Namespace(
            apk=apk, aab=aab, aapt2=Path("aapt2"), application_id=APP, launcher=LAUNCHER,
            compose_resource_package="com.cantos.theatre.resources",
            forbidden_classes=[classes or self.root / "sandbox-classes"],
            forbidden_resources=[self.root / "sandbox-resources"],
            require_class=PRODUCTION[:3], allowed_permission=[DYNAMIC])

    @staticmethod
    def dump(_aapt2, _path, manifest, proto):
        assert proto in (None, b"proto") and manifest in (b"binary", b"proto")
        return AAB_MANIFEST if proto else APK_MANIFEST


class InspectTest(InspectFixture):
    def test_clean_split_artifacts_pass_with_a_recorded_identity(self):
        findings, report = inspector.inspect(self.args(*self.artifacts()), dump=self.dump)
        self.assertEqual(findings, [])
        self.assertIn("forbidden sandbox classes: 1; forbidden resource files: 1", report)
        self.assertTrue(any(line.startswith("aab ") and "defined_classes=4" in line for line in report))
        self.assertEqual(report[-1], "PASS: no sandbox, mock, demo or test content found")

    def test_sandbox_class_resource_mismatch_and_missing_required_class_are_findings(self):
        apk, aab = self.artifacts(apk_classes=PRODUCTION + [SANDBOX],
                                  extra={"assets/renamed.png": b"sandbox artwork"})
        findings, report = inspector.inspect(self.args(apk, aab), dump=self.dump)
        self.assertIn(f"apk: defines sandbox class {SANDBOX}", findings)
        self.assertIn("apk: packages sandbox resource bytes drawable/son_ha_moon.png as assets/renamed.png", findings)
        self.assertIn("apk and aab define different class sets; they are not the same release build", findings)
        self.assertNotIn("PASS: no sandbox, mock, demo or test content found", report)
        apk, aab = self.artifacts(apk_classes=PRODUCTION[1:], aab_classes=PRODUCTION[1:])
        findings, _ = inspector.inspect(self.args(apk, aab), dump=self.dump)
        self.assertIn(f"apk: required production class {PRODUCTION[0]} is absent", findings)
        self.assertIn(f"aab: required production class {PRODUCTION[0]} is absent", findings)

    def test_empty_inventory_or_unreadable_artifact_cannot_pass(self):
        apk, aab = self.artifacts()
        empty = self.root / "empty"
        empty.mkdir()
        with self.assertRaises(inspector.InspectionError):
            inspector.inspect(self.args(apk, aab, classes=empty), dump=self.dump)
        (self.root / "broken.aab").write_bytes(b"not a zip")
        with self.assertRaises(inspector.InspectionError):
            inspector.inspect(self.args(apk, self.root / "broken.aab"), dump=self.dump)

    @unittest.skipIf(os.name != "posix", "fake aapt2 is a POSIX shell script")
    def test_cli_writes_report_and_maps_outcomes_to_exit_codes(self):
        apk, aab = self.artifacts()
        manifests = self.root / "manifests"
        manifests.mkdir()
        (manifests / "apk.txt").write_text(APK_MANIFEST)
        (manifests / "aab.txt").write_text(AAB_MANIFEST)
        aapt2 = self.root / "aapt2"
        # The repacked proto APK is a temporary file; the real artifact paths end in .apk.
        aapt2.write_text(f'#!/bin/sh\ncase "$5" in */base-proto.apk) cat "{manifests}/aab.txt";; '
                         f'*) cat "{manifests}/apk.txt";; esac\n')
        aapt2.chmod(0o700)
        report = self.root / "out/report.txt"
        common = ["--apk", str(apk), "--aab", str(aab), "--aapt2", str(aapt2), "--application-id", APP,
                  "--launcher", LAUNCHER, "--compose-resource-package", "com.cantos.theatre.resources",
                  "--forbidden-classes", str(self.root / "sandbox-classes"),
                  "--allowed-permission", DYNAMIC, "--report", str(report)]
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(inspector.main(common), 0)
            self.assertIn("PASS", report.read_text())
            self.assertEqual(inspector.main(common + ["--require-class", "Lcom/cantos/theatre/app/Absent;"]), 1)
            self.assertEqual(inspector.main(["--apk", str(self.root / "absent.apk"), *common[2:]]), 2)


class IdentitySurfaceTest(unittest.TestCase):
    """Identity v1 is first-party JSON: the built manifest has a launcher entry and nothing a browser can open."""

    @staticmethod
    def findings(text):
        return inspector.identity_surface_findings(inspector.parse_xmltree(text))

    def test_launcher_only_manifests_pass_and_the_launcher_filter_is_really_inspected(self):
        for text in (APK_MANIFEST, AAB_MANIFEST):
            elements = inspector.parse_xmltree(text)
            # Non-vacuous: the one filter exists, belongs to the launcher and is MAIN/LAUNCHER only.
            self.assertEqual(inspector.intent_filters(elements),
                             [(LAUNCHER, ["android.intent.category.LAUNCHER"], [])])
            self.assertEqual(inspector.identity_surface_findings(elements), [])
            self.assertEqual(inspector.manifest_findings(elements, APP, LAUNCHER, {DYNAMIC}), [])

    def test_a_manifest_that_still_carries_the_retired_callback_filter_is_reported(self):
        for text in (APK_MANIFEST, AAB_MANIFEST):
            retired = with_filter(text)
            self.assertEqual(self.findings(retired), [BROWSABLE_FINDING, SCHEME_FINDING])
            # The launcher entry is untouched, so this control is about the callback alone.
            self.assertEqual(inspector.manifest_findings(inspector.parse_xmltree(retired), APP, LAUNCHER, {DYNAMIC}), [])

    def test_each_half_of_the_retired_entry_is_reported_on_its_own(self):
        browsable_only = RETIRED_CALLBACK_FILTER.replace(
            f'  E: data (line=40)\n                    A: {ANDROID}scheme(0x01010027)="{APP}" (Raw: "{APP}")\n', "")
        scheme_only = RETIRED_CALLBACK_FILTER.replace("android.intent.category.BROWSABLE", "android.intent.category.DEFAULT")
        self.assertNotIn("scheme", browsable_only)
        self.assertEqual(self.findings(with_filter(APK_MANIFEST, browsable_only)), [BROWSABLE_FINDING])
        self.assertEqual(self.findings(with_filter(APK_MANIFEST, scheme_only)), [SCHEME_FINDING])
        # Another scheme, including the application-id form of a debug build, is just as forbidden.
        https = RETIRED_CALLBACK_FILTER.replace(f'"{APP}" (Raw: "{APP}")', '"https" (Raw: "https")')
        self.assertEqual(self.findings(with_filter(APK_MANIFEST, https)),
                         [BROWSABLE_FINDING, SCHEME_FINDING.replace(repr(APP), "'https'")])

    def test_a_package_visibility_query_scheme_is_not_a_receiving_filter(self):
        queries = (f"      E: queries (line=50)\n        E: intent (line=51)\n          E: action (line=52)\n"
                   f'            A: {ANDROID}name(0x01010003)="android.intent.action.VIEW"\n          E: data (line=53)\n'
                   f'            A: {ANDROID}scheme(0x01010027)="https" (Raw: "https")\n')
        self.assertEqual(self.findings(APK_MANIFEST + queries), [])

    def test_retired_provider_issuer_metadata_is_reported_and_absence_passes(self):
        issuer = (f'        E: meta-data (line=60)\n          A: {ANDROID}name(0x01010003)="CantosNativeIssuer" (Raw: "CantosNativeIssuer")\n'
                  f'          A: {ANDROID}value(0x01010024)="" (Raw: "")\n')
        self.assertEqual(self.findings(APK_MANIFEST), [])
        self.assertEqual(self.findings(APK_MANIFEST + issuer),
                         ["retired native issuer metadata is present in an Identity v1 host"])


class IdentitySurfaceWiringTest(InspectFixture):
    """The control is unconditional: no flag, build type or audit mode lets a callback entry through."""

    def dump_retired(self, _aapt2, _path, manifest, proto):
        return with_filter(self.dump(_aapt2, _path, manifest, proto))

    def test_every_build_kind_and_flag_combination_reports_the_retired_entry(self):
        for debug_apk in (False, True):
            for audit in (False, True):
                for enforce_marker in (False, True):
                    with self.subTest(debug_apk=debug_apk, audit=audit, enforce_marker=enforce_marker):
                        apk, aab = self.artifacts()
                        args = self.args(apk, None if debug_apk else aab)
                        args.debug_apk, args.audit, args.enforce_audit_marker = debug_apk, audit, enforce_marker
                        findings, report = inspector.inspect(args, dump=self.dump_retired)
                        self.assertIn(f"apk: {BROWSABLE_FINDING}", findings)
                        self.assertIn(f"apk: {SCHEME_FINDING}", findings)
                        self.assertEqual(any(f.startswith("aab: ") for f in findings), not debug_apk)
                        self.assertNotIn("PASS: no sandbox, mock, demo or test content found", report)

    def test_the_clean_launcher_only_manifest_stays_clean_in_the_same_matrix(self):
        for debug_apk in (False, True):
            with self.subTest(debug_apk=debug_apk):
                apk, aab = self.artifacts()
                args = self.args(apk, None if debug_apk else aab)
                args.debug_apk = debug_apk
                findings, _ = inspector.inspect(args, dump=self.dump)
                self.assertEqual(findings, [])

    def test_the_retired_command_line_flags_no_longer_exist(self):
        base = ["--apk", "a.apk", "--aab", "a.aab", "--aapt2", "aapt2", "--application-id", APP, "--launcher", LAUNCHER,
                "--compose-resource-package", "com.cantos.theatre.resources", "--report", str(self.root / "r.txt")]
        for retired in (["--enforce-callback"], ["--native-issuer", "https://idm.example/oauth2/openid/cantos-theatre-native"]):
            with self.subTest(retired=retired[0]):
                stderr = io.StringIO()
                with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit) as raised:
                    inspector.main(base + retired)
                self.assertEqual(raised.exception.code, 2)
                self.assertIn(f"unrecognized arguments: {retired[0]}", stderr.getvalue())


if __name__ == "__main__":
    unittest.main()
