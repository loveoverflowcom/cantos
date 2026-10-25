"""Existing native-host upgrades only. Invoked through xtask; never SSH/bootstrap/cutover.

The pure request decision precedes a rootful Linux adapter. Same schema and runtime
configuration are mandatory. All qualification/admission records remain independent
operator-reviewed inputs; this executor cannot manufacture them.
"""
import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import stat
import subprocess
import time
from types import SimpleNamespace
import urllib.request
import uuid

import release_artifact as contract
import release_oci as oci
import release_oci_build as build

SERVICES = oci.SERVICES
START = tuple(reversed(SERVICES))
STATE = Path('/var/lib/cantos-release')
UNITS = Path('/etc/containers/systemd')


def digest_bytes(data):
    return hashlib.sha256(data).hexdigest()


def digest_file(path):
    before=path.stat()
    hasher=hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda:stream.read(1024*1024), b''):
            hasher.update(chunk)
    after=path.stat()
    contract.require((before.st_ino,before.st_size,before.st_mtime_ns,before.st_ctime_ns) ==
                     (after.st_ino,after.st_size,after.st_mtime_ns,after.st_ctime_ns), 'file changed during backup hashing')
    return hasher.hexdigest()


def private_path(path, directory=False):
    path = Path(path)
    contract.require(path.is_absolute() and path == path.resolve(), 'canonical non-symlink path required')
    meta = path.lstat()
    contract.require((stat.S_ISDIR(meta.st_mode) if directory else stat.S_ISREG(meta.st_mode)) and
                     meta.st_uid == os.geteuid() and not meta.st_mode & 0o022 and
                     (directory or (meta.st_nlink == 1 and meta.st_size <= contract.MAX_JSON)), 'private owned bounded regular input required')
    return path


def pinned(path, digest):
    private_path(path)
    return contract.read_json(Path(path), digest)


def release(root, pins):
    oci.fields(pins, 'manifest manifest_digest source_sha configuration_digest platform admission admission_digest',
               'complete immutable release pins required')
    args = SimpleNamespace(**pins, dry_run=True)
    args.manifest, args.admission = Path(args.manifest), Path(args.admission)
    oci.verify(root, args)
    manifest = contract.read_json(args.manifest, args.manifest_digest)
    evidence = contract.read_json(args.manifest.parent/'evidence.json', manifest['evidence']['sha256'])
    for gate in oci.GATES:
        record = contract.read_json(args.manifest.parent/'gate-records'/(gate+'.json'),
                                    evidence['gates'][gate]['record_sha256'])
        contract.require(record.get('evidence_level') == 'integration-tested' and
                         record.get('execution_environment') == 'linux-native-exact-images',
                         'native activation requires reviewed exact-image execution evidence, not model/mock assertions')
    contract.require(manifest['compatibility'] == build.compatibility(root, args.source_sha),
                     'admitted compatibility differs from actual source registry')
    return manifest


def decide(request, old, new, now, recovery=False):
    """No effects: accept only a fresh, exact existing native-host upgrade."""
    oci.fields(request, 'schema_version host_machine_sha256 observed_at host_state generation current_manifest_sha256 '
               'previous next databases socket_directory units configuration_files mounts health_url', 'invalid native request fields')
    contract.require(type(request['schema_version']) is int and request['schema_version'] == 1,
                     'native request version')
    contract.require(request['host_state'] == 'NATIVE_READY', 'native upgrade only; no bootstrap or legacy cutover')
    contract.digest(request['host_machine_sha256'])
    contract.require(type(request['observed_at']) is int and (recovery or 0 <= now-request['observed_at'] <= 3600),
                     'fresh request within one hour required')
    contract.require(type(request['generation']) is int and request['generation'] >= 0, 'invalid generation')
    contract.require(request['current_manifest_sha256'] == request['previous']['manifest_digest'], 'current release mismatch')
    contract.require(request['previous']['manifest_digest'] != request['next']['manifest_digest'], 'release must change')
    contract.require(old['platform'] == new['platform'], 'platform must remain unchanged')
    contract.require(old['compatibility'] == new['compatibility'], 'schema/RPC/security change requires separate migration recovery')
    oci.fields(request['databases'], ' '.join(SERVICES), 'exact database owners required')
    oci.fields(request['units'], ' '.join(SERVICES), 'exact native units required')
    paths = []
    for service in SERVICES:
        p = Path(request['databases'][service])
        contract.require(re.fullmatch(r'/var/lib/cantos/[a-z0-9/-]+', str(p)) is not None and
                         '..' not in p.parts and '//' not in str(p), 'canonical Cantos database directory required')
        paths.append(p)
        contract.digest(request['units'][service])
    socket = Path(request['socket_directory'])
    contract.require(re.fullmatch(r'/run/cantos/[a-z0-9/-]+', str(socket)) is not None and
                     '..' not in socket.parts and '//' not in str(socket), 'canonical private socket directory required')
    for p in paths:
        contract.require(all(p != other and p not in other.parents and other not in p.parents
                             for other in paths if other != p), 'database roots overlap')
    contract.require(paths[0] != paths[1], 'database roots must differ')
    oci.fields(request['mounts'], ' '.join(SERVICES), 'exact mount inventory required')
    for service in SERVICES:
        validate_mount_plan(request, service)
    configs = request['configuration_files']
    contract.require(isinstance(configs, dict) and 1 <= len(configs) <= 32, 'bounded exact runtime configuration files required')
    for name, digest in configs.items():
        contract.require(re.fullmatch(r'/etc/cantos/[a-zA-Z0-9_./-]+', name) is not None and
                         '..' not in PurePosixPath(name).parts and '//' not in name, 'canonical Cantos configuration path required')
        contract.digest(digest)
    contract.require(re.fullmatch(r'http://127\.0\.0\.1:[1-9][0-9]{0,4}/[a-zA-Z0-9_/-]+', request['health_url']) is not None,
                     'explicit loopback health URL required')
    return {'operation': 'NATIVE_SAME_SCHEMA_UPGRADE', 'source_sha': new['source_sha'],
            'images': new['images'], 'activation': 'NOT_RUN', 'qualification': 'OPERATOR_ADMITTED'}


def unit_image(data, old_ref, new_ref):
    """Preserve every byte except the single exact existing Container Image directive."""
    text = data.decode('utf-8')
    sections = []
    count = 0
    for line in text.splitlines():
        if line.startswith('['):
            sections.append(line)
        if '=' in line and line.split('=',1)[0].strip() == 'Image':
            contract.require(sections and sections[-1] == '[Container]' and line == 'Image='+old_ref,
                             'unit image differs from current admitted pair')
            count += 1
        contract.require(not line.endswith('\\'), 'continued Quadlet lines unsupported')
    contract.require(count == 1, 'exactly one Image directive required')
    return ''.join(('Image='+new_ref+line[len('Image='+old_ref):]) if line.rstrip('\r\n') == 'Image='+old_ref else line
                   for line in text.splitlines(keepends=True)).encode()


def environment_files(data):
    """Parse assignment whitespace; refuse optional/continued/specifier/multiple-file forms."""
    result = []
    for line in data.decode().splitlines():
        stripped=line.strip()
        if not stripped or stripped.startswith(('#',';','[')):
            continue
        contract.require(not line.endswith('\\'), 'continued Quadlet lines unsupported')
        if '=' not in line:
            continue
        key,value=line.split('=',1)
        if key.strip() == 'EnvironmentFile':
            value=value.strip()
            contract.require(re.fullmatch(r'/etc/cantos/[A-Za-z0-9_./-]+',value) and
                             '..' not in PurePosixPath(value).parts and str(PurePosixPath(value)) == value,
                             'unsupported environment file directive')
            result.append(value)
    return result


def effective_environment_files(value, bound):
    remaining=value.strip()
    result=[]
    pattern=re.compile(r'(/[A-Za-z0-9_./-]+) \(ignore_errors=(?:yes|no)\)')
    while remaining:
        match=pattern.match(remaining)
        contract.require(match is not None, 'unparsed systemd environment file output')
        result.append(match[1])
        remaining=remaining[match.end():]
        contract.require(not remaining or remaining[0].isspace(), 'unparsed systemd environment file suffix')
        remaining=remaining.lstrip()
    contract.require(all(name in bound for name in result), 'effective systemd environment files must be bound')
    return result


def validate_mount_plan(request, service):
    mounts = request['mounts'][service]
    contract.require(isinstance(mounts, list) and 2 <= len(mounts) <= 16, 'bounded complete mount inventory required')
    destinations = []
    sources = []
    for mount in mounts:
        oci.fields(mount, 'source destination read_only', 'exact bind mount fields required')
        source, destination = mount['source'], mount['destination']
        for name in (source, destination):
            contract.require(isinstance(name, str) and re.fullmatch(r'/[A-Za-z0-9_./-]+', name) and
                             '..' not in PurePosixPath(name).parts and str(PurePosixPath(name)) == name, 'canonical mount path required')
        contract.require(type(mount['read_only']) is bool and destination != '/', 'explicit bind mount policy required')
        p=PurePosixPath(destination)
        contract.require(all(p != other and p not in other.parents and other not in p.parents for other in destinations), 'mount destinations overlap')
        destinations.append(p);sources.append(source)
        contract.require(source in [request['databases'][service], request['socket_directory'], *request['configuration_files']] or
                         (service == 'cantos-server' and source.startswith('/var/lib/cantos/media/')), 'unbound mount or foreign database')
    db = [m for m in mounts if m['source'] == request['databases'][service]]
    contract.require(len(db) == 1 and not db[0]['read_only'] and sources.count(request['socket_directory']) == 1,
                     'exact writable database and socket mount required')
    return db[0]['destination']+('/server.db' if service == 'cantos-server' else '/identity.db')


def check_container(request, service, container, images):
    uid = 10001 if service == 'cantos-server' else 10002
    expected_db = validate_mount_plan(request, service)
    expected = sorted((m['source'],m['destination'],m['read_only']) for m in request['mounts'][service])
    actual = []
    for mount in container['Mounts']:
        contract.require(mount.get('Type') == 'bind' and type(mount.get('RW')) is bool, 'named volumes and unknown mounts refused')
        actual.append((mount['Source'],mount['Destination'],not mount['RW']))
    contract.require(sorted(actual) == expected and any(container['ImageName'] == item['ref'] and
                     'sha256:'+container['Image'].removeprefix('sha256:') == item['config_digest'] for item in images) and
                     container['Config']['User'] == f'{uid}:{uid}', 'container identity or exact mount inventory mismatch')
    contract.require(container.get('Path') == '/usr/local/bin/'+service and
                     container.get('Args') == (['serve'] if service == 'cantos-identity' else []), 'native runtime command mismatch')
    env = container['Config'].get('Env', [])
    key = 'CANTOS_SERVER_DATABASE' if service == 'cantos-server' else 'CANTOS_IDENTITY_DATABASE'
    values=[value.split('=',1)[1] for value in env if value.startswith(key+'=')]
    contract.require(values == [expected_db], 'runtime database path is not backed-up mount')
    socket_mount=next(m for m in request['mounts'][service] if m['source'] == request['socket_directory'])
    expected_socket=socket_mount['destination']+'/identity.sock'
    def value(name):
        values=[item.split('=',1)[1] for item in env if item.startswith(name+'=')]
        contract.require(len(values)==1, 'one exact native runtime setting required')
        return values[0]
    contract.require(value('CANTOS_IDENTITY_SOCKET') == expected_socket, 'native socket path mismatch')
    if service == 'cantos-server':
        contract.require(value('CANTOS_IDENTITY_PEER_UID') == '10002' and value('CANTOS_IDENTITY_PEER_POLICY') == 'direct_peer', 'native peer policy mismatch')
    else:
        contract.require(value('CANTOS_IDENTITY_SERVER_UIDS') == '10001' and value('CANTOS_IDENTITY_SOCKET_MODE') == '0660', 'native caller/socket policy mismatch')



def unit_metadata(path):
    meta=path.stat()
    return {'mode':stat.S_IMODE(meta.st_mode), 'uid':meta.st_uid, 'gid':meta.st_gid}


def replace_unit(path, data, metadata):
    atomic(path, data, metadata['mode'])
    os.chown(path, metadata['uid'], metadata['gid'])
    with path.open('rb') as stream:
        os.fsync(stream.fileno())
    sync_directory(path.parent)


def atomic(path, data, mode=0o600):
    """Durably replace one owned record/unit; retain intent if interruption happens."""
    tmp = path.with_name('.'+path.name+'.'+uuid.uuid4().hex)
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL, mode)
    with os.fdopen(fd, 'wb') as stream:
        stream.write(data)
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(tmp, path)
    sync_directory(path.parent)


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def tree_inventory(root):
    """Stopped-owner data only: regular single-link files/directories, no mounts or links."""
    base = root.lstat()
    contract.require(stat.S_ISDIR(base.st_mode) and not root.is_symlink(), 'database directory required')
    result = {}
    children=[]
    for path in root.rglob('*'):
        contract.require(len(children) < 4096, 'bounded database directory required')
        children.append(path)
    for path in [root, *sorted(children)]:
        meta = path.lstat()
        contract.require(meta.st_dev == base.st_dev and meta.st_uid == base.st_uid and meta.st_gid == base.st_gid and
                         not meta.st_mode & 0o007 and (stat.S_ISDIR(meta.st_mode) or
                         (stat.S_ISREG(meta.st_mode) and meta.st_nlink == 1)), 'foreign or linked database entry refused')
        result[str(path.relative_to(root))] = {'uid': meta.st_uid, 'gid': meta.st_gid,
                'mode': stat.S_IMODE(meta.st_mode), 'sha256': digest_file(path) if path.is_file() else None}
    return result


def cold_copy(source, destination):
    before = tree_inventory(source)
    shutil.copytree(source, destination)
    for name, record in before.items():
        p = destination / name
        os.chown(p, record['uid'], record['gid'])
        os.chmod(p, record['mode'])
        if p.is_file():
            with p.open('rb') as stream:
                os.fsync(stream.fileno())
    contract.require(before == tree_inventory(source) == tree_inventory(destination), 'backup integrity or stopped-owner drift')
    for p in sorted((p for p in destination.rglob('*') if p.is_dir()), reverse=True):
        sync_directory(p)
    sync_directory(destination)
    sync_directory(destination.parent)
    return before


def no_open_database_files(databases, proc=Path('/proc'), owner_uids=()):
    """After stopping owners, refuse other native processes retaining any DB/sidecar fd."""
    roots = [str(Path(p))+'/' for p in databases]
    identities={(p.stat().st_dev,p.stat().st_ino) for root in roots for p in Path(root).rglob('*') if p.is_file()}
    for pid in proc.iterdir():
        if not pid.name.isdigit():
            continue
        try:
            contract.require(pid.stat().st_uid not in owner_uids, 'a database owner process is still present')
            descriptors = list((pid/'fd').iterdir())
        except FileNotFoundError:
            continue
        for fd in descriptors:
            try:
                target = os.readlink(fd)
                meta = fd.stat()
            except FileNotFoundError:
                continue
            contract.require((meta.st_dev,meta.st_ino) not in identities and not any(target.startswith(root) for root in roots), 'a database owner still holds an open file')


class System:
    """Rootful existing-host adapter; bounded commands, no login/downloaded helper or shell."""
    def command(self, argv):
        env = {'PATH': '/usr/sbin:/usr/bin:/sbin:/bin', 'HOME': '/root', 'LANG': 'C'}
        if argv[0] == 'podman':
            argv = ['podman', '--remote=false', *argv[1:]]
        p = subprocess.run(argv, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, timeout=180)
        contract.require(p.returncode == 0 and len(p.stdout) <= 4*1024**2, 'native command failed; reconcile journal')
        return p.stdout

    def inspect(self, service):
        values = json.loads(self.command(['podman', 'container', 'inspect', service]))
        contract.require(len(values) == 1, 'exact container required')
        return values[0]

    def unit_policy(self, request, service):
        unit = private_path(UNITS/(service+'.container'))
        source = self.command(['systemctl', 'show', '--property=SourcePath', '--value', service+'.service']).decode().strip()
        dropins = self.command(['systemctl', 'show', '--property=DropInPaths', '--value', service+'.service']).strip()
        contract.require(source == str(unit) and not dropins, 'foreign service or systemd drop-ins refused')
        dependencies = environment_files(unit.read_bytes())
        contract.require(all(name in request['configuration_files'] for name in dependencies),
                         'every runtime environment file must be bound')
        # Quadlet's own drop-ins do not necessarily appear in systemd DropInPaths.
        for directory in ('/etc/containers/systemd','/run/containers/systemd','/usr/share/containers/systemd'):
            for name in ('container.d','cantos-.container.d',service+'.container.d'):
                contract.require(not (Path(directory)/name).exists(), 'Quadlet drop-ins unsupported')
        effective = self.command(['systemctl','show','--property=EnvironmentFiles','--value',service+'.service']).decode().strip()
        effective_environment_files(effective, request['configuration_files'])

    def host_identity(self, request, old):
        contract.require(platform.system() == 'Linux' and os.geteuid() == 0 and
                         build.MACHINES.get(platform.machine()) == old['platform'], 'native Linux/rootful platform required')
        contract.require(digest_bytes(Path('/etc/machine-id').read_bytes()) == request['host_machine_sha256'], 'host identity mismatch')
        info = json.loads(self.command(['podman', 'info', '--format', 'json']))
        contract.require(info['host']['cgroupVersion'] == 'v2' and not info['host']['security']['rootless'], 'rootful cgroup v2 required')

    def observe(self, request, old):
        self.host_identity(request, old)
        for service, uid in zip(SERVICES, (10001, 10002)):
            self.unit_policy(request, service)
            unit = private_path(UNITS/(service+'.container'))
            contract.require(digest_bytes(unit.read_bytes()) == request['units'][service], 'unit changed')
            unit_image(unit.read_bytes(), old['images'][service]['ref'], old['images'][service]['ref'])
            c = self.inspect(service)
            contract.require(c['State']['Running'], 'current container must be running')
            check_container(request, service, c, [old['images'][service]])
            db = Path(request['databases'][service]); meta = db.lstat()
            contract.require(db == db.resolve() and stat.S_ISDIR(meta.st_mode) and meta.st_uid == uid and
                             meta.st_gid == uid and not meta.st_mode & 0o007, 'database ownership mismatch')
        for name, digest in request['configuration_files'].items():
            p = private_path(name)
            contract.require(digest_bytes(p.read_bytes()) == digest, 'runtime config changed')
        socket = Path(request['socket_directory']); meta = socket.lstat()
        contract.require(socket == socket.resolve() and stat.S_ISDIR(meta.st_mode) and meta.st_uid == 10002 and
                         meta.st_gid == 10001 and meta.st_mode & stat.S_ISGID and not meta.st_mode & 0o007, 'Identity-owned shared socket directory required')
        running=json.loads(self.command(['podman','ps','--format','json']))
        for row in running:
            names=row.get('Names',[])
            if isinstance(names,str):names=[names]
            if any(name in SERVICES for name in names):continue
            identity=row.get('Id','')
            contract.require(re.fullmatch(r'[a-f0-9]{64}',identity), 'bounded container identity required')
            other=json.loads(self.command(['podman','container','inspect',identity]))[0]
            for mount in other['Mounts']:
                if mount.get('Type') == 'bind':
                    source=Path(mount['Source'])
                    contract.require(all(source != Path(db) and source not in Path(db).parents and Path(db) not in source.parents
                                         for db in request['databases'].values()), 'another container owns a database bind')
        sizes = [p.stat().st_size for service in SERVICES for p in Path(request['databases'][service]).rglob('*') if p.is_file()]
        capacity = os.statvfs(STATE)
        contract.require(shutil.disk_usage(STATE).free >= sum(sizes)*2+64*1024**2 and
                         capacity.f_favail >= len(sizes)+100, 'insufficient backup capacity/inodes')

    def recovery_host(self, request, old, new):
        self.host_identity(request, old)
        for service in SERVICES:
            self.unit_policy(request, service)
        # The only running containers that may remain are from the reviewed pair.
        for service in SERVICES:
            running = json.loads(self.command(['podman', 'ps', '--format', 'json', '--filter', 'name=^'+service+'$']))
            if running:
                c = self.inspect(service)
                check_container(request, service, c, [old['images'][service], new['images'][service]])
        for service, uid in zip(SERVICES, (10001, 10002)):
            p = Path(request['databases'][service]); meta = p.lstat()
            contract.require(p == p.resolve() and meta.st_uid == uid and stat.S_ISDIR(meta.st_mode) and
                             not meta.st_mode & 0o007, 'recovery database ownership drift')

    def pull(self, release):
        for service in SERVICES:
            item = release['images'][service]
            self.command(['podman', 'pull', item['ref']])
            values = json.loads(self.command(['podman', 'image', 'inspect', item['ref']]))
            contract.require(len(values) == 1 and item['ref'] in values[0].get('RepoDigests', []), 'pulled registry digest mismatch')
            actual = build.check_local_image(service, values[0], release['source_sha'], release['configuration_sha256'], release['platform'])
            contract.require(actual == item['config_digest'], 'pulled OCI config mismatch')

    def stop(self, request):
        for service in SERVICES:
            self.command(['systemctl', 'stop', service+'.service'])
            contract.require(self.command(['systemctl', 'show', '--property=ActiveState', '--value', service+'.service']).strip() == b'inactive', 'unit did not stop')
            running = json.loads(self.command(['podman', 'ps', '--format', 'json', '--filter', 'name=^'+service+'$']))
            contract.require(running == [], 'database owner still running')
        no_open_database_files(request['databases'].values(), owner_uids=(10001,10002))

    def start(self, request, release):
        self.command(['systemctl', 'daemon-reload'])
        for service in START:
            self.command(['systemctl', 'start', service+'.service'])
            self.command(['systemctl', 'is-active', '--quiet', service+'.service'])
        for service in SERVICES:
            c = self.inspect(service)
            contract.require(c['State']['Running'], 'started service must be running')
            check_container(request, service, c, [release['images'][service]])
        socket=Path(request['socket_directory'])/'identity.sock'; meta=socket.lstat()
        contract.require(stat.S_ISSOCK(meta.st_mode) and meta.st_uid == 10002 and meta.st_gid == 10001 and
                         stat.S_IMODE(meta.st_mode) == 0o660, 'started socket ownership mismatch')
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
        with opener.open(request['health_url'], timeout=10) as response:
            contract.require(response.status == 200 and len(response.read(65537)) <= 65536, 'health observation failed')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


@contextlib.contextmanager
def locked(state):
    private_path(state, directory=True)
    contract.require(not state.stat().st_mode & 0o077, 'private release state required')
    fd = os.open(state/'lock', os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
    try:
        contract.require(os.fstat(fd).st_nlink == 1 and os.fstat(fd).st_uid == os.geteuid(), 'foreign lock')
        fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield
    finally:
        os.close(fd)


def upgrade(request, old, new, manifest_digest, ports, state=STATE):
    """Under one host lock, preserve recovery bytes before changing either unit.

    Recovery is explicit: an intent marker always blocks replay after failure/crash.
    No DB migration, destructive restore or automatic rollback is ever performed.
    """
    with locked(state):
        contract.require(not (state/'in-progress.json').exists(), 'reconcile interrupted operation before replay')
        current = pinned(state/'current.json', None)
        contract.require(current['generation'] == request['generation'] and
                         current['manifest_sha256'] == request['current_manifest_sha256'] and
                         current['images'] == old['images'], 'host generation/current pair changed')
        ports.observe(request, old)
        attempt = state/('attempt-'+uuid.uuid4().hex)
        attempt.mkdir(mode=0o700)
        old_units = {s: (UNITS/(s+'.container')).read_bytes() for s in SERVICES}
        metadata = {s: unit_metadata(UNITS/(s+'.container')) for s in SERVICES}
        atomic(attempt/'unit-metadata.json', contract.canonical(metadata))
        for s, data in old_units.items():
            atomic(attempt/(s+'.container'), data)
        atomic(attempt/'previous.json', contract.canonical(current))
        atomic(attempt/'request.json', contract.canonical(request))
        marker = {'attempt': attempt.name, 'state': 'RECOVERY_REQUIRED', 'target_manifest_sha256': manifest_digest}
        atomic(state/'in-progress.json', contract.canonical(marker))
        ports.pull(new)  # Exact digest only; no retries after failure.
        ports.stop(request)
        backups = {}
        for s in SERVICES:
            backups[s] = cold_copy(Path(request['databases'][s]), attempt/(s+'-database'))
        atomic(attempt/'backup.json', contract.canonical(backups))
        # Recheck immutable unit/config inputs immediately before replacement, after stop/copy.
        for s in SERVICES:
            path = UNITS/(s+'.container')
            contract.require(digest_bytes(path.read_bytes()) == request['units'][s] and unit_metadata(path) == metadata[s], 'unit drift after backup')
        for name, digest in request['configuration_files'].items():
            contract.require(digest_bytes(private_path(name).read_bytes()) == digest, 'configuration drift after backup')
        for s in SERVICES:
            replace_unit(UNITS/(s+'.container'), unit_image(old_units[s], old['images'][s]['ref'], new['images'][s]['ref']), metadata[s])
        ports.start(request, new)
        updated = {'generation': current['generation']+1, 'manifest_sha256': manifest_digest,
                   'images': new['images'], 'previous_manifest_sha256': current['manifest_sha256']}
        atomic(state/'current.json', contract.canonical(updated))
        atomic(attempt/'receipt.json', contract.canonical({'operation': 'ACTIVATED', **updated, 'backup': 'PASS',
                                                        'request_sha256': digest_bytes(contract.canonical(request))}))
        (state/'in-progress.json').unlink()
        sync_directory(state)
        return {'operation': 'ACTIVATED', 'generation': updated['generation'], 'attempt': attempt.name}


def rollback(request, old, new, attempt_name, ports, state=STATE):
    """Explicit image-only recovery, same schema/config; never copy a backup over data."""
    contract.require(re.fullmatch(r"attempt-[a-f0-9]{32}", attempt_name) is not None, 'invalid recovery attempt')
    with locked(state):
        attempt = private_path(state/attempt_name, directory=True)
        saved = pinned(attempt/'request.json', None)
        contract.require(saved == request and not (attempt/'rollback.json').exists(), 'recovery request mismatch or already rolled back')
        marker = state/'in-progress.json'
        if marker.exists():
            contract.require(pinned(marker, None)['attempt'] == attempt_name, 'another operation needs recovery')
        metadata = pinned(attempt/'unit-metadata.json', None)
        current = pinned(state/'current.json', None)
        contract.require((current['generation'] == request['generation'] and current['images'] == old['images'] and
                          current['manifest_sha256'] == request['previous']['manifest_digest']) or
                         (current['generation'] == request['generation']+1 and current['images'] == new['images'] and
                          current['manifest_sha256'] == request['next']['manifest_digest']), 'recovery generation drift')
        # Prove current unit/config bytes are only the exact old or new generation.
        for service in SERVICES:
            original = private_path(attempt/(service+'.container')).read_bytes()
            contract.require(digest_bytes(original) == request['units'][service], 'saved unit corrupt')
            updated = unit_image(original, old['images'][service]['ref'], new['images'][service]['ref'])
            contract.require(private_path(UNITS/(service+'.container')).read_bytes() in (original, updated), 'foreign recovery unit')
        for name, digest in request['configuration_files'].items():
            contract.require(digest_bytes(private_path(name).read_bytes()) == digest, 'recovery config changed')
        def fence():
            contract.require(pinned(state/'current.json', None) == current, 'recovery generation changed')
            for service in SERVICES:
                path=private_path(UNITS/(service+'.container'))
                original=private_path(attempt/(service+'.container')).read_bytes()
                contract.require(digest_bytes(original) == request['units'][service] and unit_metadata(path) == metadata[service] and
                                 path.read_bytes() in (original,unit_image(original,old['images'][service]['ref'],new['images'][service]['ref'])),
                                 'recovery unit drift')
            for name,digest in request['configuration_files'].items():
                contract.require(digest_bytes(private_path(name).read_bytes()) == digest, 'recovery config changed')
            ports.recovery_host(request, old, new)
        fence()
        atomic(marker, contract.canonical({'attempt': attempt_name, 'state': 'RECOVERY_REQUIRED'}))
        ports.pull(old)
        fence()
        ports.stop(request)
        fence()
        for service in SERVICES:
            replace_unit(UNITS/(service+'.container'), (attempt/(service+'.container')).read_bytes(), metadata[service])
        ports.start(request, old)
        restored = {'generation': current['generation']+1, 'manifest_sha256': request['previous']['manifest_digest'],
                    'images': old['images'], 'previous_manifest_sha256': request['next']['manifest_digest']}
        atomic(state/'current.json', contract.canonical(restored))
        atomic(attempt/'rollback.json', contract.canonical({'operation': 'ROLLED_BACK', **restored,
                                                         'database_restore': 'NOT_PERFORMED'}))
        marker.unlink()
        sync_directory(state)
        return {'operation': 'ROLLED_BACK', 'generation': restored['generation'], 'database_restore': 'NOT_PERFORMED'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--request', required=True, type=Path)
    parser.add_argument('--request-digest', required=True)
    parser.add_argument('--rollback-attempt', help='explicit recovery of one retained attempt; no database restore')
    parser.add_argument('--execute', action='store_true', help='explicit existing-host write; default is offline validation')
    args = parser.parse_args()
    try:
        request = pinned(args.request, args.request_digest)
        root = Path(__file__).resolve().parents[1]
        old, new = (release(root, request[key]) for key in ('previous', 'next'))
        decision = decide(request, old, new, time.time(), recovery=bool(args.rollback_attempt))
        if args.execute:
            decision = (rollback(request, old, new, args.rollback_attempt, System()) if args.rollback_attempt else
                        upgrade(request, old, new, request['next']['manifest_digest'], System()))
        print(json.dumps(decision, sort_keys=True))
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError, json.JSONDecodeError):
        parser.exit(1, 'native upgrade refused or interrupted; inspect private intent/backup records before recovery; no automatic replay\n')


if __name__ == '__main__':
    main()
