"""Exercise the adoption adapter on the disposable qualifier's exact native image pair.

Creates only a uniquely named private bridge and fixture containers, with no published port.
The normal four gates retain their isolated network policy; this is separate host-adapter evidence.
"""
import copy
import json
import time

import release_artifact as contract
import release_native_adapter as adapter
import release_native_migration as migration
import release_native_forward as forward
import release_oci as oci


def run(fixture):
    f = fixture
    name = f.prefix+'-bridge'
    names = {service: f.prefix+'-bridge-'+service for service in oci.SERVICES}
    created = False
    try:
        f.adapter_phase = 'BRIDGE_CREATE'
        f.pod('network', 'create', '--internal', '--subnet', '10.231.42.0/24', name)
        created = True
        network = json.loads(f.pod('network', 'inspect', name))[0]
        f.adapter_phase = 'BRIDGE_START'
        for service in reversed(oci.SERVICES):
            options = f.options(service)
            index = options.index('--tmpfs')
            options = options[:index]+options[index+2:]  # Adoption's complete bind inventory refuses other mount types.
            f.names.append(names[service])
            f.pod('run', '-d', '--name', names[service], '--network', name if service == 'cantos-server' else 'none',
                  *options, *(['--env', 'CANTOS_BIND_ADDRESS=0.0.0.0:8080'] if service == 'cantos-server' else []),
                  f.image(service), *(['serve'] if service == 'cantos-identity' else []))
        request = {'databases': {s: str(p) for s, p in f.databases.items()}, 'socket_directory': str(f.socket_dir),
                   'configuration_files': {}, 'peer_policy': 'direct_peer',
                   'health_url': {'kind': 'podman-bridge', 'network': name, 'network_id': network['id'],
                                  'subnet': '10.231.42.0/24', 'port': 8080, 'path': adapter.HEALTH_PATH},
                   'mounts': {s: [{'source': str(f.databases[s]), 'destination': '/database', 'read_only': False},
                                  {'source': str(f.socket_dir), 'destination': '/identity', 'read_only': s == 'cantos-server'}]
                              for s in oci.SERVICES}}

        class FixtureSystem(migration.AdoptionSystem):
            def inspect(self, service):
                return json.loads(f.pod('inspect', names[service]))[0]

            def command(self, argv):
                contract.require(argv[:3] == ['podman', 'network', 'inspect'], 'fixture command boundary')
                return f.pod(*argv[1:])

        system = FixtureSystem()
        f.adapter_phase = 'BRIDGE_RUNTIME_AND_HTTP'
        # Whitelisted shape names only: never emit argv, env, addresses or subprocess stderr.
        f.adapter_shapes = {service: type(system.inspect(service)['Config'].get('Entrypoint')).__name__
                            for service in oci.SERVICES}
        deadline = time.monotonic()+30
        while True:
            try:
                system.health(request, f.manifest)
                break
            except OSError:
                contract.require(time.monotonic() < deadline, 'BRIDGE_HTTP_NOT_READY')
                time.sleep(0.2)
        for service in oci.SERVICES:
            f.adapter_phase = 'BRIDGE_PROCESS_CHECK'
            system.check_container(request, service, system.inspect(service), [f.manifest['images'][service]])
        wrong = copy.deepcopy(request)
        f.adapter_phase = 'BRIDGE_FOREIGN_NETWORK_CONTROL'
        wrong['health_url']['network_id'] = '0'*64
        try:
            system.health(wrong, f.manifest)
        except ValueError:
            pass
        else:
            raise ValueError('BRIDGE_FOREIGN_NETWORK_ACCEPTED')

        # Exercise the new recovery observer against these real containers and /proc. The
        # unit and host-identity seams remain fixture doubles, so this is explicitly not a
        # full systemd or transition-executor qualification.
        class FixtureForwardSystem(forward.ForwardSystem):
            def host_identity(self, request, release):
                pass

            def unit_policy(self, request, service):
                pass

            def inspect(self, service):
                return json.loads(f.pod('inspect', names[service]))[0]

            def command(self, argv):
                contract.require(argv and argv[0] == 'podman', 'fixture podman boundary')
                args = list(argv[1:])
                for service in oci.SERVICES:
                    token = 'name=^'+service+'$'
                    if token in args:
                        args[args.index(token)] = 'name=^'+names[service]+'$'
                raw = f.pod(*args)
                if args[:3] == ['ps', '--format', 'json'] and '--filter' not in args:
                    rows = json.loads(raw)
                    for row in rows:
                        actual = row.get('Names', [])
                        if isinstance(actual, str): actual = [actual]
                        row['Names'] = [next((service for service in oci.SERVICES if name == names[service]), name)
                                        for name in actual]
                    return json.dumps(rows).encode()
                return raw

        f.adapter_phase = 'FORWARD_BRIDGE_RUNTIME'
        forward_system = FixtureForwardSystem()
        forward_system.recovery_host(request, f.manifest, f.manifest)
        # A real third container with a read-only database bind must still be rejected even
        # without an open database file. It has no network and is removed in the finally block.
        foreign = f.prefix+'-foreign-bind'
        foreign_started = False
        try:
            f.pod('run', '-d', '--name', foreign, '--network=none', '--read-only', '--cap-drop=ALL',
                  '--security-opt=no-new-privileges', '--user', '10001:10001',
                  '--mount', f'type=bind,src={f.databases["cantos-server"]},dst=/foreign,ro=true',
                  '--entrypoint', '/bin/sleep', f.image('cantos-server'), '120')
            foreign_started = True
            f.names.append(foreign)
            try:
                forward_system.recovery_host(request, f.manifest, f.manifest)
            except ValueError as error:
                contract.require(str(error) == 'another container owns a database bind', 'unexpected foreign bind refusal')
            else:
                raise ValueError('FORWARD_FOREIGN_DATABASE_BIND_ACCEPTED')
        finally:
            if foreign_started:
                f.pod('rm', '-f', foreign)
                f.names.remove(foreign)
        f.forward_bridge_record = {'status': 'PASS', 'source_sha': f.manifest['source_sha'],
                                   'images': f.manifest['images'],
                                   'checks': ['real-podman-forward-recovery-host','real-proc-argv','foreign-database-bind-fence'],
                                   'systemd_unit_policy': 'NOT_RUN', 'executor_transition': 'NOT_RUN', 'activation': 'NOT_RUN'}
        return {'status': 'PASS', 'source_sha': f.manifest['source_sha'], 'images': f.manifest['images'],
                'checks': adapter.CHECKS, 'activation': 'NOT_RUN'}
    finally:
        for container in names.values():
            if container in f.names:
                f.pod('rm', '-f', container)
                f.names.remove(container)
        if created:
            f.pod('network', 'rm', name)
