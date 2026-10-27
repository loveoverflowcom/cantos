"""Strict adoption-only Podman representations and container-bound bridge readiness.

No address is operator supplied. A bridge selector names a reviewed network identity/subnet;
the running exact container supplies its current address. This module never changes networking.
"""
import ipaddress
import os
from pathlib import Path
import re

import release_artifact as contract
import release_oci as oci

HEALTH_PATH = '/api/v1/theatre/works'
CHECKS = ['real-podman-inspect-and-proc-argv', 'private-bridge-http-no-published-port', 'foreign-network-refused']
PRIVATE_NETWORKS = tuple(ipaddress.ip_network(value) for value in ('10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16'))


def selector(value):
    if isinstance(value, str):
        match = re.fullmatch(r'http://127\.0\.0\.1:([1-9][0-9]{0,4})/[A-Za-z0-9_/-]+', value)
        contract.require(match is not None and int(match[1]) <= 65535, 'explicit loopback health required')
        return 'loopback'
    oci.fields(value, 'kind network network_id subnet port path', 'exact bridge health selector required')
    contract.require(value['kind'] == 'podman-bridge' and
                     isinstance(value['network'], str) and re.fullmatch(r'[a-zA-Z0-9][a-zA-Z0-9_-]{0,62}', value['network']) and
                     re.fullmatch(r'[a-f0-9]{64}', value['network_id']), 'bound bridge network required')
    subnet = ipaddress.ip_network(value['subnet'], strict=True)
    contract.require(isinstance(subnet, ipaddress.IPv4Network) and
                     any(subnet.subnet_of(private) for private in PRIVATE_NETWORKS), 'private IPv4 bridge subnet required')
    contract.require(type(value['port']) is int and value['port'] == 8080 and value['path'] == HEALTH_PATH,
                     'exact Server readiness route required')
    return 'podman-bridge'


def command_view(service, container):
    """Only two observed Server inspect encodings; actual process argv is checked separately."""
    entry = '/usr/local/bin/'+service
    tail = ['serve'] if service == 'cantos-identity' else []
    config = container['Config']
    contract.require(service in oci.SERVICES and container.get('Path') == entry and
                     config.get('Entrypoint') == [entry] and
                     (config.get('Cmd') == tail or (not tail and config.get('Cmd') is None)) and
                     (container.get('Args') == tail or (not tail and container.get('Args') == [entry])),
                     'native inspected command mismatch')
    return {**container, 'Args': tail}


def process_identity(container):
    state = container['State']
    contract.require(isinstance(container.get('Id'), str) and re.fullmatch(r'[a-f0-9]{64}', container['Id']) and
                     state.get('Running') is True and type(state.get('Pid')) is int and state['Pid'] > 1 and
                     isinstance(state.get('StartedAt'), str) and state['StartedAt'], 'running process identity required')
    return container['Id'], state['Pid'], state['StartedAt']


def check_process(service, container, inspect_again, *, proc=Path('/proc')):
    identity = process_identity(container)
    expected = ('/usr/local/bin/'+service).encode()+b'\0'+(b'serve\0' if service == 'cantos-identity' else b'')
    # Open the PID directory once: a disappearing/reused PID must not retarget the cmdline read.
    directory = os.open(proc/str(identity[1]), os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC)
    try:
        descriptor = os.open('cmdline', os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=directory)
        try:
            contract.require(os.read(descriptor, 4097) == expected, 'native process argv mismatch')
        finally:
            os.close(descriptor)
        contract.require(process_identity(inspect_again()) == identity, 'native process changed during observation')
    finally:
        os.close(directory)


def bridge_url(value, container, network):
    contract.require(selector(value) == 'podman-bridge', 'bridge selector required')
    process_identity(container)
    contract.require(network.get('name') == value['network'] and network.get('id') == value['network_id'] and
                     network.get('driver') == 'bridge', 'bridge network identity mismatch')
    subnets = network.get('subnets')
    contract.require(isinstance(subnets, list) and len(subnets) == 1 and
                     subnets[0].get('subnet') == value['subnet'], 'bridge subnet changed or ambiguous')
    settings, host = container['NetworkSettings'], container['HostConfig']
    networks = settings['Networks']
    contract.require(set(networks) == {value['network']} and
                     host.get('NetworkMode') in ('bridge', value['network'], value['network_id']) and
                     host.get('PortBindings') in (None, {}) and
                     all(binding in (None, []) for binding in (settings.get('Ports') or {}).values()),
                     'exclusive unpublished bridge required')
    endpoint = networks[value['network']]
    subnet = ipaddress.ip_network(value['subnet'])
    address = ipaddress.ip_address(endpoint['IPAddress'])
    gateway = ipaddress.ip_address(subnets[0]['gateway'])
    contract.require(endpoint.get('NetworkID') == value['network_id'] and
                     endpoint.get('IPPrefixLen') == subnet.prefixlen and endpoint.get('Gateway') == str(gateway) and
                     address in subnet and gateway in subnet and
                     address not in (subnet.network_address, subnet.broadcast_address, gateway) and
                     gateway not in (subnet.network_address, subnet.broadcast_address), 'container bridge address mismatch')
    return f'http://{address}:8080{HEALTH_PATH}'


def bridge_health(value, inspect_container, inspect_network, check_container, read_http):
    before = inspect_container()
    check_container(before)
    identity = process_identity(before)
    address = bridge_url(value, before, inspect_network())
    read_http(address)
    after = inspect_container()
    check_container(after)
    contract.require(process_identity(after) == identity and
                     bridge_url(value, after, inspect_network()) == address,
                     'health target changed during observation')
