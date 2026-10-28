"""Qualify a separate sample operator binary against exact images and synthetic data only.

The fixture does not assert production rights, create an admission or touch a real database.
It requires a clean exact-source checkout, a source-bound build receipt, and a disposable
rootful Linux Podman environment. The generated report remains separate from image gates.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile

import release_artifact as contract
import release_oci as oci
import release_oci_build as image_build
import release_oci_qualify as qualify

SOURCE = 'services/cantos-server/tests/fixtures/fictional-sample-metadata.json'
SOURCE_SHA = '0ad007cb18d46b58df25dbc2e8ea98bf58bce752c9e6d640fc4a78a90ef779d6'
CHECKS = ['missing-rights-refused', 'operator-publish-and-replay', 'v2-empty-v3-six-works-twelve-units',
          'no-cover-or-playback', 'non-login-owner-token-refused', 'withdraw-and-replay', 'withdrawal-hidden']


def digest(path):
    before = path.stat()
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for part in iter(lambda: stream.read(1048576), b''):
            h.update(part)
    after = path.stat()
    contract.require((before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) ==
                     (after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns), 'binary changed')
    return h.hexdigest()


def request(action, operation_id, confirmed=True):
    return {'operation_id': operation_id, 'action': action, 'source_sha256': SOURCE_SHA,
            'operator_reference': 'synthetic isolated fixture', 'evidence_reference': 'synthetic fixture only',
            'rights_statement': 'synthetic fixture statement; not production rights',
            'confirm_source': confirmed, 'confirm_metadata_rights': confirmed}


def pull_failure_class(stderr):
    # Podman errors can contain registry credentials or URLs. Report only a fixed class.
    message = stderr.lower()
    if b'no space left on device' in message or b'disk quota exceeded' in message:
        return 'STORAGE_EXHAUSTED'
    if b'too many requests' in message or b'toomanyrequests' in message:
        return 'REGISTRY_RATE_LIMIT'
    if b'unauthorized' in message or b'authentication required' in message:
        return 'REGISTRY_AUTH'
    if b'permission denied' in message:
        return 'PERMISSION'
    if b'manifest unknown' in message or b'manifest not found' in message:
        return 'IMAGE_NOT_FOUND'
    if b'x509:' in message or b'certificate verify failed' in message:
        return 'TLS_CERTIFICATE'
    if any(s in message for s in (b'connection refused', b'connection reset', b'connection timed out',
                                   b'network is unreachable', b'no such host', b'tls handshake timeout')):
        return 'NETWORK'
    return 'UNCLASSIFIED'


def pull_exact(fixture, service):
    # Use the same isolated store and authfile as Fixture.pod, with strictly redacted
    # diagnostic categories on failure; a failed pull still refuses qualification.
    result = subprocess.run([*fixture.podman, 'pull', '--authfile', str(fixture.authfile), fixture.image(service)],
                            cwd=fixture.root, env=fixture.env, stdin=subprocess.DEVNULL,
                            stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, timeout=600, check=False)
    contract.require(result.returncode == 0, 'PODMAN_PULL_'+pull_failure_class(result.stderr))


def qualify_operator(root, args):
    manifest, _ = qualify.validate_input(root, args)
    contract.require(qualify.native_host() == manifest['platform'], 'native qualification platform mismatch')
    build = contract.read_json(args.build_record, args.build_record_digest)
    oci.fields(build, 'schema_version operation status source_sha platform configuration_sha256 builder_base builder_image_id binary_sha256 binary_bytes '
               'admission activation', 'exact operator build receipt required')
    containerfile = image_build.git_bytes(root, manifest['source_sha'], image_build.CONTAINERFILE.format('cantos-server')).decode()
    builders = re.findall(r'^FROM (docker\.io/library/rust:[^\s]+@sha256:[a-f0-9]{64}) AS build$', containerfile, re.MULTILINE)
    contract.require(len(builders) == 1 and build['builder_base'] == builders[0] and
                     re.fullmatch(r'sha256:[a-f0-9]{64}', build['builder_image_id']) is not None,
                     'operator builder identity mismatch')
    contract.require(type(build['schema_version']) is int and build['schema_version'] == 1 and
                     type(build['binary_bytes']) is int and 64 <= build['binary_bytes'] <= 512*1024**2 and
                     build['operation'] == 'BUILD_SAMPLE_OPERATOR' and
                     build['status'] == 'BUILT_NOT_QUALIFIED' and build['source_sha'] == manifest['source_sha'] and
                     build['platform'] == manifest['platform'] and build['configuration_sha256'] == manifest['configuration_sha256'] and
                     build['admission'] == 'NOT_RUN' and
                     build['activation'] == 'NOT_RUN' and args.binary.is_file() and not args.binary.is_symlink() and
                     args.binary.stat().st_nlink == 1 and
                     args.binary.stat().st_size == build['binary_bytes'] and digest(args.binary) == build['binary_sha256'],
                     'operator build identity mismatch')
    source = (root/SOURCE).read_bytes()
    contract.require(hashlib.sha256(source).hexdigest() == SOURCE_SHA, 'fictional source bytes changed')
    contract.require(not args.output.exists() and not args.output.is_symlink(), 'fresh operator report required')
    with tempfile.TemporaryDirectory(prefix='cantos-sample-operator-qualification-') as temporary:
        work = Path(temporary).resolve()
        f = qualify.Fixture(root, work, manifest, args.authfile)
        try:
            for service in oci.SERVICES:
                pull_exact(f, service)
                qualify.inspect_identity(service, manifest['images'][service], manifest,
                                         json.loads(f.pod('image', 'inspect', f.image(service)))[0])
            f.migrate()
            inputs = work/'inputs'
            inputs.mkdir(mode=0o755)
            (inputs/'source.json').write_bytes(source)
            requests = {'deny.json': request('publish', '00000000-0000-4000-8000-000000000001', False),
                        'publish.json': request('publish', '00000000-0000-4000-8000-000000000002'),
                        'withdraw.json': request('withdraw', '00000000-0000-4000-8000-000000000003', False)}
            for name, value in requests.items():
                (inputs/name).write_bytes(contract.canonical(value))
            for path in inputs.iterdir():
                path.chmod(0o444)
            server = 'cantos-server'
            common = ['run', '--rm', '--network=none', '--pull=never', '--read-only', '--cap-drop=ALL',
                      '--security-opt=no-new-privileges', '--user', '10001:10001',
                      '--mount', f'type=bind,src={f.databases[server]},dst=/database',
                      '--mount', f'type=bind,src={args.binary},dst=/cantos-operator,ro=true',
                      '--mount', f'type=bind,src={inputs},dst=/inputs,ro=true',
                      '--env', 'CANTOS_SERVER_DATABASE=/database/server.db',
                      '--entrypoint', '/cantos-operator', f.image(server)]
            f.pod(*common, 'sample-metadata', '/inputs/deny.json', '/inputs/source.json', expected=1)
            for _ in range(2):
                f.pod(*common, 'sample-metadata', '/inputs/publish.json', '/inputs/source.json')
            f.pod(*common, 'issue-token', 'cantos_fictional_seed_v2', '1', expected=1)
            f.start()
            f.call('sample_present')
            f.stop()
            for _ in range(2):
                f.pod(*common, 'withdraw-sample-metadata', '/inputs/withdraw.json')
            f.restart()
            f.call('sample_withdrawn')
            f.stop()
            contract.require(digest(args.binary) == build['binary_sha256'], 'operator binary drift')
        finally:
            f.close()
    report = {'schema_version': 1, 'operation': 'SAMPLE_OPERATOR_QUALIFICATION', 'status': 'PASS',
              'source_sha': manifest['source_sha'], 'target_manifest_sha256': args.manifest_digest,
              'images': manifest['images'], 'binary_sha256': build['binary_sha256'],
              'build_record_sha256': args.build_record_digest, 'sample_source_sha256': SOURCE_SHA,
              'checks': CHECKS, 'evidence_level': 'integration-tested',
              'execution_environment': 'linux-native-exact-images', 'production_rights': 'NOT_ASSERTED',
              'admission': 'NOT_RUN', 'activation': 'NOT_RUN'}
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    contract.write_new(args.output/'qualification.json', report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('manifest', 'binary', 'build-record', 'authfile', 'output'):
        parser.add_argument('--'+name, type=Path, required=True)
    for name in ('manifest-digest', 'build-record-digest', 'source-sha', 'configuration-digest', 'platform'):
        parser.add_argument('--'+name, required=True)
    args = parser.parse_args()
    try:
        result = qualify_operator(Path(__file__).resolve().parents[1], args)
    except (ValueError, OSError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError) as error:
        reason = str(error) if isinstance(error, ValueError) and re.fullmatch(r'[A-Za-z0-9 _:/.-]{1,160}', str(error)) else type(error).__name__
        parser.exit(1, 'sample operator qualification failed: '+reason+'; no production acceptance\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
