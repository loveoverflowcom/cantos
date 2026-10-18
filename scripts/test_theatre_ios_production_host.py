"""Production iOS shipping-graph/source assertions; no Apple compiler or runtime claim."""
from pathlib import Path
import plistlib
import unittest
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
MOBILE = ROOT / "apps/cantos-theatre/mobile"


class TheatreIosProductionHostTest(unittest.TestCase):
    def test_shipping_target_imports_only_production_framework_and_catalog_owner(self):
        project = (MOBILE / "iosApp/Cantos.xcodeproj/project.pbxproj").read_text()
        swift = (MOBILE / "iosApp/Cantos/CantosApp.swift").read_text()
        self.assertIn("import CantosTheatre", swift)
        self.assertIn("IosCatalogOwner", swift)
        for source in (project, swift):
            self.assertNotIn("CantosTheatreDemo", source)
            self.assertNotIn("preview-sandbox", source)
            self.assertNotIn("mockAppContainer", source)
        self.assertIn("com.cantos.theatre", project)
        self.assertIn("shared/build/bin", project)
        scheme = ET.parse(MOBILE / "iosApp/Cantos.xcodeproj/xcshareddata/xcschemes/Cantos.xcscheme").getroot()
        reference = scheme.find("BuildAction/BuildActionEntries/BuildActionEntry/BuildableReference")
        self.assertEqual(reference.get("BlueprintName"), "Cantos")
        with (MOBILE / "iosApp/Cantos/Info.plist").open("rb") as source:
            self.assertEqual(plistlib.load(source), {"CantosApiOrigin": "$(CANTOS_API_ORIGIN)"})


if __name__ == "__main__":
    unittest.main()
