"""Read-only prerequisites for a future disposable native executor rehearsal.

This probe does not create fixtures or produce execution evidence. No pass here authorizes
host mutation. The full test harness must independently own every path it creates and stop
on any unexpected object; these checks do not replace isolated-runner review.
"""

import json
import os
from pathlib import Path
import platform
import shutil
import subprocess


PROTECTED_PATHS = (
    '/root/cantos-native',
    '/var/lib/cantos-release',
    '/etc/containers/systemd/cantos-server.container',
    '/etc/containers/systemd/cantos-identity.container',
)
SERVICES = ('cantos-server', 'cantos-identity')


def command(*argv):
    result = subprocess.run(argv, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                            stderr=subprocess.DEVNULL, timeout=20, check=False)
    if result.returncode or len(result.stdout) > 4 * 1024 * 1024:
        raise ValueError('prerequisite command failed')
    return result.stdout


def preflight():
    if (platform.system() != 'Linux' or platform.machine() != 'x86_64' or os.geteuid() != 0 or
            os.environ.get('GITHUB_ACTIONS') != 'true' or
            os.environ.get('RUNNER_ENVIRONMENT') != 'github-hosted'):
        raise ValueError('rootful disposable GitHub Linux runner required')
    if not Path('/run/systemd/system').is_dir() or not Path('/sys/fs/cgroup/cgroup.controllers').is_file():
        raise ValueError('systemd and cgroup v2 required')
    for name in ('systemctl', 'podman'):
        if shutil.which(name) is None:
            raise ValueError('required native runtime missing')
    if any(Path(name).exists() or Path(name).is_symlink() for name in PROTECTED_PATHS):
        raise ValueError('fixture path already exists')
    if command('systemctl', 'show', '--property=SystemState', '--value').strip() not in (b'running', b'degraded'):
        raise ValueError('systemd is not operational')
    generators = ('/usr/lib/systemd/system-generators/podman-system-generator',
                  '/lib/systemd/system-generators/podman-system-generator',
                  '/usr/lib/systemd/system-generators/quadlet', '/usr/libexec/podman/quadlet')
    if not any(Path(name).is_file() and os.access(name, os.X_OK) for name in generators):
        raise ValueError('Podman Quadlet generator required')
    info = json.loads(command('podman', '--remote=false', 'info', '--format', 'json'))
    if info['host']['cgroupVersion'] != 'v2' or info['host']['security']['rootless']:
        raise ValueError('rootful cgroup v2 Podman required')
    rows = json.loads(command('podman', '--remote=false', 'ps', '-a', '--format', 'json'))
    for row in rows:
        names = row.get('Names', [])
        if isinstance(names, str):
            names = [names]
        if any(name in SERVICES for name in names):
            raise ValueError('fixture container name already exists')
    for service in SERVICES:
        state = command('systemctl', 'show', '--property=LoadState', '--value', service+'.service').strip()
        if state != b'not-found':
            raise ValueError('fixture service name already exists')
    return {'status': 'PREREQUISITES_ONLY', 'executor_transition': 'NOT_RUN', 'activation': 'NOT_RUN'}


if __name__ == '__main__':
    try:
        print(json.dumps(preflight(), sort_keys=True))
    except ValueError as error:
        raise SystemExit('disposable rehearsal prerequisites not verified: '+str(error))
    except (OSError, KeyError, TypeError, subprocess.SubprocessError):
        raise SystemExit('disposable rehearsal prerequisites not verified: runtime probe failed')
