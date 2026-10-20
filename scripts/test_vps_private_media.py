"""Real VPS storage/promotion functions with relocated paths and OS boundary doubles.

Files, modes, TOML, browser validation and bundle rotation are real. UID/GID reports,
privileged install, image loading, activation and systemd are doubles; no VPS is contacted.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


REPOSITORY = Path(__file__).resolve().parents[1]
REVISION = "a" * 40


class PrivateMediaPromotionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.tree = self.root / "incoming" / REVISION / ("b" * 32) / "tree"
        shutil.copytree(REPOSITORY / "infra/vps", self.tree / "infra/vps")
        (self.tree / "infra/edge").mkdir(parents=True)
        (self.tree / "infra/edge/Caddyfile").write_text("candidate edge\n")
        (self.tree / "REVISION").write_text(REVISION + "\n")
        for directory in ("config", "runtime", "runtime.previous", "quadlet", "systemd", "edge", "sbin", "state"):
            (self.root / directory).mkdir(mode=0o700)
        (self.root / "runtime/REVISION").write_text("current\n")
        (self.root / "runtime.previous/REVISION").write_text("recovery\n")
        (self.root / "sbin/cantos-vps").write_text("installed helper\n")
        (self.root / "edge/Caddyfile").write_text("installed edge\n")
        (self.root / "quadlet/cantos-api-studio.container").write_text("legacy unit\n")
        (self.root / "config/edge.env").write_text(
            "CANTOS_API_HOST=api.example.test\n"
            "CANTOS_THEATRE_WEB_ORIGIN=https://listen.example.test\n"
            "CANTOS_STUDIO_WEB_ORIGIN=https://studio.example.test\n"
            "CANTOS_THEATRE_API_ORIGIN=https://api.example.test\n"
            "CANTOS_STUDIO_API_ORIGIN=https://api.example.test\n"
            "CANTOS_THEATRE_OIDC_CALLBACK=https://api.example.test/api/v1/theatre/auth/callback\n"
            "CANTOS_STUDIO_OIDC_CALLBACK=https://api.example.test/api/v1/studio/auth/callback\n")
        self.parent = self.root / "media"
        self.media = self.parent / "private"
        self.profile = self.root / "config/storage.toml"
        self.default = self.tree / "infra/vps/config/storage.filesystem.toml"
        self.default.write_text(self.default.read_text().replace('/var/lib/cantos/private', str(self.media)))
        source = (REPOSITORY / "infra/vps/bin/cantos-vps").read_text()
        for old, new in (
            ("readonly MEDIA_STORAGE_PARENT=/var/lib/cantos", f"readonly MEDIA_STORAGE_PARENT={self.parent}"),
            ("readonly QUADLET_DIR=/etc/containers/systemd", f"readonly QUADLET_DIR={self.root}/quadlet"),
            ("readonly SYSTEMD_DIR=/etc/systemd/system", f"readonly SYSTEMD_DIR={self.root}/systemd"),
            ("/usr/local/sbin/cantos-vps", f"{self.root}/sbin/cantos-vps"),
        ):
            self.assertIn(old, source)
            source = source.replace(old, new)
        self.library = source.removesuffix('main "$@"\n')
        self.environment = dict(os.environ, CANTOS_VPS_ROOT=str(self.root), TEST_TREE=str(self.tree),
                                TEST_REVISION=REVISION)

    def run_host(self, body, *, mock_activation=True, **environment):
        doubles = r'''
event() { printf '%s\n' "$*" >>"$ROOT/events"; }
require_root() { :; }
with_lock() { :; }
uname() { printf 'Linux x86_64\n'; }
sha256sum() { event checksum; }
zstd() { printf 'fixture image'; }
podman() {
  if [[ $1 == load ]]; then cat >/dev/null; event load; return; fi
  case "$4" in
    '{{.Id}}') printf '%064d\n' 0 ;;
    '{{.Os}}/{{.Architecture}}') printf 'linux/amd64\n' ;;
    *org.opencontainers.image.revision*) printf '%s\n' "$TEST_REVISION" ;;
    *schema-epoch*) printf '2\n' ;;
    *) return 91 ;;
  esac
}
unit_active() { [[ $1 == cantos-postgres ]]; }
preflight_schema() { event schema; }
systemctl() { event "systemctl $*"; }
bootstrap() { event forbidden-bootstrap; return 99; }
stat() {
  python3 - "$MEDIA_STORAGE_ROOT" "${@: -1}" <<'PY'
import os, pathlib, stat, sys
path = pathlib.Path(sys.argv[2])
owner = '10001:10001' if str(path) == sys.argv[1] else '0:0'
if os.environ.get('TEST_WRONG_OWNER') == str(path):
    owner = '0:0' if owner != '0:0' else '10001:10001'
print(f'{owner}:{stat.S_IMODE(path.stat().st_mode):o}')
PY
}
install() {
  event "install $*"
  python3 - "$@" <<'PY'
import os, pathlib, shutil, sys
args = iter(sys.argv[1:]); paths = []; mode = 0o755; directory = False
for arg in args:
    if arg in ('-o', '-g'):
        next(args)  # Ownership is simulated by stat above, never changed on the host.
    elif arg == '-m':
        mode = int(next(args), 8)
    elif arg == '-d':
        directory = True
    elif arg != '-D':
        paths.append(pathlib.Path(arg))
if directory:
    for path in paths:
        path.mkdir(parents=True, exist_ok=True); path.chmod(mode)
else:
    if os.environ.get('TEST_FAIL_PROFILE_COPY') and paths[0].name == 'storage.filesystem.toml':
        paths[-1].write_bytes(b'partial = [')
        sys.exit(74)
    if os.environ.get('TEST_PROFILE_RACE') and paths[0].name == 'storage.filesystem.toml':
        selected = paths[-1].parent / 'storage.toml'
        selected.write_bytes(b'operator profile appeared concurrently'); selected.chmod(0o644)
    paths[-1].parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(paths[0], paths[-1]); paths[-1].chmod(mode)
PY
}
render_api_configuration
'''
        if mock_activation:
            doubles += 'deploy() { check_private_media_storage; event activate; }\n'
        return subprocess.run(["bash", "-c", self.library + doubles + body],
                              env={**self.environment, **environment}, text=True,
                              capture_output=True, timeout=30)

    def promote(self, **environment):
        return self.run_host('promote_artifact "$TEST_REVISION" "$TEST_TREE" "sha256:$(printf \'%064d\' 0)"\n',
                             **environment)

    def test_first_promotion_prepares_from_incoming_bundle_before_real_install(self):
        self.assertFalse(self.media.exists())
        result = self.promote()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.profile.read_bytes(), self.default.read_bytes())
        self.assertEqual(self.media.stat().st_mode & 0o777, 0o700)
        self.assertEqual((self.root / "runtime/REVISION").read_text(), REVISION + "\n")
        self.assertEqual((self.root / "runtime.previous/REVISION").read_text(), "current\n")
        events = (self.root / "events").read_text().splitlines()
        self.assertEqual(events[-1], "activate")
        self.assertNotIn("forbidden-bootstrap", events)
        self.assertLess(next(i for i, e in enumerate(events) if 'storage.filesystem.toml' in e),
                        next(i for i, e in enumerate(events) if '/quadlet/' in e))

    def test_browser_validation_has_no_media_precondition(self):
        result = self.run_host("validate_runtime_browser_configuration\n")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(self.parent.exists())

    def test_invalid_existing_media_refuses_before_runtime_rotation_or_install(self):
        self.parent.mkdir(mode=0o755)
        self.media.mkdir(mode=0o755)
        sentinel = self.media / "immutable"
        sentinel.write_bytes(b"admitted bytes")
        result = self.promote()
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertIn("private media root must be owned", result.stderr)
        self.assertFalse(self.profile.exists())
        self.assertEqual(self.media.stat().st_mode & 0o777, 0o755)
        self.assertEqual(sentinel.read_bytes(), b"admitted bytes")
        self.assertEqual((self.root / "runtime/REVISION").read_text(), "current\n")
        self.assertEqual((self.root / "runtime.previous/REVISION").read_text(), "recovery\n")
        self.assertEqual((self.root / "sbin/cantos-vps").read_text(), "installed helper\n")
        self.assertEqual((self.root / "quadlet/cantos-api-studio.container").read_text(), "legacy unit\n")

    def prepare(self):
        return self.run_host('ensure_private_media_storage "$TEST_TREE/infra/vps/config/storage.filesystem.toml"\n')

    def assert_runtime_retained(self):
        self.assertEqual((self.root / "runtime/REVISION").read_text(), "current\n")
        self.assertEqual((self.root / "runtime.previous/REVISION").read_text(), "recovery\n")
        self.assertEqual((self.root / "sbin/cantos-vps").read_text(), "installed helper\n")
        self.assertEqual((self.root / "quadlet/cantos-api-studio.container").read_text(), "legacy unit\n")

    def test_existing_filesystem_and_operator_s3_profile_survive_install_and_replay(self):
        self.assertEqual(self.prepare().returncode, 0)
        sentinel = self.media / "immutable"
        sentinel.write_bytes(b"admitted bytes")
        longvan = (REPOSITORY / 'services/cantos-api/config/storage.longvan.example.toml').read_text()
        longvan += ('\n[storage.targets.legacy-filesystem]\ndriver = "filesystem"\n'
                    f'private_root = "{self.media}"\n')
        for profile in (self.default.read_text(), longvan):
            with self.subTest(profile=profile):
                self.profile.write_text(profile)
                self.profile.chmod(0o644)
                identity = self.profile.stat().st_ino
                for _ in range(2):
                    result = self.run_host('install_bundle "$TEST_TREE"\n')
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(self.profile.read_text(), profile)
                    self.assertEqual(self.profile.stat().st_ino, identity)
                    self.assertEqual(sentinel.read_bytes(), b"admitted bytes")

    def test_existing_mode_owner_symlink_and_malformed_profile_refuse_without_repairs(self):
        self.assertEqual(self.prepare().returncode, 0)
        original = self.profile.read_bytes()
        for path in (self.parent, self.media, self.root / 'config', self.profile):
            with self.subTest(path=path):
                mode = path.stat().st_mode & 0o777
                path.chmod(0o777)
                result = self.run_host('install_bundle "$TEST_TREE"\n')
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertEqual(path.stat().st_mode & 0o777, 0o777)
                self.assert_runtime_retained()
                path.chmod(mode)
                result = self.promote(TEST_WRONG_OWNER=str(path))
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assert_runtime_retained()
                saved = path.with_name(path.name + '.retained')
                path.rename(saved)
                path.symlink_to(saved)
                result = self.run_host('install_bundle "$TEST_TREE"\n')
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertTrue(path.is_symlink())
                self.assert_runtime_retained()
                path.unlink()
                saved.rename(path)
        for content in (b'broken = [', original.replace(b'legacy-filesystem"', b'missing"', 1),
                        original.replace(str(self.media).encode(), b'/different/root')):
            with self.subTest(content=content):
                self.profile.write_bytes(content)
                result = self.promote()
                self.assertEqual(result.returncode, 65, result.stderr)
                self.assertIn('invalid storage topology', result.stderr)
                self.assertEqual(self.profile.read_bytes(), content)
                self.assert_runtime_retained()

    def test_missing_candidate_and_copy_failure_leave_runtime_recoverable(self):
        contents = self.default.read_bytes()
        self.default.unlink()
        result = self.run_host('install_bundle "$TEST_TREE"\n')
        self.assertEqual(result.returncode, 66, result.stderr)
        self.assertFalse(self.parent.exists())
        self.assertFalse(self.profile.exists())
        self.assert_runtime_retained()
        self.default.write_bytes(contents)
        result = self.promote(TEST_FAIL_PROFILE_COPY='1')
        self.assertEqual(result.returncode, 74, result.stderr)
        self.assertFalse(self.profile.exists())
        self.assertEqual(list(self.profile.parent.glob('storage.toml.pending.*')), [])
        self.assert_runtime_retained()
        # A leftover from a killed copy is inert; retry publishes a different complete file.
        pending = self.profile.with_name('storage.toml.pending.interrupted')
        pending.write_bytes(b'partial = [')
        result = self.promote()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.profile.read_bytes(), contents)
        self.assertEqual(pending.read_bytes(), b'partial = [')

    def test_existing_profile_with_missing_media_root_is_drift_not_a_new_install(self):
        self.assertEqual(self.prepare().returncode, 0)
        self.media.rmdir()
        result = self.promote()
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertFalse(self.media.exists())
        self.assert_runtime_retained()

    def test_concurrent_profile_is_not_overwritten_by_preparation(self):
        result = self.promote(TEST_PROFILE_RACE='1')
        self.assertNotEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.profile.read_bytes(), b'operator profile appeared concurrently')
        self.assertEqual(list(self.profile.parent.glob('storage.toml.pending.*')), [])
        self.assert_runtime_retained()

    def test_browser_reload_checks_storage_only_when_its_restarted_unit_uses_it(self):
        self.profile.write_bytes(b'broken = [')
        self.profile.chmod(0o644)
        body = '''
unit_active() { return 0; }
podman() { event candidate-caddy-check; }
reload_browser "$TEST_TREE"
'''
        result = self.run_host(body)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('systemctl restart cantos-api-studio.service', (self.root / 'events').read_text())
        (self.root / 'events').unlink()
        (self.root / 'quadlet/cantos-api-studio.container').write_text(
            'Environment=CANTOS_STORAGE_CONFIG=/etc/cantos-storage.toml\n')
        result = self.run_host(body)
        self.assertEqual(result.returncode, 65, result.stderr)
        self.assertFalse((self.root / 'events').exists())

    def test_tracked_container_composition_keeps_private_storage_studio_only(self):
        studio = (REPOSITORY / 'infra/vps/quadlet/cantos-api-studio.container').read_text().splitlines()
        theatre = (REPOSITORY / 'infra/vps/quadlet/cantos-api-theatre.container').read_text()
        for line in ('ReadOnly=true', 'Environment=CANTOS_STORAGE_CONFIG=/etc/cantos-storage.toml',
                     'Volume=/var/lib/cantos/private:/var/lib/cantos/private:rw',
                     'Volume=/root/cantos-services/config/storage.toml:/etc/cantos-storage.toml:ro'):
            self.assertIn(line, studio)
        self.assertNotIn('CANTOS_STORAGE_CONFIG', theatre)
        self.assertNotIn('/var/lib/cantos/private', theatre)
        self.assertIn('USER 10001:10001', (REPOSITORY / 'infra/vps/image/Containerfile').read_text().splitlines())

    def test_runtime_guards_apply_only_to_installed_storage_capability(self):
        self.profile.write_bytes(b'broken = [')
        self.profile.chmod(0o644)
        (self.root / 'state/previous').write_text(REVISION + '\n')
        (self.root / 'state/current').write_text('c' * 40 + '\n')
        for command in ('deploy "$TEST_REVISION"', 'rollback'):
            body = 'check_revision() { event reached-image-check; return 91; }\n' + command + '\n'
            result = self.run_host(body, mock_activation=False)
            self.assertEqual(result.returncode, 91, result.stderr)
            unit = self.root / 'quadlet/cantos-api-studio.container'
            unit.write_text('Environment=CANTOS_STORAGE_CONFIG=/etc/cantos-storage.toml\n')
            result = self.run_host(body, mock_activation=False)
            self.assertEqual(result.returncode, 65, result.stderr)
            # Browser-only validation remains independent even with the storage unit installed.
            result = self.run_host('validate_runtime_browser_configuration\n')
            self.assertEqual(result.returncode, 0, result.stderr)
            unit.write_text('legacy unit\n')


if __name__ == "__main__":
    unittest.main()
