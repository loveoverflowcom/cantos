"""Repository-path regressions only; no Caddy/Firebase/cloud runtime acceptance."""

from pathlib import Path
import json
import re
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


class InfrastructureLayout(unittest.TestCase):
    def test_only_infra_owns_tracked_edge_and_hosting_assets(self):
        self.assertTrue((ROOT / "infra/edge/Caddyfile").is_file())
        self.assertTrue((ROOT / "infra/edge/duck.sh.example").is_file())
        self.assertTrue((ROOT / "infra/firebase/hosting/firebaserc.example").is_file())
        # Ignored operator files may remain privately at an old path; they are not migrated.
        tracked = subprocess.run(
            ["git", "ls-files", "services/edge", "services/firebase-hosting"],
            cwd=ROOT, check=True, capture_output=True, text=True,
        ).stdout
        self.assertEqual(tracked, "")

    def test_hosting_diagnostic_paths_resolve_from_the_relocated_config(self):
        directory = ROOT / "infra/firebase/hosting"
        policy = json.loads((directory / "firebase.json").read_text())
        self.assertEqual(set(policy), {"hosting", "emulators"})
        self.assertEqual([entry["target"] for entry in policy["hosting"]], ["theatre", "studio"])
        for entry in policy["hosting"]:
            with self.subTest(target=entry["target"]):
                self.assertEqual(
                    (directory / entry["public"]).resolve(),
                    ROOT / f'apps/cantos-{entry["target"]}/web/dist',
                )

    def test_edge_imports_are_in_file_and_container_roots_stay_absolute(self):
        source = (ROOT / "infra/edge/Caddyfile").read_text()
        snippets = set(re.findall(r"^\(([a-z_]+)\) \{", source, re.MULTILINE))
        imports = re.findall(r"^\s*import (\S+)", source, re.MULTILINE)
        self.assertTrue(imports)
        self.assertTrue(set(imports).issubset(snippets), "no relative external includes to rebase")
        # Firebase owns static bytes; the edge no longer mounts local Web dist directories.
        self.assertNotIn("/srv/theatre", source)
        self.assertNotIn("/srv/studio", source)
        unit = (ROOT / "infra/vps/quadlet/cantos-edge.container").read_text()
        for mount in [
            "/root/cantos-services/edge:/etc/caddy:ro",
            "cantos-edge-data:/data",
            "cantos-edge-config:/config",
        ]:
            self.assertIn(f"Volume={mount}", unit)
        self.assertNotIn("/edge/Caddyfile:/etc/caddy/Caddyfile", unit,
                         "directory mount must observe replaced files on reload")

    def test_private_mapping_logs_and_generated_staging_are_ignored(self):
        paths = [
            "infra/firebase/hosting/.firebaserc",
            "services/firebase-hosting/.firebaserc",
            "infra/firebase/hosting/firebase-debug.log",
            "infra/firebase/hosting/firebase-debug.1.log",
            "infra/edge/env.local",
            "infra/edge/data/certificate.pem",
            "artifacts/hosting/RUN-ID/firebase.json",
        ]
        result = subprocess.run(
            ["git", "check-ignore", "--no-index", *paths],
            cwd=ROOT, check=True, capture_output=True, text=True,
        )
        self.assertEqual(result.stdout.splitlines(), paths)


if __name__ == "__main__":
    unittest.main()
