"""VPS browser-input regressions using the real host functions and private fixtures.

No Podman, systemd, network, identity or production configuration is touched.
"""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "infra/vps/bin/cantos-vps"


class VpsBrowserConfiguration(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        for directory in ("config", "kanidm", "runtime/kanidm", "quadlet", "edge", "bundle/infra/edge"):
            (self.root / directory).mkdir(parents=True)
        (self.root / "edge/Caddyfile").write_text("previous edge\n")
        (self.root / "bundle/infra/edge/Caddyfile").write_text("reviewed edge\n")
        (self.root / "runtime/kanidm/server.toml.template").write_text('domain = "@IDM_HOST@"\n')
        self.inputs = {
            "CANTOS_THEATRE_HOST": "legacy.example.test",
            "CANTOS_STUDIO_HOST": "legacy-studio.example.test",
            "CANTOS_API_HOST": "api.example.test",
            "CANTOS_IDM_HOST": "idm.example.test",
            "CANTOS_THEATRE_FIREBASE_SITE": "theatre-fixture",
            "CANTOS_STUDIO_FIREBASE_SITE": "studio-fixture",
            "CANTOS_THEATRE_WEB_ORIGIN": "https://listen.example.test",
            "CANTOS_STUDIO_WEB_ORIGIN": "https://studio.example.test",
            "CANTOS_THEATRE_API_ORIGIN": "https://api.example.test",
            "CANTOS_STUDIO_API_ORIGIN": "https://api.example.test",
            "CANTOS_THEATRE_OIDC_CALLBACK": "https://api.example.test/api/v1/theatre/auth/callback",
            "CANTOS_STUDIO_OIDC_CALLBACK": "https://api.example.test/api/v1/studio/auth/callback",
        }
        self.environment = {**os.environ, "CANTOS_VPS_ROOT": str(self.root)}
        # Relocate the one fixed system path. All production functions remain unchanged.
        source = SCRIPT.read_text()
        self.assertEqual(source.count("readonly QUADLET_DIR=/etc/containers/systemd"), 1)
        source = source.replace("readonly QUADLET_DIR=/etc/containers/systemd",
                                f"readonly QUADLET_DIR={self.root}/quadlet")
        self.assertTrue(source.endswith('main "$@"\n'))
        self.library = source.removesuffix('main "$@"\n')

    def run_host(self, commands):
        (self.root / "config/edge.env").write_text(
            "".join(f"{key}={value}\n" for key, value in self.inputs.items()))
        return subprocess.run(["bash", "-c", self.library + commands],
                              env=self.environment, capture_output=True, text=True, timeout=10)

    def test_render_uses_explicit_surface_origins_and_api_callback_host(self):
        result = self.run_host("render_configuration\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        for surface, web in (("theatre", "listen"), ("studio", "studio")):
            self.assertEqual((self.root / f"config/api-{surface}.env").read_text(),
                             "CANTOS_ENV=production\n"
                             "CANTOS_BIND_ADDRESS=0.0.0.0:8080\n"
                             f"CANTOS_BROWSER_API_SURFACE={surface}\n"
                             f"CANTOS_WEB_ORIGIN=https://{web}.example.test\n"
                             "CANTOS_API_ORIGIN=https://api.example.test\n")

    def test_wrong_surface_callback_rejects_before_replacing_any_configuration(self):
        original = b"operator-config\n"
        for surface in ("theatre", "studio"):
            (self.root / f"config/api-{surface}.env").write_bytes(original)
        self.inputs["CANTOS_STUDIO_OIDC_CALLBACK"] = self.inputs["CANTOS_THEATRE_OIDC_CALLBACK"]
        result = self.run_host("render_configuration\n")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("CANTOS_STUDIO_OIDC_CALLBACK must equal", result.stderr)
        for surface in ("theatre", "studio"):
            self.assertEqual((self.root / f"config/api-{surface}.env").read_bytes(), original)
        self.assertFalse((self.root / "kanidm/server.toml").exists())

    def test_api_origin_must_match_the_configured_ingress(self):
        self.inputs["CANTOS_THEATRE_API_ORIGIN"] = "https://other.example.test"
        result = self.run_host("render_configuration\n")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("CANTOS_THEATRE_API_ORIGIN must match CANTOS_API_HOST", result.stderr)
        self.assertFalse((self.root / "config/api-theatre.env").exists())

    def test_api_alias_is_optional_but_never_a_wildcard_duplicate_or_primary(self):
        for alias in ("*.example.test", "api.example.test", "",
                      "other.example.test\nCANTOS_API_ALIAS_HOST=third.example.test"):
            with self.subTest(alias=alias):
                self.inputs["CANTOS_API_ALIAS_HOST"] = alias
                result = self.run_host("validate_browser_configuration\n")
                self.assertEqual(result.returncode, 65, result.stderr)
        self.inputs["CANTOS_API_ALIAS_HOST"] = "legacy-api.example.test"
        self.assertEqual(self.run_host("validate_browser_configuration\n").returncode, 0)

    def test_browser_reload_validates_before_restart_and_cannot_touch_sql_or_secrets(self):
        commands = '''
require_root() { :; }
with_lock() { :; }
unit_active() { return 0; }
systemctl() { printf 'systemctl %s\\n' "$*"; }
podman() { printf 'podman %s\\n' "$*" >>"$ROOT/actions"; }
validate_runtime_browser_configuration() { return 65; }
main reload-browser "$ROOT/bundle"
'''
        result = self.run_host(commands)
        self.assertEqual(result.returncode, 65)
        self.assertEqual(result.stdout, "")
        self.assertFalse((self.root / "actions").exists())
        commands = commands.replace('return 65;', 'return 0;')
        result = self.run_host(commands)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            'systemctl restart cantos-edge.service',
            'systemctl restart cantos-api-theatre.service',
            'systemctl restart cantos-api-studio.service',
        ])
        self.assertEqual((self.root / "actions").read_text().splitlines(), [
            f'podman exec --env-file {self.root}/config/edge.env cantos-edge caddy validate --config /etc/caddy/Caddyfile.browser-candidate --adapter caddyfile',
        ])
        self.assertEqual((self.root / "edge/Caddyfile").read_text(), "reviewed edge\n")
        self.assertEqual((self.root / "edge/Caddyfile.before-browser-reload").read_text(), "previous edge\n")

    def test_invalid_candidate_never_replaces_edge_or_restarts_services(self):
        result = self.run_host('''
require_root() { :; }
with_lock() { :; }
unit_active() { return 0; }
validate_runtime_browser_configuration() { return 0; }
systemctl() { printf 'unexpected restart\\n'; return 99; }
podman() { return 1; }
reload_browser "$ROOT/bundle"
''')
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual((self.root / "edge/Caddyfile").read_text(), "previous edge\n")
        self.assertFalse((self.root / "edge/Caddyfile.browser-candidate").exists())

    def test_legacy_host_input_does_not_substitute_for_a_missing_web_origin(self):
        del self.inputs["CANTOS_THEATRE_WEB_ORIGIN"]
        result = self.run_host("render_configuration\n")
        self.assertEqual(result.returncode, 66, result.stderr)
        self.assertIn("must define CANTOS_THEATRE_WEB_ORIGIN exactly once", result.stderr)
        self.assertFalse((self.root / "config/api-theatre.env").exists())

    def test_malformed_or_normalized_origins_are_refused_without_rendering(self):
        for origin in ("http://listen.example.test", "https://Listen.example.test",
                       "https://listen.example.test/", "https://listen.example.test:443",
                       "https://user@listen.example.test", "https://listen.example.test/path",
                       "https://listen.example.test?q=1", "https://listen.example.test#x",
                       "https://listen..example.test", "https://-listen.example.test",
                       "https://listen.example.test\r", "https://127.1", "https://api.123",
                       "https://0x7f.1", "https://api.0x7f", "https://api.0x"):
            with self.subTest(origin=origin):
                self.inputs["CANTOS_THEATRE_WEB_ORIGIN"] = origin
                result = self.run_host("render_configuration\n")
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertIn("CANTOS_THEATRE_WEB_ORIGIN must be an exact HTTPS origin", result.stderr)
                self.assertFalse((self.root / "config/api-theatre.env").exists())

    def test_duplicate_origin_key_is_not_silently_selected(self):
        self.inputs["CANTOS_THEATRE_WEB_ORIGIN"] += "\nCANTOS_THEATRE_WEB_ORIGIN=https://other.example.test"
        result = self.run_host("render_configuration\n")
        self.assertEqual(result.returncode, 66, result.stderr)
        self.assertIn("must define CANTOS_THEATRE_WEB_ORIGIN exactly once", result.stderr)

    def test_web_surfaces_cannot_share_an_origin(self):
        self.inputs["CANTOS_STUDIO_WEB_ORIGIN"] = self.inputs["CANTOS_THEATRE_WEB_ORIGIN"]
        result = self.run_host("render_configuration\n")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("Theatre and Studio must have distinct Web origins", result.stderr)

    def test_deploy_and_rollback_refuse_stale_env_before_external_effects(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        file = self.root / "config/api-studio.env"
        file.write_text(file.read_text().replace("https://studio.example.test", "https://stale.example.test"))
        original = file.read_bytes()
        for command in ("deploy fixture", "rollback"):
            with self.subTest(command=command):
                result = self.run_host("""
require_root() { :; }
with_lock() { :; }
check_revision() { printf image >>"$ROOT/effects"; }
podman() { printf podman >>"$ROOT/effects"; }
systemctl() { printf systemctl >>"$ROOT/effects"; }
""" + command + "\n")
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertIn("has stale CANTOS_WEB_ORIGIN", result.stderr)
                self.assertFalse((self.root / "effects").exists())
                self.assertEqual(file.read_bytes(), original)

    def test_active_oidc_requires_current_configured_callback_receipt(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        (self.root / "config/api-studio-oidc.env").write_text("CANTOS_STUDIO_OIDC_ISSUER=https://idm.example.test/fixture\n")
        result = self.run_host("validate_runtime_browser_configuration\n")
        self.assertEqual(result.returncode, 66, result.stderr)
        self.assertIn("oidc-callbacks.env must define CANTOS_STUDIO_OIDC_CALLBACK exactly once", result.stderr)
        receipt = self.root / "config/oidc-callbacks.env"
        receipt.write_text("CANTOS_STUDIO_OIDC_CALLBACK=https://legacy-studio.example.test/api/v1/studio/auth/callback\n")
        result = self.run_host("validate_runtime_browser_configuration\n")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("has stale CANTOS_STUDIO_OIDC_CALLBACK", result.stderr)
        receipt.write_text("CANTOS_STUDIO_OIDC_CALLBACK=https://api.example.test/api/v1/studio/auth/callback\n")
        result = self.run_host("validate_runtime_browser_configuration\n")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_identity_clients_receive_web_origins_and_api_surface_callbacks(self):
        result = self.run_host("""
ensure_client() { printf '%s|%s|%s\\n' "$2" "$4" "$5"; }
validate_browser_configuration
configure_browser_clients idm.example.test
""")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.splitlines(), [
            "cantos-studio-web-bff|https://studio.example.test|https://api.example.test/api/v1/studio/auth/callback",
            "cantos-theatre-web-bff|https://listen.example.test|https://api.example.test/api/v1/theatre/auth/callback",
        ])

    def write_dropin(self, content):
        directory = self.root / "quadlet/cantos-api-studio.container.d"
        directory.mkdir(exist_ok=True)
        (directory / "20-fixture.conf").write_text(content)

    def test_later_oidc_environment_cannot_override_base_browser_keys(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        supplemental = self.root / "config/api-studio-oidc.env"
        self.write_dropin(f"[Container]\nEnvironmentFile={supplemental}\n")
        for key in ("CANTOS_ENV", "CANTOS_BIND_ADDRESS", "CANTOS_WEB_ORIGIN",
                    "CANTOS_API_ORIGIN", "CANTOS_BROWSER_API_SURFACE"):
            for assignment in (f"{key}=stale", f"  {key} =stale"):
                with self.subTest(assignment=assignment):
                    supplemental.write_text(f"{assignment}\n")
                    result = self.run_host("validate_runtime_browser_configuration\n")
                    self.assertEqual(result.returncode, 65, result.stderr)
                    self.assertIn("overrides protected API browser configuration", result.stderr)

    def test_unsupported_override_directives_are_refused(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        for directive in ("Environment=SAFE=value CANTOS_WEB_ORIGIN=https://stale.example.test",
                          "PodmanArgs=--env CANTOS_API_ORIGIN=https://stale.example.test",
                          "EnvironmentHost=CANTOS_WEB_ORIGIN", "ExecStart=/bin/other-api",
                          "EnvironmentFile=-/missing/optional.env",
                          "Secret=fixture,type=env,target=CANTOS_API_ORIGIN"):
            with self.subTest(directive=directive):
                self.write_dropin(f"[Container]\n{directive}\n")
                result = self.run_host("validate_runtime_browser_configuration\n")
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertIn("unsupported API", result.stderr)

    def test_registration_environment_file_remains_supported_without_browser_overrides(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        supplemental = self.root / "config/registration-fixture.env"
        supplemental.write_text("# Literal supplemental values\n\n"
                                "CANTOS_THEATRE_REGISTRATION_ENABLED=false\n"
                                'CANTOS_STUDIO_ADMIN_SUBJECTS=["fixture-a", "fixture-b"]\n')
        self.write_dropin(f"[Container]\nEnvironmentFile={supplemental}\n")
        result = self.run_host("validate_runtime_browser_configuration\n")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_direct_registration_and_unrelated_unit_options_remain_supported(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        self.write_dropin("[Unit]\nAfter=fixture.service\n[Container]\n"
                          "Environment=CANTOS_THEATRE_REGISTRATION_ENABLED=false\n"
                          "Volume=/private/fixture:/run/secrets/fixture:ro\n"
                          "[Service]\nRestartSec=5\n")
        result = self.run_host("validate_runtime_browser_configuration\n")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_direct_environment_cannot_override_protected_browser_key(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        self.write_dropin("[Container]\nEnvironment=CANTOS_API_ORIGIN=https://stale.example.test\n")
        result = self.run_host("validate_runtime_browser_configuration\n")
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("overrides protected API browser configuration", result.stderr)

    def test_supplemental_environment_refuses_host_forwarding_and_nonassignment_forms(self):
        self.assertEqual(self.run_host("render_configuration\n").returncode, 0)
        supplemental = self.root / "config/supplemental-fixture.env"
        self.write_dropin(f"[Container]\nEnvironmentFile={supplemental}\n")
        for line in ("CANTOS_WEB_ORIGIN", "PATH", "CANTOS_*", "export SAFE=value", "SAFE*=value"):
            with self.subTest(line=line):
                supplemental.write_text(f"{line}\n")
                result = self.run_host("validate_runtime_browser_configuration\n")
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertIn("unsupported supplemental API env assignment", result.stderr)


if __name__ == "__main__":
    unittest.main()
