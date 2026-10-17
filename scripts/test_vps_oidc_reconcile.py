"""Exercise existing-account OIDC shell effects with a recording provider double."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
HOST = (ROOT / "infra/vps/bin/cantos-vps").read_text().removesuffix('\nmain "$@"\n')
TOOLS = ROOT / "infra/vps/bin"
PROVIDER = r'''
import json,os,sys
from pathlib import Path
root=Path(os.environ['TEST_OIDC_ROOT'])
state=json.loads((root/'provider.json').read_text())
args=sys.argv[1:]
with (root/'events').open('a') as output:output.write(' '.join(args)+'\n')
if args[:3]==['kanidm','group','get']:
 print(json.dumps({'attrs':{'name':[args[3]],'uuid':['fixture-group-id'],'member':['fixture-member']}}))
 sys.exit(0)
assert args[:3]==['kanidm','system','oauth2'], 'unexpected provider boundary'
action,client=args[3:5]
if action=='get':
 if client in state:print(json.dumps({'attrs':state[client]}))
 sys.exit(0)
if action=='show-basic-secret':
 print('fixture-client-secret-never-output')
 sys.exit(0)
assert action in ['set-landing-url','add-redirect-url','remove-redirect-url',
 'enable-strict-redirect-url','disable-strict-redirect-url'], 'account/credential mutation forbidden'
if action==os.environ.get('TEST_OIDC_FAIL_ACTION') and not (root/'failed-once').exists():
 (root/'failed-once').write_text('1')
 sys.exit(1)
attrs=state[client]
if action=='set-landing-url':attrs['oauth2_rs_origin_landing']=[args[5]]
elif action=='add-redirect-url':attrs['oauth2_rs_origin']=sorted(set(attrs['oauth2_rs_origin']+[args[5]]))
elif action=='remove-redirect-url':attrs['oauth2_rs_origin'].remove(args[5])
else:attrs['oauth2_strict_redirect_uri']=['true' if action.startswith('enable') else 'false']
(root/'provider.json').write_text(json.dumps(state))
'''


class VpsOidcReconcileTests(unittest.TestCase):
    def test_nested_operations_reuse_the_same_lock_descriptor(self):
        with tempfile.TemporaryDirectory() as directory:
            lock = Path(directory) / "operation.lock"
            script = HOST.replace("readonly LOCK=/run/lock/cantos-vps.lock", f"readonly LOCK={lock}")
            script += '''
locks=0
flock() { locks=$((locks + 1)); command flock "$@"; }
with_lock
with_lock
[[ $locks -eq 1 ]]
python3 - "$LOCK" <<'PY'
import fcntl,sys
with open(sys.argv[1],'a') as file:
 try:fcntl.flock(file,fcntl.LOCK_EX|fcntl.LOCK_NB)
 except BlockingIOError:sys.exit(0)
 sys.exit(1)
PY
'''
            result = subprocess.run(["bash"], input=script, capture_output=True,
                                    text=True, check=False)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_pending_cutover_blocks_mutation_before_provider_or_runtime_effects(self):
        for command in ("deploy", "rollback", "install", "bootstrap", "identity", "backup", "restore"):
            with self.subTest(command=command), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "state").mkdir()
                (root / "state/epoch2-cutover.pending").write_text("pending-recovery")
                result = subprocess.run(["bash"], input=HOST + f"\nmain {command}\n",
                    capture_output=True, text=True, check=False,
                    env=dict(os.environ, CANTOS_VPS_ROOT=directory))
                self.assertEqual(result.returncode, 75, result.stderr)
                self.assertIn("cutover pending", result.stderr)

    def fixture(self, root):
        config = root / "config"
        config.mkdir()
        (root / "secrets").mkdir()
        (root / "secrets/kanidm-idm-admin.password").write_text("fixture-admin-password")
        (root / "quadlet").mkdir()
        edge = ["CANTOS_IDM_HOST=idm.example.test", "CANTOS_API_HOST=api.example.test"]
        provider = {}
        originals = {}
        for surface in ("theatre", "studio"):
            web = f"https://{surface}.web.app"
            callback = f"https://api.example.test/api/v1/{surface}/auth/callback"
            edge += [f"CANTOS_{surface.upper()}_WEB_ORIGIN={web}",
                     f"CANTOS_{surface.upper()}_API_ORIGIN=https://api.example.test",
                     f"CANTOS_{surface.upper()}_OIDC_CALLBACK={callback}"]
            name = f"cantos-{surface}-web-bff"
            provider[name] = {
                "name": [name], "uuid": [f"fixture-{surface}-id"],
                "oauth2_rs_origin_landing": [f"https://old-{surface}.example.test/"],
                "oauth2_rs_origin": [f"https://old-{surface}.example.test/callback"],
                "oauth2_strict_redirect_uri": ["true"],
                "oauth2_rs_scope_map": [f"cantos_{surface}: {{\"openid\"}}"],
                "oauth2_rs_basic_secret": ["fixture-client-secret-never-output"],
                "key_internal_data": ["fixture-signing-key-never-output"],
            }
            oidc = f"CANTOS_{surface.upper()}_OIDC_ISSUER=https://idm.example.test/oauth2/openid/{name}\n"
            if surface == "studio":
                oidc += 'CANTOS_STUDIO_ADMIN_SUBJECTS=["00000000-0000-4000-8000-000000000001"]\n'
            (config / f"api-{surface}-oidc.env").write_text(oidc)
            originals[f"api-{surface}.env"] = f"previous-{surface}-runtime-config\n"
            (config / f"api-{surface}.env").write_text(originals[f"api-{surface}.env"])
        (config / "edge.env").write_text("\n".join(edge) + "\n")
        (root / "provider.json").write_text(json.dumps(provider))
        (root / "provider.py").write_text(PROVIDER)
        return provider, originals

    def run_case(self, *, failure=None, validator_exit=False, missing_client=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            provider, originals = self.fixture(root)
            if missing_client:
                state = dict(provider)
                del state["cantos-studio-web-bff"]
                (root / "provider.json").write_text(json.dumps(state))
            env = dict(os.environ, CANTOS_VPS_ROOT=directory,
                       TEST_OIDC_ROOT=directory, CANTOS_VPS_HELPERS=str(TOOLS))
            if failure:
                env["TEST_OIDC_FAIL_ACTION"] = failure
            script = HOST.replace("readonly QUADLET_DIR=/etc/containers/systemd",
                                  f"readonly QUADLET_DIR={root}/quadlet")
            script += '''
require_root() { :; }
with_lock() { :; }
podman() { [[ $1 == secret && $2 == inspect ]]; }
kanidm_cli() { shift; python3 "$TEST_OIDC_ROOT/provider.py" "$@"; }
source "$CANTOS_VPS_HELPERS/cantos-oidc-reconcile"
'''
            if validator_exit:
                script += 'validate_runtime_browser_configuration() { exit 65; }\n'
            script += "reconcile_existing_identity\n"
            result = subprocess.run(["bash"], input=script, capture_output=True,
                                    text=True, env=env, check=False)
            after = json.loads((root / "provider.json").read_text())
            files = {p.name: p.read_text() for p in (root / "config").iterdir()}
            events = (root / "events").read_text().splitlines()
            return result, provider, after, originals, files, events

    def test_existing_clients_reconciled_without_accounts_groups_secrets_or_roster_changes(self):
        result, before, after, _, files, events = self.run_case()
        self.assertEqual(result.returncode, 0, result.stderr)
        for surface in ("theatre", "studio"):
            client = f"cantos-{surface}-web-bff"
            self.assertEqual(after[client]["oauth2_rs_origin"],
                             [f"https://api.example.test/api/v1/{surface}/auth/callback"])
            self.assertEqual(after[client]["oauth2_rs_origin_landing"], [f"https://{surface}.web.app/"])
            for key in ("oauth2_rs_basic_secret", "key_internal_data", "oauth2_rs_scope_map", "uuid"):
                self.assertEqual(after[client][key], before[client][key])
            self.assertIn(f"CANTOS_BROWSER_API_SURFACE={surface}\n", files[f"api-{surface}.env"])
        self.assertIn("oidc-callbacks.env", files)
        self.assertFalse(any("person " in e or "group add" in e or "reset" in e for e in events))
        self.assertNotIn("fixture-client-secret", result.stdout + result.stderr)
        self.assertNotIn("fixture-signing-key", result.stdout + result.stderr)

    def test_partial_provider_failure_restores_previous_clients_and_config_without_receipt(self):
        result, before, after, originals, files, _ = self.run_case(failure="remove-redirect-url")
        self.assertEqual(result.returncode, 70, result.stderr)
        self.assertEqual(after, before)
        for name, value in originals.items():
            self.assertEqual(files[name], value)
        self.assertNotIn("oidc-callbacks.env", files)

    def test_exit_in_final_host_validator_is_contained_and_rolls_back_provider_and_config(self):
        result, before, after, originals, files, _ = self.run_case(validator_exit=True)
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertEqual(after, before)
        for name, value in originals.items():
            self.assertEqual(files[name], value)
        self.assertNotIn("oidc-callbacks.env", files)

    def test_missing_existing_client_refuses_before_any_provider_mutation(self):
        result, _, _, _, files, events = self.run_case(missing_client=True)
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertTrue(all(" get " in event or "show-basic-secret" in event for event in events), events)
        self.assertNotIn("oidc-callbacks.env", files)


if __name__ == "__main__":
    unittest.main()
