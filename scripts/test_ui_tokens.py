"""Token contract checks independent of the Leptos runtime and browser."""

import json
from html.parser import HTMLParser
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

from ui_tokens import (
    OUTPUTS, ROOT, SOURCE, TEMPLATE, check_outputs, contrast, css_value,
    load_source, outputs, rust_variant, validate, variable, write_outputs,
)


def token(kind, value):
    return {"type": kind, "value": value}


def fixture():
    """A small independently specified contract, separate from the actual palette."""
    return {
        "version": "0.2.0",
        "tokens": {
            "type.family.ui": token("fontFamily", '"Noto Sans", system-ui, sans-serif'),
            "type.body.size": token("dimension", 16),
            "type.body.lineHeight": token("number", 1.5),
            "type.body.weight": token("fontWeight", 400),
            "space.controlInline": token("dimension", 12),
            "shape.reading": token("dimension", 12),
            "size.control": token("dimension", 44),
            "state.hoverOpacity": token("number", 0.08),
            "elevation.overlay": token("shadow", {
                "x": 0, "y": 8, "blur": 24, "spread": -2,
                "color": "#000000", "opacity": 0.16,
            }),
            "motion.feedback.duration": token("duration", 140),
            "motion.navigation.duration": token("duration", 260),
            "motion.emphasis.duration": token("duration", 260),
            "motion.effects.easing": token("cubicBezier", [0.2, 0, 0, 1]),
            "breakpoint.medium": token("dimension", 600),
            "breakpoint.expanded": token("dimension", 840),
        },
        "themes": {
            "light": {"color.onSurface": token("color", "#000000"),
                      "color.surface": token("color", "#ffffff")},
            "dark": {"color.onSurface": token("color", "#ffffff"),
                     "color.surface": token("color", "#000000")},
        },
        "aliases": {"shape.field": "shape.reading"},
        "contrast_pairs": [["color.onSurface", "color.surface", 4.5]],
        "reduced_motion": {"motion.feedback.duration": 0, "motion.navigation.duration": 0,
                           "motion.emphasis.duration": 0},
        "density": {"compact": {"size.control": 44, "space.controlInline": 12},
                    "comfortable": {"size.control": 48, "space.controlInline": 16}},
    }


SAMPLE_TEMPLATE = (
    ".cantos-ui .sample { color: var(--cantos-color-on-surface); }\n"
    "@media (min-width: {{breakpoint.medium}}) { .cantos-ui .sample { display: grid; } }\n"
    "@media (min-width: {{breakpoint.expanded}}) { .cantos-ui .sample { display: flex; } }\n"
)


class TokenContract(unittest.TestCase):
    def setUp(self):
        self.source = fixture()

    def test_legacy_cli_propagates_missing_output_failure(self):
        with tempfile.TemporaryDirectory() as directory:
            result = subprocess.run(
                [sys.executable, str(ROOT / "scripts/studio_tokens.py"), "--check",
                 "--output-root", directory], capture_output=True, text=True,
            )
        self.assertEqual(result.returncode, 1, result.stdout + result.stderr)
        self.assertIn("FAIL: generated UI token drift", result.stderr)

    def test_historical_token_snapshot_preserves_its_published_bytes(self):
        import hashlib
        raw = (ROOT / "packages/ui-system/shared/legacy/studio-tokens-0.1.0.json").read_bytes()
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         "c320a5b95afa41adc59f9c2a84e3dd2ae780a86dd9ea7f558f191963d992e571")
        historical = load_source(raw)
        canonical = validate(load_source(SOURCE.read_bytes()))
        current_roles = set(canonical["tokens"]) | set(canonical["themes"]["light"]) | set(canonical["aliases"])
        self.assertLessEqual(set(historical["themes"]["light"]) | set(historical["logical_dimensions"]),
                             current_roles)

    def test_real_web_consumers_load_tokens_and_component_styles_before_app_geometry(self):
        class Stylesheets(HTMLParser):
            def __init__(self):
                super().__init__()
                self.paths = []

            def handle_starttag(self, tag, attributes):
                attributes = dict(attributes)
                if tag == "link" and "data-trunk" in attributes and attributes.get("rel") == "css":
                    self.paths.append(attributes["href"])

        for app, geometry in (("cantos-studio", "studio.css"), ("cantos-theatre", "src/styles/theatre.css")):
            with self.subTest(app=app):
                app_root = ROOT / "apps" / app / "web"
                links = Stylesheets()
                links.feed((app_root / "index.html").read_text())
                paths = [(app_root / path).resolve() for path in links.paths]
                expected = [ROOT / OUTPUTS["css"], ROOT / OUTPUTS["foundation"], app_root / geometry]
                for path in expected:
                    self.assertEqual(paths.count(path), 1, f"missing/duplicate stylesheet: {path}")
                    self.assertTrue(path.is_file(), f"missing stylesheet source: {path}")
                positions = [paths.index(path) for path in expected]
                self.assertEqual(positions, sorted(positions), "app geometry must follow the shared foundation")

    def refuse(self, message):
        with self.assertRaisesRegex(ValueError, message):
            validate(self.source)

    def test_contrast_formula_is_anchored_at_known_srgb_results(self):
        self.assertEqual(contrast("#000000", "#ffffff"), 21)
        self.assertEqual(contrast("#654390", "#654390"), 1)
        self.assertAlmostEqual(contrast("#777777", "#ffffff"), 4.478089453577214)
        self.assertAlmostEqual(contrast("#767676", "#ffffff"), 4.542224959605253)
        self.assertEqual(contrast("#777777", "#ffffff"), contrast("#ffffff", "#777777"))

    def test_json_refuses_duplicate_keys_at_any_depth_and_nonfinite_constants(self):
        cases = [('{"version":"0.2.0","version":"0.2.0"}', "duplicate JSON key: version"),
                 ('{"tokens":{"space.sm":{},"space.sm":{}}}', "duplicate JSON key: space.sm"),
                 ('{"value":NaN}', "non-finite JSON number: NaN"),
                 ('{"value":Infinity}', "non-finite JSON number: Infinity"),
                 ('{"value":-Infinity}', "non-finite JSON number: -Infinity")]
        for raw, message in cases:
            with self.subTest(raw=raw), self.assertRaisesRegex(ValueError, message):
                load_source(raw)

    def test_outputs_are_byte_identical_for_json_reordering_and_whitespace(self):
        original = outputs(json.dumps(self.source), SAMPLE_TEMPLATE)
        reordered = {key: value for key, value in reversed(list(self.source.items()))}
        for key in ("tokens", "aliases", "density", "themes"):
            reordered[key] = dict(reversed(list(self.source[key].items())))
        self.assertEqual(original, outputs(json.dumps(reordered, indent=4), SAMPLE_TEMPLATE))
        self.assertEqual(original, outputs(json.dumps(self.source), SAMPLE_TEMPLATE))
        self.assertEqual(self.source, fixture(), "pure generation must not mutate its input")

    def test_each_type_has_an_exact_css_conversion(self):
        cases = [("dimension", 16, "1rem"), ("dimension", 0, "0rem"),
                 ("number", 1.5, "1.5"), ("fontWeight", 600, "600"),
                 ("duration", 140, "140ms"), ("color", "#654390", "#654390"),
                 ("fontFamily", '"Noto Serif", Georgia, serif', '"Noto Serif", Georgia, serif'),
                 ("cubicBezier", [0.2, 0, 0, 1], "cubic-bezier(0.2, 0, 0, 1)")]
        for kind, value, expected in cases:
            with self.subTest(kind=kind, value=value):
                self.assertEqual(css_value(token(kind, value)), expected)
        self.assertEqual(css_value(self.source["tokens"]["elevation.overlay"]),
                         "0rem 0.5rem 1.5rem -0.125rem rgba(0, 0, 0, 0.16)")

    def test_fractional_auth_outline_retains_web_and_native_units_without_rounding(self):
        self.source["version"] = "0.4.0"
        self.source["tokens"]["size.authOutline"] = token("dimension", 1.5)
        generated = outputs(json.dumps(self.source), SAMPLE_TEMPLATE)
        self.assertEqual(css_value(token("dimension", 1.5)), "0.09375rem")
        self.assertIn("--cantos-size-auth-outline: 0.09375rem;", generated[OUTPUTS["css"]])
        self.assertIn('TokenValue("size.authOutline", "dimension", "1.5", "1.5", null, null, null)',
                      generated[OUTPUTS["kotlin"]])
        self.assertIn('Self::SizeAuthOutline => "0.09375rem",', generated[OUTPUTS["rust"]])

    def test_scoped_motion_requires_an_explicit_zero_reduced_motion_mapping(self):
        self.source["tokens"]["motion.shape.duration"] = token("duration", 220)
        self.refuse("reduced_motion: expected keys")
        self.source["reduced_motion"]["motion.shape.duration"] = 0
        generated = outputs(json.dumps(self.source), SAMPLE_TEMPLATE)
        self.assertIn("--cantos-motion-shape-duration: 0ms;", generated[OUTPUTS["css"]])
        self.assertIn('"motion.shape.duration"', generated[OUTPUTS["kotlin"]])

    def test_generation_covers_aliases_themes_density_motion_and_static_system_fallback(self):
        generated = outputs(json.dumps(self.source), SAMPLE_TEMPLATE)
        css, rust, foundation = (generated[OUTPUTS[name]] for name in ("css", "rust", "foundation"))
        self.assertIn(":where(.cantos-ui) {", css)
        self.assertNotIn(":root", css)
        self.assertIn('.cantos-ui:not([data-theme="light"]):not([data-theme="dark"])', css)
        self.assertIn('--cantos-shape-field: var(--cantos-shape-reading);', css)
        self.assertIn('.cantos-ui[data-density="comfortable"] {\n  --cantos-size-control: 3rem;', css)
        self.assertIn("@media (prefers-reduced-motion: reduce)", css)
        self.assertIn("--cantos-motion-feedback-duration: 0ms;", css)
        self.assertIn('pub const VERSION: &str = "0.2.0";', rust)
        self.assertIn("pub const ALL: &[Token]", rust)
        self.assertIn("Self::ShapeField => Self::ShapeReading.css_value(theme)", rust)
        self.assertIn('crate::Theme::Light | crate::Theme::System => "#000000"', rust)
        self.assertIn("min-width: 37.5rem", foundation)
        self.assertIn("min-width: 52.5rem", foundation)
        self.assertNotIn("{{", foundation)
        roles = set(self.source["tokens"]) | set(self.source["themes"]["light"]) | set(self.source["aliases"])
        for role in roles:
            with self.subTest(role=role):
                self.assertIn(variable(role) + ":", css)
                self.assertIn(f"Token::{rust_variant(role)},", rust)
                self.assertIn(f'Self::{rust_variant(role)} => "{variable(role)}"', rust)

    def test_refuses_invalid_schema_theme_coverage_and_unknown_types(self):
        mutations = [
            (lambda value: value.update(version="0.1.0"), "unsupported token version"),
            (lambda value: value.update(extra={}), "source: expected keys"),
            (lambda value: value["themes"].pop("dark"), "themes: expected keys"),
            (lambda value: value["themes"]["dark"].pop("color.surface"), "identical roles"),
            (lambda value: value["themes"]["dark"]["color.surface"].update(type="mystery"), "unknown token type"),
            (lambda value: value["tokens"].update({"color.surface": token("color", "#ffffff")}), "must not overlap"),
            (lambda value: value["tokens"]["space.controlInline"].update(unit="px"), "expected keys"),
            (lambda value: value["tokens"].pop("state.hoverOpacity"), "missing required semantic token family"),
        ]
        for mutate, message in mutations:
            with self.subTest(message=message):
                self.source = fixture()
                mutate(self.source)
                self.refuse(message)
        self.source = fixture()
        self.source["tokens"]["type.body.size"]["type"] = "number"
        self.source["themes"]["light"]["type.body.size"] = self.source["tokens"].pop("type.body.size")
        self.source["themes"]["dark"]["type.body.size"] = token("dimension", 16)
        self.refuse("type.body.size: expected dimension type, got number")

    def test_semantic_roles_refuse_every_wrong_type_before_css_generation(self):
        cases = [
            ("space.controlInline", "color", "#ff0000", "dimension"),
            ("shape.reading", "number", 12, "dimension"),
            ("size.control", "number", 44, "dimension"),
            ("breakpoint.medium", "number", 600, "dimension"),
            ("type.family.ui", "color", "#ff0000", "fontFamily"),
            ("type.body.size", "number", 16, "dimension"),
            ("type.body.lineHeight", "dimension", 1.5, "number"),
            ("type.body.weight", "number", 400, "fontWeight"),
            ("motion.feedback.duration", "number", 140, "duration"),
            ("motion.effects.easing", "color", "#ff0000", "cubicBezier"),
            ("elevation.overlay", "color", "#ff0000", "shadow"),
            ("state.hoverOpacity", "dimension", 0.08, "number"),
            ("brand.primary", "dimension", 16, "color"),
        ]
        for role, kind, value, expected_type in cases:
            with self.subTest(role=role):
                self.source = fixture()
                self.source["tokens"][role] = token(kind, value)
                with self.assertRaises(ValueError) as error:
                    outputs(json.dumps(self.source), SAMPLE_TEMPLATE)
                self.assertEqual(str(error.exception),
                                 f"{role}: expected {expected_type} type, got {kind}")
        self.source = fixture()
        self.source["themes"]["dark"]["color.surface"] = token("dimension", 16)
        with self.assertRaises(ValueError) as error:
            validate(self.source)
        self.assertEqual(str(error.exception), "color.surface: expected color type, got dimension")

    def test_ambiguous_subtypes_and_alias_type_changes_are_refused(self):
        for role, kind, value in [
            ("type.body.tracking", "number", 0),
            ("type.family", "fontFamily", "serif"),
            ("type.family.ui.size", "dimension", 16),
            ("motion.feedback.spring", "number", 1),
            ("motion.feedback", "duration", 140),
            ("state.hover", "number", 0.08),
            ("state.hoverOpacity.extra", "number", 0.08),
        ]:
            with self.subTest(role=role):
                self.source = fixture()
                self.source["tokens"][role] = token(kind, value)
                with self.assertRaises(ValueError) as error:
                    validate(self.source)
                self.assertEqual(str(error.exception), f"{role}: unrecognized semantic subtype")
        self.source = fixture()
        self.source["aliases"]["shape.field"] = "color.surface"
        with self.assertRaises(ValueError) as error:
            validate(self.source)
        self.assertEqual(str(error.exception), "shape.field: alias requires dimension type, target color.surface has color")

    def test_refuses_unresolved_unsafe_or_out_of_range_values(self):
        cases = [("type.body.size", "dimension", -1, "nonnegative dimension"),
                 ("type.body.size", "dimension", True, "nonnegative dimension"),
                 ("type.body.size", "dimension", float("inf"), "nonnegative dimension"),
                 ("state.hoverOpacity", "number", 1.01, "opacity must be"),
                 ("state.hoverOpacity", "number", float("nan"), "nonnegative number"),
                 ("type.body.lineHeight", "number", 0, "line height must be positive"),
                 ("type.body.weight", "fontWeight", 99, "font weight must be"),
                 ("type.body.weight", "fontWeight", 400.0, "font weight must be"),
                 ("motion.feedback.duration", "duration", 1.5, "duration must be"),
                 ("motion.feedback.duration", "duration", -1, "duration must be"),
                 ("motion.effects.easing", "cubicBezier", [0.2, -0.1, 0, 1], "cubic bezier"),
                 ("motion.effects.easing", "cubicBezier", [0, 1, 0], "cubic bezier"),
                 ("type.family.ui", "fontFamily", "serif; color: red", "safe CSS font family"),
                 ("type.family.ui", "fontFamily", '"Noto\\ Sans", serif', "safe CSS font family"),
                 ("type.family.ui", "fontFamily", "serif\n", "safe CSS font family")]
        for role, kind, value, message in cases:
            with self.subTest(role=role, value=value):
                self.source = fixture()
                self.source["tokens"][role] = token(kind, value)
                self.refuse(message)
        for color in ("<pending>", "#FFFFFF", "#fff", "rgb(0, 0, 0)"):
            with self.subTest(color=color):
                self.source = fixture()
                self.source["themes"]["light"]["color.surface"]["value"] = color
                self.refuse("resolved lowercase #rrggbb")

    def test_refuses_invalid_shadow_offsets_blur_opacity_and_color(self):
        for field, value, message in [("x", float("inf"), "shadow x must be finite"),
                                      ("blur", -1, "shadow blur/opacity"),
                                      ("opacity", 1.1, "shadow blur/opacity"),
                                      ("color", "#FFF", "resolved lowercase")]:
            with self.subTest(field=field):
                self.source = fixture()
                self.source["tokens"]["elevation.overlay"]["value"][field] = value
                self.refuse(message)

    def test_refuses_invalid_names_mapping_collisions_and_alias_chains(self):
        for role, message in [("space.8", "invalid semantic role"),
                              ("reference.lavender", "unknown semantic family"),
                              ("space.row-gap", "invalid semantic role")]:
            with self.subTest(role=role):
                self.source = fixture()
                self.source["tokens"][role] = token("dimension", 8)
                self.refuse(message)
        self.source = fixture()
        self.source["tokens"]["space.rowGap"] = token("dimension", 8)
        self.source["tokens"]["space.row.gap"] = token("dimension", 8)
        self.refuse("CSS name collision")
        for aliases, message in [({"shape.field": "shape.unknown"}, "target a canonical role directly"),
                                 ({"shape.field": "shape.legacy", "shape.legacy": "shape.reading"}, "target a canonical role directly"),
                                 ({"shape.field": "shape.legacy", "shape.legacy": "shape.field"}, "target a canonical role directly"),
                                 ({"shape.reading": "shape.reading"}, "overlaps a canonical role")]:
            with self.subTest(aliases=aliases):
                self.source = fixture()
                self.source["aliases"] = aliases
                self.refuse(message)

    def test_refuses_weaker_or_unmet_contrast_floors_and_invalid_pairs(self):
        for pair, message in [("not-a-pair", "expected \\[foreground"),
                              (["color.onSurface", "color.surface", 0], "WCAG floor"),
                              (["color.onSurface", "color.surface", True], "WCAG floor"),
                              (["color.onSurface", "color.surface", 4.49], "WCAG floor"),
                              (["shape.reading", "color.surface", 3], "canonical color roles"),
                              (["unknown", "color.surface", 3], "canonical color roles")]:
            with self.subTest(pair=pair):
                self.source = fixture()
                self.source["contrast_pairs"] = [pair]
                self.refuse(message)
        self.source = fixture()
        self.source["themes"]["light"]["color.onSurface"]["value"] = "#777777"
        self.refuse(r"light: color.onSurface on color.surface contrast 4.478 < 4.5")
        self.source = fixture()
        self.source["contrast_pairs"] *= 2
        self.refuse("duplicate foreground/background pair")

    def test_refuses_missing_or_nonzero_reduced_motion_and_small_density_targets(self):
        self.source["reduced_motion"].pop("motion.feedback.duration")
        self.refuse("reduced_motion: expected keys")
        self.source = fixture()
        self.source["reduced_motion"]["motion.feedback.duration"] = 100
        self.refuse("reduced motion duration must be zero")
        self.source = fixture()
        self.source["density"]["compact"]["size.control"] = 43
        self.refuse("control hit area must be at least 44")
        self.source = fixture()
        self.source["density"]["comfortable"]["space.controlInline"] = True
        self.refuse("finite nonnegative dimension")
        self.source = fixture()
        self.source["tokens"]["breakpoint.medium"]["value"] = 840
        self.refuse("medium must precede expanded")

    def test_template_refuses_unknown_non_breakpoint_malformed_and_undefined_roles(self):
        for template, message in [("{{space.controlInline}}", "unknown breakpoint placeholder"),
                                  ("{{breakpoint.unknown}}", "unknown breakpoint placeholder"),
                                  ("{{breakpoint.medium}", "malformed placeholder"),
                                  ("var(--cantos-color-missing)", "undefined CSS roles"),
                                  ("var(--cantos-web-shape-missing)", "undefined CSS roles"),
                                  (".a { --cantos-shape-reading: 2rem; }", "may declare only new --cantos-web-"),
                                  (".a { --cantos-color-new: red; }", "may declare only new --cantos-web-"),
                                  (".a { --cantos-web-color-surface: red; }\n.b { --cantos-color-surface: red; }",
                                   "may declare only new --cantos-web-")]:
            with self.subTest(template=template), self.assertRaisesRegex(ValueError, message):
                outputs(json.dumps(self.source), template)

    def test_template_may_declare_and_use_a_renderer_local_web_role(self):
        template = ":where(.cantos-ui) {\n  --cantos-web-shape-control: 1.5rem;\n}\n.a { border-radius: var(--cantos-web-shape-control); }\n"
        generated = outputs(json.dumps(self.source), template)
        self.assertIn("--cantos-web-shape-control: 1.5rem;", generated[OUTPUTS["foundation"]])
        for name in ("css", "rust", "kotlin"):
            self.assertNotIn("web-shape", generated[OUTPUTS[name]], f"{name} must not learn Web-only roles")

    def test_check_mode_reports_every_missing_and_drifted_output_without_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            expected = outputs(json.dumps(self.source), SAMPLE_TEMPLATE, directory)
            self.assertEqual(len(check_outputs(expected)), len(OUTPUTS))
            write_outputs(expected)
            self.assertEqual(check_outputs(expected), [])
            paths = list(expected)
            paths[0].unlink()
            paths[1].write_text("manual drift", encoding="utf-8")
            paths[2].write_text("manual drift", encoding="utf-8")
            failures = check_outputs(expected)
            self.assertEqual(len(failures), 3)
            self.assertTrue(failures[0].startswith("missing:"))
            self.assertTrue(all(failure.startswith("drift:") for failure in failures[1:]))
            self.assertFalse(paths[0].exists())
            self.assertEqual(paths[1].read_text(), "manual drift")

    def test_native_mapping_keeps_logical_values_and_all_names_with_no_css_units(self):
        expected = outputs(json.dumps(self.source), SAMPLE_TEMPLATE)
        kotlin = expected[OUTPUTS["kotlin"]]
        self.assertIn('TokenValue("size.control", "dimension", "44", "44", "44", "48", null)', kotlin)
        self.assertIn('TokenValue("shape.field", "dimension", "12", "12", null, null, "shape.reading")', kotlin)
        self.assertIn('TokenValue("type.body.size", "dimension", "16", "16"', kotlin)
        self.assertNotIn('"1rem"', kotlin)
        self.assertIn('require(theme == "light" || theme == "dark")', kotlin)
        self.assertIn('reducedMotion && name in reducedRoles', kotlin)

    def test_cli_validates_all_artifacts_before_writing_and_check_mode_is_read_only(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, template = root / "source.json", root / "foundation.in"
            source.write_text(json.dumps(self.source), encoding="utf-8")
            template.write_text(SAMPLE_TEMPLATE, encoding="utf-8")
            command = [sys.executable, str(ROOT / "scripts/ui_tokens.py"), "--source", str(source),
                       "--template", str(template), "--output-root", str(root)]
            generated = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(generated.returncode, 0, generated.stderr)
            expected = outputs(source.read_bytes(), SAMPLE_TEMPLATE, root)
            before = {path: path.read_bytes() for path in expected}
            template.write_text("{{breakpoint.invalid}}", encoding="utf-8")
            refused = subprocess.run(command, capture_output=True, text=True)
            self.assertEqual(refused.returncode, 1)
            self.assertIn("unknown breakpoint placeholder", refused.stderr)
            self.assertEqual(before, {path: path.read_bytes() for path in expected})
            template.write_text(SAMPLE_TEMPLATE, encoding="utf-8")
            path = next(iter(expected))
            path.write_text("drift", encoding="utf-8")
            checked = subprocess.run(command + ["--check"], capture_output=True, text=True)
            self.assertEqual(checked.returncode, 1)
            self.assertIn("drift:", checked.stderr)
            self.assertEqual(path.read_text(), "drift")

    def test_failed_staging_preserves_existing_outputs_and_removes_temporary_files(self):
        with tempfile.TemporaryDirectory() as directory:
            expected = outputs(json.dumps(self.source), SAMPLE_TEMPLATE, directory)
            write_outputs(expected)
            before = {path: path.read_bytes() for path in expected}
            invalid = {path: content + "changed" for path, content in expected.items()}
            invalid[next(reversed(invalid))] = "\ud800"
            with self.assertRaises(UnicodeEncodeError):
                write_outputs(invalid)
            self.assertEqual(before, {path: path.read_bytes() for path in expected})
            self.assertEqual(list(Path(directory).rglob(".*.css.*")), [])
            self.assertEqual(list(Path(directory).rglob(".*.rs.*")), [])


class CanonicalRepositoryTokens(unittest.TestCase):
    def test_v3_readable_body_and_targets_keep_platform_units_and_editor_role(self):
        source = validate(load_source(SOURCE.read_bytes()))
        self.assertEqual(source["version"], "0.6.0")
        self.assertEqual(source["tokens"]["type.body.size"], token("dimension", 16))
        self.assertEqual(source["tokens"]["type.editor.size"], token("dimension", 14))
        self.assertEqual(source["tokens"]["size.control"], token("dimension", 48))
        self.assertEqual(source["density"]["compact"], {"size.control": 48, "space.controlInline": 12})
        self.assertEqual(source["density"]["comfortable"], {"size.control": 48, "space.controlInline": 16})
        generated = outputs(SOURCE.read_bytes(), TEMPLATE.read_text(encoding="utf-8"))
        self.assertIn("--cantos-type-body-size: 1rem;", generated[OUTPUTS["css"]])
        self.assertIn("--cantos-size-control: 3rem;", generated[OUTPUTS["css"]])
        self.assertIn('TokenValue("type.body.size", "dimension", "16", "16", null, null, null)',
                      generated[OUTPUTS["kotlin"]])
        self.assertIn('TokenValue("size.control", "dimension", "48", "48", "48", "48", null)',
                      generated[OUTPUTS["kotlin"]])

    def test_reported_wrong_type_mutations_of_actual_source_are_refused(self):
        cases = [("type.body.size", "number", 14, "dimension"),
                 ("type.family.ui", "color", "#ff0000", "fontFamily"),
                 ("space.sm", "color", "#ff0000", "dimension")]
        for role, kind, value, expected_type in cases:
            with self.subTest(role=role):
                source = load_source(SOURCE.read_bytes())
                source["tokens"][role] = token(kind, value)
                with self.assertRaises(ValueError) as error:
                    outputs(json.dumps(source), TEMPLATE.read_text(encoding="utf-8"))
                self.assertEqual(str(error.exception),
                                 f"{role}: expected {expected_type} type, got {kind}")

    def test_version_rejects_non_string_input_with_a_validation_error(self):
        for version in (None, [], {}, 201):
            source = load_source(SOURCE.read_bytes())
            source["version"] = version
            with self.subTest(version=version):
                with self.assertRaisesRegex(ValueError, "unsupported token version"):
                    validate(source)

    def test_actual_source_meets_every_contrast_floor_in_both_themes(self):
        source = validate(load_source(SOURCE.read_bytes()))
        for theme, tokens in source["themes"].items():
            colors = source["tokens"] | tokens
            for foreground, background, floor in source["contrast_pairs"]:
                with self.subTest(theme=theme, foreground=foreground, background=background):
                    self.assertGreaterEqual(contrast(colors[foreground]["value"], colors[background]["value"]), floor)

    def test_all_committed_outputs_equal_pure_generation(self):
        expected = outputs(SOURCE.read_bytes(), TEMPLATE.read_text(encoding="utf-8"))
        self.assertEqual(check_outputs(expected), [])

    def test_all_ui_system_and_app_css_role_uses_are_defined(self):
        source = validate(load_source(SOURCE.read_bytes()))
        roles = source["tokens"] | source["themes"]["light"] | source["aliases"]
        defined = {variable(role) for role in roles} | web_roles()
        consumers = list((ROOT / "packages/ui-system/leptos").glob("*.css*"))
        consumers += list((ROOT / "apps/cantos-studio/web").glob("*.css"))
        consumers += list((ROOT / "apps/cantos-theatre/web").glob("*.css"))
        self.assertTrue(consumers)
        for path in consumers:
            with self.subTest(path=path.relative_to(ROOT)):
                used = set(re.findall(r"var\(\s*(--cantos-[a-z0-9-]+)", path.read_text(encoding="utf-8")))
                self.assertLessEqual(used, defined, f"undefined roles in {path}")


WEB_SHAPE_ROLES = {
    "--cantos-web-shape-control": "1.5rem",
    "--cantos-web-shape-reading": "0.75rem",
    "--cantos-web-shape-container": "1.125rem",
    "--cantos-web-shape-overlay": "1.5rem",
    "--cantos-web-shape-panel": "1.5rem",
}
# These historical CMP roles must not leak into Web's regular controls. Scoped
# auth/account roles are intentionally shared, and do not redefine that scale.
CANONICAL_SHAPE_USE = re.compile(
    r"var\(\s*--cantos-shape-(?:detail|control|reading|field|container|overlay|thumbnail)\b")


def web_roles():
    """Renderer-local roles declared by the committed Leptos template."""
    return set(re.findall(r"(?<![\w-])(--cantos-web-[a-z0-9-]+)\s*:", TEMPLATE.read_text(encoding="utf-8")))


def web_css_files():
    paths = list((ROOT / "packages/ui-system/leptos").glob("*.css*"))
    paths += list((ROOT / "packages/ui-system/leptos/preview").glob("*.css"))
    paths += list((ROOT / "apps/cantos-studio/web").glob("*.css"))
    paths += list((ROOT / "apps/cantos-theatre/web/src/styles").glob("*.css"))
    return sorted(path for path in paths if path.name not in {"tokens.css"})


def declared_values(css):
    return dict(re.findall(r"(--cantos-web-shape-[a-z]+)\s*:\s*([^;]+);", css))


class WebShapeIndependence(unittest.TestCase):
    """Web corners (24/12/18/24, panel 24) must not move when the shared CMP shape scale does."""

    def test_web_shape_roles_hold_the_documented_historical_values_in_the_one_source(self):
        self.assertEqual(declared_values(TEMPLATE.read_text(encoding="utf-8")), WEB_SHAPE_ROLES)
        self.assertEqual(declared_values(OUTPUTS["foundation"].read_text(encoding="utf-8")), WEB_SHAPE_ROLES)

    def test_only_the_template_declares_web_shape_values(self):
        for path in web_css_files():
            if path in (TEMPLATE, OUTPUTS["foundation"]):
                continue
            with self.subTest(path=path.relative_to(ROOT)):
                self.assertEqual(declared_values(path.read_text(encoding="utf-8")), {})

    def test_web_components_use_web_shape_roles_never_the_shared_cmp_scale(self):
        for path in web_css_files():
            if path == OUTPUTS["foundation"]:
                continue
            with self.subTest(path=path.relative_to(ROOT)):
                self.assertEqual(CANONICAL_SHAPE_USE.findall(path.read_text(encoding="utf-8")), [])
        generated = OUTPUTS["foundation"].read_text(encoding="utf-8")
        self.assertEqual(CANONICAL_SHAPE_USE.findall(generated), [])

    def test_changing_regular_shared_shape_values_leaves_web_corners_and_pill_untouched(self):
        source = load_source(SOURCE.read_bytes())
        template = TEMPLATE.read_text(encoding="utf-8")
        baseline = outputs(json.dumps(source), template)
        for role in ("shape.detail", "shape.control", "shape.reading", "shape.container",
                     "shape.overlay", "shape.thumbnail"):
            source["tokens"][role]["value"] = 1
        shrunk = outputs(json.dumps(source), template)
        self.assertEqual(shrunk[OUTPUTS["foundation"]].split("*/", 1)[1],
                         baseline[OUTPUTS["foundation"]].split("*/", 1)[1])
        self.assertEqual(declared_values(shrunk[OUTPUTS["foundation"]]), WEB_SHAPE_ROLES)
        self.assertIn("--cantos-shape-control: 0.0625rem;", shrunk[OUTPUTS["css"]])
        self.assertIn("--cantos-shape-pill: 62.4375rem;", shrunk[OUTPUTS["css"]])

    def test_cmp_outputs_and_canonical_scale_stay_independent_of_web_roles(self):
        source = validate(load_source(SOURCE.read_bytes()))
        self.assertEqual({role: source["tokens"][role]["value"] for role in
                          ("shape.detail", "shape.control", "shape.reading", "shape.container",
                           "shape.overlay", "shape.thumbnail")},
                         {"shape.detail": 4, "shape.control": 8, "shape.reading": 12,
                          "shape.container": 16, "shape.overlay": 20, "shape.thumbnail": 10})
        for name in ("css", "rust", "kotlin"):
            with self.subTest(output=name):
                self.assertNotIn("web-shape", OUTPUTS[name].read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
