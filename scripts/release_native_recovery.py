"""Offline current-authority reconciliation primitive (ADR0057), never implicit activation.

The adapter supplies only stopped-owner CLI calls, verified cold-directory copy and immutable
image identity. No SQL, password, credential, HTTP session, provider or host-service command is
accepted here. The caller must separately qualify its adapter and production executor.
"""
import hashlib
import json
import os
from pathlib import Path
import re

import release_artifact as contract
import release_native as native
import release_oci as oci

VERSION = 1
STAGES = ('CAPTURE', 'STAGE', 'IDENTITY', 'SESSIONS', 'COMPLETE')


def operation(value):
    contract.require(isinstance(value, str) and re.fullmatch('[0-9a-f]{32}', value) and
                     value != '0'*32, 'canonical nonzero recovery operation required')
    return value


def barrier(path, op, ready=False):
    """Root/operator publishes this outside both restore roots; service mounts are read-only."""
    native.atomic(Path(path), ('CANTOS_RECOVERY_BARRIER_V1\n'+
                  ('ready' if ready else 'blocked')+' '+operation(op)+'\n').encode(), mode=0o644)


def proof(value, service):
    contract.require(isinstance(value, dict) and value.get('owner') == service and
                     value.get('exposure') == 'NOT_AUTHORIZED', 'owner recovery proof required')
    allowed = {'owner','schema','instance','revision','exposure','backup'} | (
        {'epoch'} if service == 'cantos-identity' else {'all_prior_sessions_retired'})
    contract.require(set(value) <= allowed, 'unexpected owner recovery proof fields')
    operation(value.get('instance'))
    contract.require(type(value.get('revision')) is int and value['revision'] > 0 and
                     type(value.get('schema')) is int and value['schema'] > 0,
                     'owner freshness marker required')
    if service == 'cantos-identity':
        operation(value.get('epoch'))
    else:
        contract.require(type(value.get('all_prior_sessions_retired')) is bool, 'current session state required')
    return value


def marker(value):
    return {key: value[key] for key in ('owner', 'schema', 'instance', 'revision')}


def same(current, expected):
    contract.require(marker(current) == marker(expected), 'current security authority changed')
    if current['owner'] == 'cantos-identity':
        contract.require(current['epoch'] == expected['epoch'], 'Identity source epoch changed')


def save(path, value):
    native.atomic(path, contract.canonical(value))


def validate_intent(value, state):
    oci.fields(value, 'schema_version operation request_sha256 stage sources exports staged effects', 'invalid recovery intent fields')
    contract.require(type(value['schema_version']) is int and value['schema_version']==VERSION and value['stage'] in STAGES,
                     'invalid recovery intent version/stage')
    services = set(oci.SERVICES)
    for name in ('sources','exports','staged','effects'):
        contract.require(type(value[name]) is dict and set(value[name]) <= services, 'invalid recovery intent owner map')
    for service, source in value['sources'].items(): proof(source, service)
    for service, effect in value['effects'].items(): proof(effect, service)
    for service, path in value['staged'].items():
        contract.require(path == str(state/'staged'/service), 'foreign staged recovery root')
    index=STAGES.index(value['stage'])
    if index>=1:
        contract.require(set(value['sources'])==services and set(value['exports'])==services, 'incomplete current security cut')
    if index>=2: contract.require(set(value['staged'])==services, 'incomplete staged pair')
    if index>=3:
        contract.require('cantos-identity' in value['effects'] and
            value['effects']['cantos-identity']['epoch'] != value['sources']['cantos-identity']['epoch'], 'unrotated recovery intent')
    if index>=4:
        contract.require(set(value['effects'])==services and value['effects']['cantos-server']['all_prior_sessions_retired'] is True,
                         'unretired recovery intent')


def reconcile(state, request, ports):
    """Explicit, fail-closed current-cut recovery; caller never supplies a historical merge.

    Ports: observe(service, root), export(service, source, destination), copy(source, target),
    rotate(root, operation, expected_epoch), retire(root), reconcile(service, root),
    directory_record(root). Each is finite, owner-scoped and separately testable.
    Sources are complete current stopped-owner roots; the external fence must not be copied.
    """
    state = Path(state)
    native.private_path(state, directory=True)
    contract.require(not state.stat().st_mode & 0o077, 'private recovery state required')
    oci.fields(request, 'schema_version operation barrier source_sha configuration_sha256 images sources',
               'exact recovery request required')
    contract.require(type(request['schema_version']) is int and request['schema_version'] == VERSION, 'unsupported recovery request')
    op = operation(request['operation'])
    contract.sha(request['source_sha'])
    contract.digest(request['configuration_sha256'])
    contract.require(set(request['images']) == set(oci.SERVICES) and set(request['sources']) == set(oci.SERVICES),
                     'complete owner pair required')
    for service in oci.SERVICES:
        oci.image_ref(request['images'][service], service)
    contract.require(ports.release_identity() == {key:request[key] for key in ('source_sha','configuration_sha256','images')},
                     'recovery adapter release pins changed')
    exposure = Path(request['barrier'])
    contract.require(exposure.is_absolute() and exposure.parent.resolve() == exposure.parent and
                     exposure.parent.is_dir(), 'canonical exposure directory required')
    exposure_meta = exposure.parent.stat()
    contract.require(exposure_meta.st_uid == os.geteuid() and not exposure_meta.st_mode & 0o022,
                     'trusted exposure directory required')
    source_paths = {service: Path(request['sources'][service]) for service in oci.SERVICES}
    contract.require(len(set(source_paths.values())) == 2 and all(path.is_absolute() and path.resolve() == path
                     and path.is_dir() and not exposure.is_relative_to(path) and not state.is_relative_to(path)
                     for path in source_paths.values()), 'disjoint current source roots required')
    raw = contract.canonical(request)
    digest = hashlib.sha256(raw).hexdigest()
    with native.locked(state):
        intent_path = state/'intent.json'
        if intent_path.exists():
            intent = native.pinned(intent_path, None)
            validate_intent(intent,state)
            contract.require(intent.get('request_sha256') == digest and intent.get('operation') == op and
                             intent.get('stage') in STAGES, 'another or invalid recovery intent')
        else:
            # Close before any source export, copy or security transition. A crash before
            # intent publication leaves a closed barrier, never a reopened application.
            barrier(exposure, op)
            intent = {'schema_version':VERSION, 'operation':op, 'request_sha256':digest,
                      'stage':'CAPTURE', 'sources':{}, 'exports':{}, 'staged':{}, 'effects':{}}
            save(intent_path, intent)
        # Replaying COMPLETE does not reopen: only the caller's independent qualification
        # and explicit expose() may publish READY. Every attempt starts/remains closed.
        barrier(exposure, op)
        if intent['stage'] == 'CAPTURE':
            for service in oci.SERVICES:
                source = source_paths[service]
                observed = proof(ports.observe(service, source), service)
                if service not in intent['sources']:
                    intent['sources'][service] = observed
                    save(intent_path, intent)
                same(observed, intent['sources'][service])
                destination = state/'authority'/service
                if service not in intent['exports']:
                    if destination.exists():
                        # Unknown export publication: a complete integrity-checked snapshot
                        # with the exact current owner marker may be reconciled; never overwrite.
                        same(proof(ports.observe(service, destination), service), observed)
                    else:
                        ports.export(service, source, destination)
                    same(proof(ports.observe(service, destination), service), observed)
                    same(proof(ports.observe(service, source), service), observed)
                    intent['exports'][service] = ports.directory_record(destination)
                    save(intent_path, intent)
            intent['stage'] = 'STAGE'
            save(intent_path, intent)
        if intent['stage'] == 'STAGE':
            # Revalidate BOTH sources after capture, before either staged mutation.
            for service in oci.SERVICES:
                same(proof(ports.observe(service, source_paths[service]), service), intent['sources'][service])
                original = state/'authority'/service
                contract.require(ports.directory_record(original) == intent['exports'][service], 'security cut bytes changed')
                destination = state/'staged'/service
                if not destination.exists():
                    ports.copy(original, destination)
                    contract.require(ports.directory_record(destination) == intent['exports'][service], 'staged source differs')
                # Unknown copy acknowledgement is reconciled by the real owner below.
                # Opening may checkpoint physical journal bytes; the exact current marker
                # still binds the unique committed logical state.
                same(proof(ports.observe(service, destination), service), intent['sources'][service])
                intent['staged'][service] = str(destination)
            intent['stage'] = 'IDENTITY'
            save(intent_path, intent)
        if intent['stage'] == 'IDENTITY':
            root = Path(intent['staged']['cantos-identity'])
            # This only acknowledges the exact next DB marker. A pre-commit/old copy
            # cannot be reconciled and remains closed. Rotate itself has a bound receipt.
            ports.reconcile('cantos-identity', root)
            result = proof(ports.rotate(root, op, intent['sources']['cantos-identity']['epoch']), 'cantos-identity')
            contract.require(result['epoch'] != intent['sources']['cantos-identity']['epoch'], 'restore epoch not rotated')
            intent['effects']['cantos-identity'] = result
            intent['stage'] = 'SESSIONS'
            save(intent_path, intent)
        if intent['stage'] == 'SESSIONS':
            root = Path(intent['staged']['cantos-server'])
            ports.reconcile('cantos-server', root)
            result = proof(ports.retire(root), 'cantos-server')
            contract.require(result.get('all_prior_sessions_retired') is True, 'session retirement proof required')
            intent['effects']['cantos-server'] = result
            intent['stage'] = 'COMPLETE'
            save(intent_path, intent)
        for service in oci.SERVICES:
            current = proof(ports.observe(service, Path(intent['staged'][service])), service)
            same(current, intent['effects'][service])
            if service=='cantos-server':
                contract.require(current['all_prior_sessions_retired'] is True, 'recovery sessions became active')
            else:
                contract.require(current['epoch'] != intent['sources'][service]['epoch'], 'recovery epoch is stale')
        # Deliberately no READY effect. This is state reconciliation, not HTTP/exact-image,
        # restricted-policy, source-admission, host-executor or production acceptance.
        return {'operation':op, 'stage':'COMPLETE', 'request_sha256':digest,
                'staged':intent['staged'], 'owners':intent['effects'],
                'exposure':'BLOCKED', 'production_activation':'NOT_RUN'}


def expose(state, request, ports):
    """Explicit private rehearsal exposure, only after exact current reconciliation.

    This grants no production activation or restricted-delivery permission. Caller must own
    isolated ingress, run the actual HTTP probes, and re-close on any failure/unknown result.
    """
    result = reconcile(state, request, ports)
    barrier(Path(request['barrier']), result['operation'], ready=True)
    return result
