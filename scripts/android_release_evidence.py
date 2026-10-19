"""Source-bound release/audit snapshots for theatre-host; never builds or distributes."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import subprocess
import sys
import uuid

MOBILE = Path('apps/cantos-theatre/mobile')
REPORT = MOBILE / 'androidApp/build/reports/release-boundary/release-evidence.json'


def identity(root):
    names = subprocess.check_output(['git', 'ls-files', '-z', '--cached', '--others', '--exclude-standard'], cwd=root).split(b'\0')
    hasher = hashlib.sha256()
    for name in sorted(set(filter(None, names))):
        path = root / name.decode()
        if path.is_symlink():
            raw = str(path.readlink()).encode()
        else:
            raw = path.read_bytes()
        hasher.update(name + b'\0' + hashlib.sha256(raw).digest())
    return {'source_sha': subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=root, text=True).strip(),
            'input_sha256': hasher.hexdigest(),
            'dirty': bool(subprocess.check_output(['git', 'status', '--porcelain', '--untracked-files=normal'], cwd=root))}


def snapshot(root, base, destination, files):
    result = {}
    for label, relative in files.items():
        path = base / relative
        if path.is_symlink() or not path.is_file() or path.stat().st_size == 0:
            raise ValueError(f'missing/nonregular release input: {relative}')
        target = destination / label
        target.parent.mkdir(parents=True, exist_ok=True)
        with target.open('xb') as handle:
            handle.write(path.read_bytes())
        target.chmod(0o400)
        result[label] = {'path': target.relative_to(root).as_posix(), 'bytes': target.stat().st_size,
                         'sha256': hashlib.sha256(target.read_bytes()).hexdigest()}
    return result


def validate_pair(audit, optimized):
    if audit['identity'] != optimized['identity']:
        raise ValueError('source/config inputs changed between audit and optimized release')
    if audit['graph'] != optimized['graph']:
        raise ValueError('audit and optimized release dependency graphs differ')


def validate_inspected(root, files):
    report = (root / files['inspection.txt']['path']).read_text()
    for label, name in [('apk', 'app.apk'), ('aab', 'app.aab')]:
        matches = re.findall(r'^' + label + r' .* sha256=([0-9a-f]{64}) ', report, re.M)
        if matches != [files[name]['sha256']]:
            raise ValueError('artifact changed between inspection and snapshot')
    if 'mapping.txt' in files and ('mapping_sha256=' + files['mapping.txt']['sha256']) not in report:
        raise ValueError('mapping changed between inspection and snapshot')


def execute(root, phase, signed=False):
    report = root / REPORT
    if phase == 'begin':
        run = root / 'artifacts/android-release' / uuid.uuid4().hex
        run.mkdir(parents=True)
        state = {'schema': 1, 'identity': identity(root), 'run': run.relative_to(root).as_posix(),
                 'variant': 'release', 'signed': signed, 'audit_distributable': False}
    else:
        state = json.loads(report.read_text())
        if state['identity'] != identity(root) or state['signed'] != signed:
            raise ValueError('source/config inputs changed during release audit')
        run = root / state['run']
        base = root / MOBILE / 'androidApp' / ('build-release-audit' if phase == 'audit' else 'build')
        apk = 'androidApp-release.apk' if phase != 'audit' and signed else 'androidApp-release-unsigned.apk'
        files = {'app.apk': f'outputs/apk/release/{apk}', 'app.aab': 'outputs/bundle/release/androidApp-release.aab',
                 'inspection.txt': 'reports/release-boundary/artifact-inspection.txt',
                 'dependencies.txt': 'reports/release-boundary/resolved-dependencies.txt',
                 'source-inputs.json': 'reports/release-boundary/source-inputs.json'}
        if phase != 'audit':
            files.update({'mapping.txt': 'outputs/mapping/release/mapping.txt',
                          'pre-shrink-inputs.json': 'reports/release-boundary/pre-shrink-inputs.json'})
        current = {'identity': identity(root), 'graph': (base / files['dependencies.txt']).read_text(),
                   'files': snapshot(root, base, run / phase, files), 'audit': phase == 'audit'}
        validate_inspected(root, current['files'])
        if phase != 'audit':
            validate_pair(state['audit'], current)
            # Audit bytes must still be exactly what the earlier inspector saw.
            for record in state['audit']['files'].values():
                if hashlib.sha256((root / record['path']).read_bytes()).hexdigest() != record['sha256']:
                    raise ValueError('audit snapshot changed')
            current['apk_bytes'] = current['files']['app.apk']['bytes']
            state['apk_size'] = {'metric': 'APK file bytes; same source/features/toolchain, unsigned audit',
                                 'audit': state['audit']['files']['app.apk']['bytes'],
                                 'optimized': current['apk_bytes'],
                                 'estimated_download': 'NOT_RUN', 'installed_footprint': 'NOT_RUN'}
        state[phase] = current
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps(state, indent=2) + '\n')
    # Persist each phase separately so later builds cannot rewrite its original record.
    (root / state['run'] / (phase + '.json')).write_text(json.dumps(state, indent=2) + '\n')
    print(f'PASS Android release evidence {phase}: {report}')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=['begin', 'audit', 'optimized', 'fingerprint'])
    parser.add_argument('--signed', action='store_true')
    args = parser.parse_args()
    if args.phase == 'fingerprint':
        print(identity(Path.cwd())['input_sha256'])
    else:
        execute(Path.cwd(), args.phase, args.signed)


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, KeyError) as error:
        print(f'Android release evidence failed: {error}', file=sys.stderr)
        sys.exit(1)
