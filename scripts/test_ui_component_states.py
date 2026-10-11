"""Independent contrast and mounted-consumer anatomy checks for the UI system."""

import json
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parent.parent


def channels(color):
    return tuple(int(color[index:index + 2], 16) / 255 for index in (1, 3, 5))


def luminance(rgb):
    linear = [value / 12.92 if value <= 0.04045 else ((value + 0.055) / 1.055) ** 2.4
              for value in rgb]
    return sum(weight * value for weight, value in zip((0.2126, 0.7152, 0.0722), linear))


def ratio(a, b):
    light, dark = sorted((luminance(a), luminance(b)), reverse=True)
    return (light + 0.05) / (dark + 0.05)


class ComponentStates(unittest.TestCase):
    def test_studio_validation_controls_keep_unique_ids_across_mounted_panes(self):
        # Both panes remain mounted even while hidden. Inspect the real CSR callsites;
        # native string fixtures of a single action cannot detect this collision.
        callers = (ROOT / "apps/cantos-studio/web/src/view.rs",
                   ROOT / "apps/cantos-studio/web/src/adaptation/browser.rs")
        ids = []
        for path in callers:
            calls = re.findall(r"<ValidationControls\s+(.*?)\s*/>", path.read_text(), re.S)
            self.assertEqual(len(calls), 1, path)
            control_id = re.search(r'\bid="([^\"]+)"', calls[0])
            self.assertIsNotNone(control_id, f"{path}: caller must own its validation ID")
            ids.append(control_id.group(1))
        self.assertEqual(len(ids), len(set(ids)), "mounted validation controls share an ID")
        reserved_ids = [value + suffix for value in ids for suffix in ("", "-reason")]
        self.assertEqual(len(reserved_ids), len(set(reserved_ids)))
        component = callers[0].read_text().split("fn ValidationControls(", 1)[1].split(
            "fn dispatch_save(", 1)[0]
        self.assertRegex(component, r"<ActionButton\s+id=id\b")

    def test_oracle_anchors_black_white_and_alpha_composition(self):
        self.assertEqual(ratio(channels("#000000"), channels("#ffffff")), 21)
        self.assertAlmostEqual(ratio(channels("#777777"), channels("#ffffff")),
                               4.478089453577214)

    def test_actual_navigation_foreground_retains_text_contrast_under_hover(self):
        source = json.loads((ROOT / "packages/ui-system/shared/tokens.json").read_text())
        template = (ROOT / "packages/ui-system/leptos/foundation.css.in").read_text()
        block = re.search(r"\.cui-nav-item \{([^}]+)\}", template).group(1)
        foreground_variable = re.search(r"\bcolor: var\((--cantos-color-[a-z-]+)\)", block).group(1)
        self.assertIn("currentColor calc(var(--cantos-state-hover-opacity) * 100%)", template)
        alpha = source["tokens"]["state.hoverOpacity"]["value"]
        backgrounds = ("color.background", "color.surface", "color.surfaceContainer",
                       "color.surfaceContainerHigh")
        for theme, roles in source["themes"].items():
            variables = {"--cantos-" + re.sub(r"([A-Z])", r"-\1", role.replace(".", "-")).lower(): role
                         for role in roles}
            foreground = channels(roles[variables[foreground_variable]]["value"])
            for role in backgrounds:
                with self.subTest(theme=theme, background=role):
                    background = channels(roles[role]["value"])
                    hovered = tuple(alpha * ink + (1 - alpha) * paper
                                    for ink, paper in zip(foreground, background))
                    self.assertGreaterEqual(ratio(foreground, hovered), 4.5)
