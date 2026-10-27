"""Build a source-pinned static Linux xtask outside the deployment host.

Workspace discovery embeds its compile-time source directory. Preserve that existing contract:
build at a deterministic source-addressed path and require the operator's exact checkout there.
No relocatable-root override, compiler downgrade, containerized host execution or VPS install.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import tomllib

import release_artifact as contract
import release_oci as oci
import release_oci_build as build

PREFIX = Path('/opt/cantos-native-executor')
TARGET = 'x86_64-unknown-linux-musl'


def identity(root, compiler, *, system=platform):
    contract.require(system.system() == 'Linux' and system.machine() == 'x86_64', 'native Linux amd64 executor builder required')
    sha = oci.git(root, 'rev-parse', 'HEAD').decode().strip()
    contract.sha(sha)
    contract.require(not oci.git(root, 'status', '--porcelain', '--untracked-files=normal').strip(), 'clean executor source required')
    pin = tomllib.loads((root/'rust-toolchain.toml').read_text())['toolchain']['channel']
    contract.require(compiler.startswith('rustc '+pin+' '), 'exact pinned executor compiler required')
    return sha, pin, PREFIX/sha


def static_binary(path, program_headers, dynamic):
    data = path.read_bytes()
    contract.require(len(data) >= 64 and data[:6] == b'\x7fELF\x02\x01' and data[18:20] == b'\x3e\x00' and
                     'INTERP' not in program_headers and '(NEEDED)' not in dynamic,
                     'static amd64 ELF without loader or shared dependencies required')
    return {'path': 'xtask', 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}


def produce(root, output):
    compiler = build.sh(root, ['rustc', '--version']).strip()
    sha, pin, checkout = identity(root, compiler)
    contract.require(PREFIX.is_dir() and PREFIX.resolve() == PREFIX and not checkout.exists() and
                     not checkout.is_symlink() and not output.exists(), 'fresh canonical executor build/output paths required')
    # Local objects only; no origin credentials or remote fetch. The isolated build is not the
    # operator's deployment checkout, and the build tree itself is never delivered.
    build.sh(root, ['git', 'clone', '--shared', '--no-checkout', '--', str(root), str(checkout)])
    build.sh(checkout, ['git', 'checkout', '--detach', sha])
    build.sh(checkout, ['cargo', 'build', '--locked', '--release', '--package', 'xtask', '--target', TARGET,
                       '--config', 'target.x86_64-unknown-linux-musl.linker="musl-gcc"'])
    binary = checkout/'target'/TARGET/'release/xtask'
    record = static_binary(binary, build.sh(checkout, ['readelf', '--program-headers', str(binary)]),
                           build.sh(checkout, ['readelf', '--dynamic', str(binary)]))
    # Runtime smoke has no compiler/cargo on PATH. It must reach the actual Python adoption
    # refusal, not just clap help, from a different cwd. No input or production effect exists.
    smoke = checkout/'target/executor-smoke'
    smoke.mkdir()
    (smoke/'python3').symlink_to(shutil.which('python3'))
    observed = subprocess.run([str(binary), 'release', 'native-adopt', '--request', str(smoke/'absent.json'),
                               '--request-digest', '0'*64], cwd=smoke, env={'PATH': str(smoke), 'HOME': str(smoke)},
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
    contract.require(observed.returncode == 1 and b'native adoption refused or interrupted' in observed.stderr,
                     'prebuilt executor must reach Python without Rust or Cargo')
    contract.require(oci.git(checkout, 'rev-parse', 'HEAD').decode().strip() == sha and
                     not oci.git(checkout, 'status', '--porcelain', '--untracked-files=normal').strip(),
                     'executor build source changed')
    output.mkdir(parents=True)
    shutil.copyfile(binary, output/'xtask')
    os.chmod(output/'xtask', 0o755)
    result = {'schema_version': 1, 'kind': 'cantos-native-executor', 'source_sha': sha,
              'platform': 'linux/amd64', 'compiler': compiler, 'toolchain_pin': pin, 'target': TARGET,
              'required_checkout': str(checkout), 'binary': record, 'no_compiler_runtime_smoke': 'PASS',
              'runtime_requirements': ['python3>=3.11', 'git-with-no-lazy-fetch', 'rootful-podman', 'systemd', 'Linux-procfs'],
              'activation': 'NOT_RUN', 'admission': 'NOT_RUN'}
    contract.write_new(output/'executor.json', result)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    try:
        result = produce(Path(__file__).resolve().parents[1], args.output.resolve())
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError):
        parser.exit(1, 'native executor build refused; preserve build evidence; no deployment\n')
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
