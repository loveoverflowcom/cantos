"""Pre-optimization production boundary; primitive of the Android Gradle/xtask lane."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import sys
import zipfile

from inspect_android_release import class_findings, resource_hashes


def source_inventory(root):
    result = set()
    for path in Path(root).rglob('*.kt'):
        text = path.read_text()
        package = re.search(r'^package ([\w.]+)', text, re.M)
        if package:
            prefix = 'L' + package[1].replace('.', '/') + '/'
            result.add(prefix + path.stem + 'Kt;')
            result.update(prefix + name + ';' for name in re.findall(
                r'\b(?:class|interface|object)\s+(\w+)', text))
    if not result:
        raise ValueError('empty sandbox source inventory')
    return result


def source_findings(mobile):
    sandbox = mobile / 'preview-sandbox/src/commonMain'
    forbidden = source_inventory(sandbox)
    hashes = resource_hashes(sandbox / 'composeResources')
    findings = []
    roots = [mobile / module / 'src' / source for module in ('androidApp', 'shared')
             for source in ('main', 'debug', 'release', 'commonMain', 'androidMain', 'jvmHttpMain')]
    fixture_sources = {p.read_bytes() for p in sandbox.rglob('*.kt')}
    for root in roots:
        if not root.exists():
            continue
        for path in root.rglob('*'):
            if not path.is_file():
                continue
            raw = path.read_bytes()
            label = path.relative_to(mobile).as_posix()
            if hashlib.sha256(raw).hexdigest() in hashes:
                findings.append(f'{label}: copied sandbox resource bytes')
            if path.suffix == '.kt':
                package = re.search(r'^package ([\w.]+)', raw.decode(), re.M)
                if package:
                    prefix = 'L' + package[1].replace('.', '/') + '/'
                    names = {prefix + path.stem + 'Kt;'} | {prefix + n + ';' for n in re.findall(
                        r'\b(?:class|interface|object)\s+(\w+)', raw.decode())}
                    findings.extend(f'{label}: {f}' for f in class_findings(names, forbidden))
                if raw in fixture_sources:
                    findings.append(f'{label}: copied sandbox source bytes')
    return findings


def input_findings(paths, forbidden=None, forbidden_hashes=None):
    """Inspect actual R8 input archives/directories before shrinking can hide their names."""
    findings, records, count = [], [], 0
    for root in sorted(set(map(Path, paths))):
        files = sorted(root.rglob('*')) if root.is_dir() else [root]
        for path in files:
            if not path.is_file():
                continue
            if path.suffix == '.class':
                # Class directories are also checked by source and the unshrunk DEX audit.
                name = path.as_posix()
                start = next((name.find('/' + p) for p in ('com/cantos/', 'kotlin/test/', 'org/junit/')
                              if '/' + p in name), -1)
                classes = {'L' + name[start + 1:-6] + ';'} if start >= 0 else set()
            elif path.suffix in ('.jar', '.zip'):
                with zipfile.ZipFile(path) as archive:
                    classes = {'L' + n[:-6] + ';' for n in archive.namelist() if n.endswith('.class')}
            else:
                continue
            count += len(classes)
            findings.extend(class_findings(classes, forbidden or set()))
            if path.suffix in ('.jar', '.zip') and forbidden_hashes:
                with zipfile.ZipFile(path) as archive:
                    for name in archive.namelist():
                        if not name.endswith('/') and hashlib.sha256(archive.read(name)).hexdigest() in forbidden_hashes:
                            findings.append(f'{path.name}: copied sandbox resource bytes at {name}')
            records.append({'name': path.name, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
    if not records or not count:
        raise ValueError('no compiled pre-shrink class inputs selected')
    return sorted(set(findings)), records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mobile', type=Path)
    parser.add_argument('--input', action='append', default=[])
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    findings, records = source_findings(args.mobile) if args.mobile else [], []
    if args.input:
        sandbox = args.mobile / 'preview-sandbox/src/commonMain' if args.mobile else None
        more, records = input_findings(args.input,
            source_inventory(sandbox) if sandbox else set(),
            resource_hashes(sandbox / 'composeResources') if sandbox else {})
        findings += more
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps({'findings': findings, 'inputs': records}, indent=2) + '\n')
    if findings:
        raise ValueError('\n'.join(findings))
    print(f'PASS Android source/pre-shrink boundary: {args.report}')


if __name__ == '__main__':
    try:
        main()
    except (ValueError, OSError, zipfile.BadZipFile) as error:
        print(f'Android boundary failed: {error}', file=sys.stderr)
        sys.exit(1)
