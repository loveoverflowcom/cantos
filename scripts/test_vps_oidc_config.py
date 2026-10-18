"""Pure browser OIDC reconciliation/privacy tests; no provider or host mutation."""

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "infra/vps/bin/cantos-oidc-config.py"
SPEC = importlib.util.spec_from_file_location("cantos_oidc_config", SCRIPT)
OIDC = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(OIDC)
CLIENT = "cantos-studio-web-bff"


def raw_client():
    return {"attrs": {"name": [CLIENT], "uuid": ["client-identity"],
                      "oauth2_rs_origin_landing": ["https://old-studio.example.test/"],
                      "oauth2_rs_origin": ["https://old-studio.example.test/api/v1/studio/auth/callback"],
                      "oauth2_strict_redirect_uri": ["false"],
                      "oauth2_rs_scope_map": ["studio-group: openid"],
                      "oauth2_rs_basic_secret": ["private-basic-secret-fixture"],
                      "key_internal_data": ["private-signing-key-fixture"],
                      "last_modified_cid": ["original-change-metadata"]}}


class VpsOidcConfiguration(unittest.TestCase):
    def setUp(self):
        self.raw = raw_client()
        self.before = OIDC.snapshot(CLIENT, self.raw)
        self.desired = OIDC.target(CLIENT, "https://studio.example.test",
                                   "https://api.example.test/api/v1/studio/auth/callback")

    def run_cli(self, *args, content=None):
        return subprocess.run([sys.executable, str(SCRIPT), *args], input=content,
                              capture_output=True, text=True, timeout=10)

    def test_pretty_snapshot_omits_secrets_and_retains_only_safe_fields(self):
        result = self.run_cli("snapshot", CLIENT, content=json.dumps(self.raw, indent=2))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout), self.before)
        self.assertEqual(set(self.before), OIDC.SAFE_KEYS)
        for private in ("private-basic-secret-fixture", "private-signing-key-fixture",
                        "oauth2_rs_basic_secret", "key_internal_data", "studio-group"):
            self.assertNotIn(private, result.stdout + result.stderr)

    def test_secret_key_scope_or_identity_change_is_detected_without_emitting_values(self):
        for key in ("name", "uuid", "oauth2_rs_scope_map", "oauth2_rs_basic_secret", "key_internal_data"):
            with self.subTest(key=key):
                changed = copy.deepcopy(self.raw)
                changed["attrs"][key] = ["changed-protected-value"]
                if key == "name":
                    with self.assertRaises(ValueError):
                        OIDC.snapshot(CLIENT, changed)
                else:
                    after = OIDC.snapshot(CLIENT, changed)
                    with self.assertRaises(ValueError):
                        OIDC.verify(self.before, after)

    def test_only_mutable_fields_and_change_metadata_may_change(self):
        changed = copy.deepcopy(self.raw)
        changed["attrs"].update({"oauth2_rs_origin_landing": [self.desired["landing_url"]],
                                 "oauth2_rs_origin": self.desired["redirect_urls"],
                                 "oauth2_strict_redirect_uri": ["true"],
                                 "last_modified_cid": ["updated-change-metadata"]})
        after = OIDC.snapshot(CLIENT, changed)
        OIDC.verify(self.before, after, self.desired)
        self.assertEqual(after["preserved_fingerprint"], self.before["preserved_fingerprint"])
        changed["attrs"]["unexpected_protected_attribute"] = ["new-value"]
        with self.assertRaises(ValueError):
            OIDC.verify(self.before, OIDC.snapshot(CLIENT, changed), self.desired)

    def test_attribute_and_set_order_do_not_change_protected_fingerprint(self):
        self.raw["attrs"]["oauth2_rs_scope_map"].append("other-group: openid")
        before = OIDC.snapshot(CLIENT, self.raw)
        changed = {"attrs": dict(reversed(list(self.raw["attrs"].items())))}
        changed["attrs"]["oauth2_rs_scope_map"] = list(reversed(changed["attrs"]["oauth2_rs_scope_map"]))
        self.assertEqual(before, OIDC.snapshot(CLIENT, changed))

    def test_redacted_secret_values_are_hashed_as_observed_not_unmasked(self):
        redacted = copy.deepcopy(self.raw)
        for key in ("oauth2_rs_basic_secret", "key_internal_data"):
            redacted["attrs"][key] = ["hidden"]
        observed = OIDC.snapshot(CLIENT, redacted)
        self.assertNotEqual(observed["preserved_fingerprint"], self.before["preserved_fingerprint"])
        # Equal masks establish only equal readable payload, never underlying secret equality.
        OIDC.verify(observed, OIDC.snapshot(CLIENT, copy.deepcopy(redacted)))
        result = self.run_cli("snapshot", CLIENT, content=json.dumps(redacted))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertNotIn("hidden", result.stdout)

    def test_plan_adds_new_redirect_before_removing_old_and_is_idempotent(self):
        self.assertEqual(OIDC.plan(CLIENT, self.before, self.desired), [
            ("set-landing-url", "https://studio.example.test/"),
            ("add-redirect-url", "https://api.example.test/api/v1/studio/auth/bearer/callback"),
            ("add-redirect-url", "https://api.example.test/api/v1/studio/auth/callback"),
            ("remove-redirect-url", "https://old-studio.example.test/api/v1/studio/auth/callback"),
            ("enable-strict-redirect-url",),
        ])
        after = {**self.desired, "preserved_fingerprint": self.before["preserved_fingerprint"]}
        self.assertEqual(OIDC.plan(CLIENT, after, self.desired), [])
        self.assertEqual(OIDC.plan(CLIENT, after, self.before)[-1], ("disable-strict-redirect-url",))
        with self.assertRaises(ValueError):
            OIDC.plan(CLIENT, after, {**self.desired, "strict_redirect": False})

    def test_targets_pin_only_the_exact_legacy_and_bearer_callbacks_per_surface(self):
        for surface in ("studio", "theatre"):
            with self.subTest(surface=surface):
                desired = OIDC.target(f"cantos-{surface}-web-bff", f"https://{surface}.example.test",
                                      f"https://api.example.test/api/v1/{surface}/auth/callback")
                self.assertEqual(desired["redirect_urls"], [
                    f"https://api.example.test/api/v1/{surface}/auth/bearer/callback",
                    f"https://api.example.test/api/v1/{surface}/auth/callback",
                ])
                self.assertTrue(desired["strict_redirect"])
                current = {**desired, "preserved_fingerprint": "a" * 64}
                self.assertEqual(OIDC.plan(desired["client"], current, desired), [])

    def test_legacy_only_or_additional_callbacks_do_not_satisfy_dual_readback(self):
        after = {**self.desired, "preserved_fingerprint": self.before["preserved_fingerprint"]}
        for callbacks in ([self.desired["redirect_urls"][-1]],
                          [*self.desired["redirect_urls"], "https://api.example.test/unreviewed"]):
            with self.subTest(callbacks=callbacks), self.assertRaises(ValueError):
                OIDC.verify(self.before, {**after, "redirect_urls": sorted(callbacks)}, self.desired)

    def test_verify_requires_exact_target_and_same_captured_client(self):
        after = {**self.desired, "preserved_fingerprint": self.before["preserved_fingerprint"]}
        OIDC.verify(self.before, after, self.desired)
        for key, bad in (("redirect_urls", self.before["redirect_urls"]),
                         ("strict_redirect", False), ("landing_url", self.before["landing_url"]),
                         ("client", "cantos-theatre-web-bff"), ("preserved_fingerprint", None)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                OIDC.verify(self.before, {**after, key: bad}, self.desired)

    def test_target_refuses_wrong_surface_and_nonexact_or_ambiguous_origins(self):
        for web in ("http://studio.example.test", "https://Studio.example.test", "https://studio.example.test/",
                    "https://studio.example.test:443", "https://user@studio.example.test", "https://studio.example.test/path",
                    "https://studio.example.test?q=1", "https://studio.example.test#x", "https://studio..example.test",
                    "https://-studio.example.test", "https://127.1", "https://api.0x7f", "https://studio.example.test\t"):
            with self.subTest(web=web), self.assertRaises(ValueError):
                OIDC.target(CLIENT, web, self.desired["redirect_urls"][0])
        for callback in ("https://api.example.test/api/v1/theatre/auth/callback",
                         "https://api.example.test/api/v1/studio/auth/bearer/callback",
                         "https://api.example.test/api/v1/studio/auth/../auth/callback",
                         "https://api.example.test/api/v1/studio/auth/callback/",
                         "https://api.example.test:443/api/v1/studio/auth/callback",
                         "https://user@api.example.test/api/v1/studio/auth/callback",
                         "https://api.example.test/api/v1/studio/auth/callback?x=1"):
            with self.subTest(callback=callback), self.assertRaises(ValueError):
                OIDC.target(CLIENT, "https://studio.example.test", callback)
        for client in ("cantos-studio", "other-client", "cantos-theatre-native", ""):
            with self.subTest(client=client), self.assertRaises(ValueError):
                OIDC.target(client, "https://studio.example.test", self.desired["redirect_urls"][0])

    def test_snapshot_and_cli_refuse_ambiguous_json_or_injected_values_privately(self):
        for content in ('{"attrs":{},"attrs":{}}', '{"private-basic-secret-fixture":', '[]',
                        json.dumps({"attrs": {**self.raw["attrs"], "oauth2_rs_origin": ["https://api.example.test/\nsecret"]}}),
                        json.dumps({"attrs": {**self.raw["attrs"], "oauth2_rs_origin": ["https://api.example.test/\x7f"]}}),
                        json.dumps({"attrs": {**self.raw["attrs"], "oauth2_strict_redirect_uri": ["TRUE"]}})):
            with self.subTest(content=content):
                result = self.run_cli("snapshot", CLIENT, content=content)
                self.assertEqual(result.returncode, 65)
                self.assertEqual(result.stdout, "")
                self.assertEqual(result.stderr, "cantos-oidc-config: configuration refused\n")

    def test_group_identity_and_membership_changes_are_detected_but_never_output(self):
        group = {"attrs": {"name": ["private-group"], "uuid": ["group-id"],
                            "member": ["private-member-a", "private-member-b"], "last_modified_cid": ["v1"]}}
        baseline = OIDC.group_fingerprint(group)
        reordered = copy.deepcopy(group)
        reordered["attrs"]["member"].reverse()
        reordered["attrs"]["last_modified_cid"] = ["v2"]
        self.assertEqual(baseline, OIDC.group_fingerprint(reordered))
        for key in ("name", "uuid", "member"):
            changed = copy.deepcopy(group)
            changed["attrs"][key] = ["different"]
            self.assertNotEqual(baseline, OIDC.group_fingerprint(changed))
        result = self.run_cli("group-fingerprint", content=json.dumps(group))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, baseline + "\n")
        self.assertNotIn("private", result.stdout + result.stderr)

    def test_secret_fingerprint_normalizes_only_json_representation_and_terminal_lf(self):
        secret = "private-secret_FIXTURE-123"
        expected = hashlib.sha256(secret.encode("ascii")).hexdigest()
        for content in (secret, secret + "\n", json.dumps(secret), json.dumps(secret) + "\n",
                        '"\\u0070rivate-secret_FIXTURE-123"'):
            with self.subTest(content=content):
                self.assertEqual(OIDC.secret_fingerprint(content), expected)
                result = self.run_cli("secret-fingerprint", content=content)
                self.assertEqual((result.returncode, result.stdout), (0, expected + "\n"), result.stderr)
                self.assertEqual(result.stderr, "")
                self.assertNotIn(secret, result.stdout + result.stderr)
        for length in (16, 4096):
            self.assertEqual(OIDC.secret_fingerprint("a" * length),
                             hashlib.sha256(("a" * length).encode("ascii")).hexdigest())

    def test_secret_fingerprint_refuses_masked_malformed_or_multiline_input_privately(self):
        secret = "private-secret_FIXTURE-123"
        for content in ("", "hidden", '"hidden"', "a" * 15, "a" * 4097, " " + secret,
                        secret + " ", secret + "\r\n", secret + "\n\n", secret + "\nother",
                        json.dumps(secret) + "\n\n", json.dumps(secret) + "\r\n",
                        json.dumps(secret + "\n"), json.dumps({"secret": secret}),
                        json.dumps([secret]), "null", '"unterminated', secret + "/"):
            with self.subTest(content=content):
                result = self.run_cli("secret-fingerprint", content=content)
                self.assertEqual((result.returncode, result.stdout), (65, ""))
                self.assertEqual(result.stderr, "cantos-oidc-config: configuration refused\n")
                self.assertNotIn(secret, result.stdout + result.stderr)

    def browser_input(self):
        return "".join(f"CANTOS_{surface}_{field}={value}\n" for surface, web in
                       (("THEATRE", "listen"), ("STUDIO", "studio")) for field, value in (
                           ("WEB_ORIGIN", f"https://{web}.example.test"),
                           ("API_ORIGIN", "https://api.example.test"),
                           ("OIDC_CALLBACK", f"https://api.example.test/api/v1/{surface.lower()}/auth/callback")))

    def test_merge_preserves_unrelated_bytes_and_adds_only_missing_browser_keys(self):
        edge = "# operator text\nCANTOS_API_HOST=api.example.test\nPRIVATE=value with spaces\n\nCANTOS_STUDIO_WEB_ORIGIN=https://old.example.test\nNO_FINAL=newline"
        result = OIDC.merge_browser(self.browser_input(), edge)
        self.assertTrue(result.startswith(edge.replace("https://old.example.test", "https://studio.example.test")))
        self.assertEqual(OIDC.env_assignments(result), {
            **OIDC.env_assignments(edge), **OIDC.env_assignments(self.browser_input(), True)})
        self.assertEqual(OIDC.merge_browser(self.browser_input(), result), result)

    def test_merge_refuses_incomplete_duplicate_quoted_exported_and_cross_surface_input(self):
        edge = "CANTOS_API_HOST=api.example.test\nKEEP=private-value\n"
        valid = self.browser_input()
        bad_inputs = [valid.rsplit("\n", 2)[0], valid + valid.splitlines()[0] + "\n",
                      valid + "UNRELATED=value\n", valid.replace("https://listen.example.test", '"https://listen.example.test"'),
                      "export " + valid, " " + valid, valid.replace("\n", "\r\n"), valid.replace("\n", "\u2028"),
                      valid.replace("https://api.example.test", "https://wrong.example.test"),
                      valid.replace("/theatre/auth/callback", "/studio/auth/callback"),
                      valid.replace("https://studio.example.test", "https://listen.example.test")]
        for inputs in bad_inputs:
            with self.subTest(inputs=inputs), self.assertRaises(ValueError):
                OIDC.merge_browser(inputs, edge)
        for bad_edge in (edge + "CANTOS_API_HOST=other.example.test\n", edge + "KEEP=duplicate\n",
                         edge + "lower=value\nlower=duplicate\n",
                         edge.replace("CANTOS_API_HOST", " CANTOS_API_HOST"), "KEEP=value\n"):
            with self.subTest(edge=bad_edge), self.assertRaises(ValueError):
                OIDC.merge_browser(valid, bad_edge)

    def test_cli_plan_verify_and_merge_use_files_and_never_write_them(self):
        with tempfile.TemporaryDirectory() as directory:
            paths = [Path(directory) / name for name in ("before", "desired", "after", "input", "edge")]
            contents = [json.dumps(self.before), json.dumps(self.desired),
                        json.dumps({**self.desired, "preserved_fingerprint": self.before["preserved_fingerprint"]}),
                        self.browser_input(), "CANTOS_API_HOST=api.example.test\nKEEP=untouched\n"]
            for path, content in zip(paths, contents):
                path.write_text(content)
            result = self.run_cli("plan", CLIENT, str(paths[0]), str(paths[1]))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout.splitlines()[-1], "enable-strict-redirect-url")
            self.assertTrue(all(len(line.split("\t")) in (1, 2) for line in result.stdout.splitlines()))
            result = self.run_cli("verify", str(paths[0]), str(paths[2]), str(paths[1]))
            self.assertEqual((result.returncode, result.stdout), (0, ""), result.stderr)
            result = self.run_cli("merge-browser", str(paths[3]), str(paths[4]))
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(result.stdout, OIDC.merge_browser(contents[3], contents[4]))
            self.assertEqual([path.read_text() for path in paths], contents)


if __name__ == "__main__":
    unittest.main()
