"""Render synthetic Quadlet fixture inputs; no file writes, network, or release evidence.

This prototype is deliberately separate from the production executor. A future disposable
runner harness must verify the rendered units with actual systemd and the real host adapter.
"""
import re

SERVICES = ('cantos-server', 'cantos-identity')
ROOT = '/root/cantos-native'
ENV = {
    'cantos-server': {
        'CANTOS_ENV': 'production', 'CANTOS_BIND_ADDRESS': '0.0.0.0:8080',
        'CANTOS_SERVER_DATABASE': '/database/server.db',
        'CANTOS_STUDIO_WEB_ORIGIN': 'https://studio.example.test',
        'CANTOS_THEATRE_WEB_ORIGIN': 'https://theatre.example.test',
        'CANTOS_IDENTITY_SOCKET': '/identity/identity.sock', 'CANTOS_IDENTITY_PEER_UID': '10002',
        'CANTOS_IDENTITY_PEER_POLICY': 'direct_peer',
    },
    'cantos-identity': {
        'CANTOS_IDENTITY_DATABASE': '/database/identity.db', 'CANTOS_IDENTITY_SOCKET': '/identity/identity.sock',
        'CANTOS_IDENTITY_SOCKET_MODE': '0660', 'CANTOS_IDENTITY_SERVER_UIDS': '10001',
        'CANTOS_IDENTITY_SIGNUP_LINK_BASE': 'https://theatre.example.test/signup',
        'CANTOS_IDENTITY_MAIL_PROVIDER': 'smtp', 'CANTOS_IDENTITY_SMTP_HOST': '127.0.0.1',
        'CANTOS_IDENTITY_SMTP_PORT': '2525', 'CANTOS_IDENTITY_SMTP_TLS': 'loopback-plaintext',
        'CANTOS_IDENTITY_SMTP_FROM': 'cantos@example.test',
    },
}


def render(service, image, network):
    if service not in SERVICES or not re.fullmatch(
            r'ghcr\.io/loveoverflowcom/'+service+r'@sha256:[a-f0-9]{64}', image):
        raise ValueError('exact service image digest required')
    if not re.fullmatch(r'cantos-rehearsal-[a-f0-9]{32}', network):
        raise ValueError('unique fixture network required')
    server = service == 'cantos-server'
    db = ROOT+('/server-db' if server else '/identity-db')
    env_path = ROOT+'/config/'+('server.env' if server else 'identity.env')
    uid = '10001:10001' if server else '10002:10002'
    selected_network = network if server else 'none'
    socket = ROOT+'/run:/identity'+(':ro' if server else '')
    unit = ('[Unit]\nDescription=Disposable Cantos executor rehearsal\n'
            f'[Service]\nExecStartPre=/usr/bin/podman image exists {image}\n'
            f'[Container]\nContainerName={service}\nImage={image}\nPull=never\nEnvironmentFile={env_path}\nUser={uid}\n'
            f'Network={selected_network}\nReadOnly=true\nNoNewPrivileges=true\nDropCapability=ALL\n'
            f'Volume={db}:/database\nVolume={socket}\n'+('Exec=serve\n' if not server else '')+
            '[Install]\nWantedBy=multi-user.target\n')
    env = ''.join(key+'='+value+'\n' for key, value in ENV[service].items())
    return {'unit': unit.encode(), 'environment_path': env_path, 'environment': env.encode(),
            'database_path': db, 'socket_directory': ROOT+'/run'}
