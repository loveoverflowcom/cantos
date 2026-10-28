"""Synthetic private-bridge signup/login journey for the disposable executor fixture.

All proof and credential bytes remain in private in-memory pipes. They are test-only and never
included in output or evidence. This module does not operate on a production host.
"""
import base64
import email
import email.policy
import http.client
import json
from pathlib import Path
import re
import select
import subprocess
import sys
import urllib.parse

import release_native_adapter as adapter

EMAIL = 'executor-rehearsal@example.test'
PASSWORD = 'Synthetic-fixture-only-4738!'
BASE = '/api/v1/theatre/native/identity/'


def line(process):
    if process.stdout is None or not select.select([process.stdout], [], [], 60)[0]:
        raise ValueError('fixture smtp timeout')
    raw = process.stdout.readline(65537)
    if not raw or len(raw) > 65536:
        raise ValueError('fixture smtp response bounded')
    return json.loads(raw)


def request(address, method, path, expected, payload=None, headers=None):
    connection = http.client.HTTPConnection(address, 8080, timeout=20)
    try:
        body = None if payload is None else json.dumps(payload)
        connection.request(method, path, body, {'Content-Type': 'application/json', **(headers or {})})
        response = connection.getresponse()
        data = response.read(65537)
        if response.status != expected or len(data) > 65536:
            raise ValueError('fixture HTTP response mismatch')
        return json.loads(data) if data else None
    finally:
        connection.close()


def server_address(system, layout):
    selector = layout['health_url']
    container = system.inspect('cantos-server')
    networks = json.loads(system.command(['podman', 'network', 'inspect', selector['network']]))
    if len(networks) != 1:
        raise ValueError('fixture network ambiguous')
    url = adapter.bridge_url(selector, container, networks[0])
    return urllib.parse.urlsplit(url).hostname


def signup(system, layout):
    identity = system.inspect('cantos-identity')
    _, pid, _ = adapter.process_identity(identity)
    # The isolated network belongs to this synthetic container; no production network is touched.
    system.command(['nsenter', '-t', str(pid), '-n', 'ip', 'link', 'set', 'lo', 'up'])
    helper = Path(__file__).with_name('release_native_executor_smtp_fixture.py')
    process = subprocess.Popen(['nsenter', '-t', str(pid), '-n', sys.executable, str(helper)],
                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    try:
        if line(process) != {'ready': True}:
            raise ValueError('fixture smtp not ready')
        address = server_address(system, layout)
        accepted = request(address, 'POST', BASE+'signup', 202, {'email': EMAIL})
        if accepted.get('accepted') is not True:
            raise ValueError('fixture signup not accepted')
        message = email.message_from_bytes(base64.b64decode(line(process)['message'], validate=True),
                                           policy=email.policy.default)
        matched = re.search(r'https://theatre\.example\.test/signup#[^\s]+', message.get_content())
        if matched is None:
            raise ValueError('fixture signup proof missing')
        parts = urllib.parse.parse_qs(urllib.parse.urlsplit(matched[0]).fragment, strict_parsing=True)
        command = {'operation_id': parts['operation'][0], 'proof_id': parts['proof'][0],
                   'proof_secret': parts['secret'][0], 'password': PASSWORD, 'explicit_confirmation': True}
        if request(address, 'POST', BASE+'signup/complete', 200, command) != {'status': 'completed'}:
            raise ValueError('fixture signup completion mismatch')
        return login(system, layout)
    finally:
        if process.poll() is None:
            process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)


def login(system, layout):
    return login_session(system, layout)['actor_id']


def login_session(system, layout):
    value = request(server_address(system, layout), 'POST', BASE+'login', 200,
                    {'email': EMAIL, 'password': PASSWORD})
    actor = value.get('actor_id')
    if not isinstance(actor, str) or not actor or not isinstance(value.get('access_token'), str):
        raise ValueError('fixture actor missing')
    return value


def verify_session(system, layout, session):
    value = request(server_address(system, layout), 'GET', BASE+'session', 200,
                    headers={'Authorization': 'Bearer '+session['access_token']})
    if value.get('actor_id') != session['actor_id']:
        raise ValueError('fixture accepted session changed')
