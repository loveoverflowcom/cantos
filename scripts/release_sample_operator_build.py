"""Build a source-pinned Linux sample operator artifact; never deploy or open a database.

The binary stays outside the Server image. Its privileged CLI is an operator-only tool; build
success does not constitute sample qualification, admission, rights verification or activation.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

import release_artifact as contract
import release_oci_build as build

PHASE = 'PREFLIGHT'
# The one digest-pinned Rust build stage and the operator binary's size bounds. Qualification
# reads both from here, so the receipt it checks cannot drift from what the build accepted.
BUILDER = re.compile(r'^FROM (docker\.io/library/rust:[^\s]+@sha256:[a-f0-9]{64}) AS build$', re.MULTILINE)
BINARY_BYTES = range(64, 512*1024**2 + 1)


def pinned_builder(containerfile):
    builders = BUILDER.findall(containerfile)
    contract.require(len(builders) == 1, 'exact pinned Rust builder required')
    return builders[0]


def produce(root, output):
    global PHASE
    platform = 'linux/amd64'
    sha = build.preflight(root, platform)
    configuration = build.configuration_identity(root, sha, platform)
    containerfile = build.CONTAINERFILE.format('cantos-server')
    builder = pinned_builder(build.git_bytes(root, sha, containerfile).decode())
    contract.require(output.is_absolute() and not output.exists() and not output.is_symlink(), 'fresh output required')
    with tempfile.TemporaryDirectory(prefix='cantos-sample-operator-build-') as temporary:
        PHASE = 'SOURCE_CONTEXT'
        work = Path(temporary)
        context = work/'context'
        context.mkdir()
        build.sh(root, ['git', 'archive', '--format=tar', '-o', str(work/'source.tar'), sha])
        build.sh(root, ['tar', '-xf', str(work/'source.tar'), '-C', str(context)])
        tag = 'localhost/cantos-sample-operator-build:'+sha
        PHASE = 'BUILD_STAGE'
        build.sh(root, ['podman', 'build', '--format', 'oci', '--platform', platform, '--pull=missing',
                        '--target', 'build', '-f', str(context/containerfile),
                        '--build-arg', 'CANTOS_REVISION='+sha, '--build-arg', 'CANTOS_CONFIGURATION='+configuration,
                        '-t', tag, str(context)])
        image = json.loads(build.sh(root, ['podman', 'image', 'inspect', tag]))[0]
        image_id = 'sha256:'+contract.digest(image['Id'].removeprefix('sha256:'))
        created = build.sh(root, ['podman', 'create', '--entrypoint', '/bin/true', image_id]).strip()
        contract.require(re.fullmatch(r'[a-f0-9]{64}', created) is not None, 'exact temporary container required')
        binary = work/'cantos-operator'
        PHASE = 'EXTRACT_BINARY'
        try:
            build.sh(root, ['podman', 'cp', created+':/out/cantos-operator', str(binary)])
        finally:
            build.sh(root, ['podman', 'rm', created])
        contract.require(binary.is_file() and not binary.is_symlink() and binary.stat().st_nlink == 1 and
                         binary.stat().st_size in BINARY_BYTES, 'bounded regular operator binary required')
        data = binary.read_bytes()
        PHASE = 'VERIFY_BINARY'
        contract.require(len(data) >= 64 and data[:6] == b'\x7fELF\x02\x01' and data[18:20] == b'\x3e\x00',
                         'native amd64 ELF required')
        contract.require(build.preflight(root, platform) == sha,
                         'source changed during operator build')
        output.mkdir(mode=0o700, parents=True, exist_ok=False)
        PHASE = 'PERSIST_ARTIFACT'
        shutil.copyfile(binary, output/'cantos-operator')
        os.chmod(output/'cantos-operator', 0o755)
        record = {'schema_version': 1, 'operation': 'BUILD_SAMPLE_OPERATOR', 'status': 'BUILT_NOT_QUALIFIED',
                  'source_sha': sha, 'platform': platform, 'configuration_sha256': configuration,
                  'builder_base': builder, 'builder_image_id': image_id,
                  'binary_sha256': hashlib.sha256(data).hexdigest(), 'binary_bytes': len(data),
                  'admission': 'NOT_RUN', 'activation': 'NOT_RUN'}
        contract.write_new(output/'operator-build.json', record)
        return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = produce(Path(__file__).resolve().parents[1], args.output.resolve())
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        parser.exit(1, 'sample operator build refused at '+PHASE+': '+type(error).__name__+
                    '; no deployment or database operation\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
