"""Real-file process observations and deterministic topology/HTTP refusal controls."""
import copy
from pathlib import Path
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, HTTPServer
from unittest.mock import patch
from urllib.error import HTTPError

import release_native as native
import release_native_adapter as adapter
import release_native_migration as migration
from test_release_native_migration import adoption_request, releases


def selector():
    return {'kind': 'podman-bridge', 'network': 'fixture-net', 'network_id': 'a'*64,
            'subnet': '10.42.0.0/24', 'port': 8080, 'path': adapter.HEALTH_PATH}


def container(service='cantos-server'):
    entry = '/usr/local/bin/'+service
    return {'Id': 'b'*64, 'Path': entry, 'Args': ['serve'] if service == 'cantos-identity' else [entry],
            'Config': {'Entrypoint': [entry], 'Cmd': ['serve'] if service == 'cantos-identity' else None},
            'State': {'Running': True, 'Pid': 12345, 'StartedAt': 'fixture-start'},
            'HostConfig': {'NetworkMode': 'fixture-net', 'PortBindings': {}},
            'NetworkSettings': {'Ports': {'8080/tcp': None}, 'Networks': {'fixture-net': {
                'NetworkID': 'a'*64, 'IPAddress': '10.42.0.15', 'IPPrefixLen': 24, 'Gateway': '10.42.0.1'}}}}


def network():
    return {'name': 'fixture-net', 'id': 'a'*64, 'driver': 'bridge',
            'subnets': [{'subnet': '10.42.0.0/24', 'gateway': '10.42.0.1'}]}


class Commands(unittest.TestCase):
    def test_only_reviewed_inspect_shapes_have_one_semantic_command(self):
        for service in ('cantos-server', 'cantos-identity'):
            c = container(service)
            expected = ['serve'] if service == 'cantos-identity' else []
            self.assertEqual(adapter.command_view(service, c)['Args'], expected)
            c['Config']['Entrypoint'] = '/usr/local/bin/'+service
            self.assertEqual(adapter.command_view(service, c)['Args'], expected)
            if service == 'cantos-server':
                c['Args'] = []
                c['Config']['Cmd'] = []
                self.assertEqual(adapter.command_view(service, c)['Args'], [])
        c = container()
        self.assertEqual(c['Args'], ['/usr/local/bin/cantos-server'])  # Input never normalized in place.

    def test_extra_args_shell_wrapper_migrate_and_config_disagreement_refuse(self):
        for field, value in [('Path', '/bin/sh'), ('Args', ['--help']), ('Args', ['/usr/local/bin/cantos-server', '--help']),
                             ('Args', ['/usr/local/bin/cantos-server', '/usr/local/bin/cantos-server'])]:
            with self.subTest(field=field, value=value), self.assertRaisesRegex(ValueError, 'inspected command'):
                adapter.command_view('cantos-server', container() | {field: value})
        for field, value in [('Entrypoint', ['/bin/sh']), ('Entrypoint', '/usr/local/bin/cantos-server --help'),
                             ('Entrypoint', '/bin/sh'), ('Entrypoint', None), ('Cmd', ['--help'])]:
            c = container(); c['Config'][field] = value
            with self.assertRaisesRegex(ValueError, 'inspected command'):
                adapter.command_view('cantos-server', c)
        c = container('cantos-identity'); c['Args'] = ['migrate']
        with self.assertRaisesRegex(ValueError, 'inspected command'):
            adapter.command_view('cantos-identity', c)

    def test_actual_nul_delimited_process_argv_and_stable_identity_are_mandatory(self):
        for service in ('cantos-server', 'cantos-identity'):
            c = container(service)
            expected = ('/usr/local/bin/'+service).encode()+b'\0'+(b'serve\0' if service == 'cantos-identity' else b'')
            with tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary); directory = root/str(c['State']['Pid']); directory.mkdir()
                path = directory/'cmdline'; path.write_bytes(expected)
                adapter.check_process(service, c, lambda: c, proc=root)
                for bad in (b'', expected[:-1], expected+b'--help\0', b'/bin/sh\0', b'x'*4097):
                    path.write_bytes(bad)
                    with self.assertRaisesRegex(ValueError, 'process argv'):
                        adapter.check_process(service, c, lambda: c, proc=root)
                path.write_bytes(expected)
                for field, value in [('Pid', 12346), ('StartedAt', 'another-start'), ('Running', False)]:
                    changed = copy.deepcopy(c); changed['State'][field] = value
                    with self.assertRaisesRegex(ValueError, 'process'):
                        adapter.check_process(service, c, lambda: changed, proc=root)
                path.unlink(); path.symlink_to('/dev/null')
                with self.assertRaises(OSError):
                    adapter.check_process(service, c, lambda: c, proc=root)


class Health(unittest.TestCase):
    def test_http_reader_refuses_redirects_and_oversized_bodies_without_ambient_proxy(self):
        hits = []
        class Handler(BaseHTTPRequestHandler):
            def do_GET(self):
                hits.append(self.path)
                self.send_response(308 if self.path == '/redirect' else 200)
                if self.path == '/redirect': self.send_header('Location', '/foreign')
                self.end_headers()
                self.wfile.write(b'x'*65537 if self.path == '/large' else b'{}')

            def log_message(self, *_):
                pass
        with HTTPServer(('127.0.0.1', 0), Handler) as server:
            thread = threading.Thread(target=server.serve_forever); thread.start()
            try:
                origin = 'http://127.0.0.1:'+str(server.server_port)
                with patch.dict('os.environ', {'http_proxy': 'http://127.0.0.1:1', 'no_proxy': ''}):
                    native.System().read_health(origin+'/ok')
                    with self.assertRaises(HTTPError): native.System().read_health(origin+'/redirect')
                    with self.assertRaisesRegex(ValueError, 'health observation'): native.System().read_health(origin+'/large')
                self.assertEqual(hits, ['/ok', '/redirect', '/large'])
            finally:
                server.shutdown(); thread.join()

    def test_descriptor_is_a_selector_not_an_operator_address(self):
        value = selector()
        request = adoption_request() | {'health_url': value}
        self.assertEqual(migration.decide(request, *releases(),  request['observed_at'])['activation'], 'NOT_RUN')
        self.assertEqual(adapter.bridge_url(value, container(), network()), 'http://10.42.0.15:8080'+adapter.HEALTH_PATH)
        legacy = container(); legacy['NetworkSettings']['Networks']['fixture-net']['NetworkID'] = 'fixture-net'
        self.assertEqual(adapter.bridge_url(value, legacy, network()), 'http://10.42.0.15:8080'+adapter.HEALTH_PATH)
        with self.assertRaisesRegex(ValueError, 'network identity'):
            adapter.bridge_url(value, legacy, network() | {'id': 'c'*64})
        changed = container(); changed['NetworkSettings']['Networks']['fixture-net']['IPAddress'] = '10.42.0.20'
        self.assertEqual(adapter.bridge_url(value, changed, network()), 'http://10.42.0.20:8080'+adapter.HEALTH_PATH)
        for patch in [{'address': '10.42.0.99'}, {'port': 80}, {'port': True}, {'path': '/admin'},
                      {'subnet': '169.254.0.0/16'}, {'subnet': '8.8.8.0/24'}, {'subnet': '::/0'},
                      {'network': 'host'}, {'network_id': '../elsewhere'}]:
            v = value | patch
            if patch == {'network': 'host'}:
                # Valid identifier alone never authorizes host networking.
                with self.assertRaisesRegex(ValueError, 'network identity'):
                    adapter.bridge_url(v, container(), network())
            else:
                with self.subTest(patch=patch), self.assertRaises(ValueError):
                    adapter.selector(v)
        for url in ['http://10.42.0.15:8080'+adapter.HEALTH_PATH, 'http://localhost:8080/health',
                    'http://127.0.0.1:8080/health?x=1', 'https://127.0.0.1/health']:
            with self.assertRaisesRegex(ValueError, 'loopback'):
                adapter.selector(url)

    def test_topology_drift_foreign_address_and_published_ports_refuse_before_http(self):
        for address in ['127.0.0.1', '10.43.0.2', '10.42.0.0', '10.42.0.1', '10.42.0.255', '169.254.169.254', '8.8.8.8']:
            c = container(); c['NetworkSettings']['Networks']['fixture-net']['IPAddress'] = address
            with self.subTest(address=address), self.assertRaisesRegex(ValueError, 'address mismatch'):
                adapter.bridge_url(selector(), c, network())
        mutations = [lambda c: c['HostConfig'].update(NetworkMode='host'),
                     lambda c: c['HostConfig'].update(PortBindings={'8080/tcp': [{'HostPort': '8080'}]}),
                     lambda c: c['NetworkSettings']['Networks'].update(foreign={}),
                     lambda c: c['NetworkSettings']['Networks']['fixture-net'].update(NetworkID='foreign-name'),
                     lambda c: c['NetworkSettings']['Networks']['fixture-net'].update(NetworkID='c'*64),
                     lambda c: c['NetworkSettings']['Networks']['fixture-net'].update(Gateway=''),
                     lambda c: c['NetworkSettings']['Networks']['fixture-net'].update(IPPrefixLen=16)]
        for mutate in mutations:
            c = container(); mutate(c); reads = []
            with self.assertRaises(ValueError):
                adapter.bridge_health(selector(), lambda: c, network, lambda _: None, reads.append)
            self.assertEqual(reads, [])
        for patch in [{'id': 'c'*64}, {'driver': 'macvlan'}, {'subnets': []}]:
            with self.assertRaisesRegex(ValueError, 'network|subnet'):
                adapter.bridge_url(selector(), container(), network() | patch)

    def test_runtime_and_network_are_rechecked_after_http(self):
        c = container(); calls = []
        adapter.bridge_health(selector(), lambda: c, network, lambda _: calls.append('check'), calls.append)
        self.assertEqual(calls, ['check', 'http://10.42.0.15:8080'+adapter.HEALTH_PATH, 'check'])
        for change in ('address', 'process', 'network'):
            c = container(); net = network()
            def read(_):
                if change == 'address': c['NetworkSettings']['Networks']['fixture-net']['IPAddress'] = '10.42.0.16'
                if change == 'process': c['State']['StartedAt'] = 'restarted'
                if change == 'network': net['id'] = 'c'*64
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'changed|identity'):
                adapter.bridge_health(selector(), lambda: c, lambda: net, lambda _: None, read)


if __name__ == '__main__':
    unittest.main()
