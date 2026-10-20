import assert from 'node:assert/strict';
import http from 'node:http';
import { inspect } from 'node:util';
import { test } from 'node:test';
import { Recipient, localOrigin } from './recipient.mjs';

const operation = '00000000-0000-4000-8000-000000000001';
const webOrigin = 'http://127.0.0.1:8082';
const continueUrl = `${webOrigin}/?firebaseEnrollment=${operation}`;
const email = 'recipient@cantos.test';

async function fixture(t) {
  const state = { codes: [], paths: [], status: 200, contentType: 'application/json', body: null };
  const server = http.createServer((request, response) => {
    state.paths.push(request.url);
    response.writeHead(state.status, { 'Content-Type': state.contentType });
    response.end(state.body ?? JSON.stringify({ oobCodes: state.codes }));
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  const emulatorOrigin = `http://127.0.0.1:${server.address().port}`;
  const options = { emulatorOrigin, project: 'demo-cantos-recipient', email, webOrigin, continueUrl };
  const entry = (code, overrides = {}) => {
    const oobLink = new URL('/emulator/action', emulatorOrigin);
    oobLink.searchParams.set('mode', 'verifyEmail');
    oobLink.searchParams.set('oobCode', code);
    oobLink.searchParams.set('continueUrl', continueUrl);
    return { email, requestType: 'VERIFY_EMAIL', oobCode: code, oobLink: oobLink.href, ...overrides };
  };
  return { state, options, entry, recipient: new Recipient(options) };
}

function error(code) { return value => value?.code === code && value.message === code; }

test('inventory delta selects exact run recipient, purpose and operation without consuming default link', async t => {
  const { state, entry, recipient } = await fixture(t);
  state.codes = [entry('old-proof')];
  const previous = await recipient.snapshot();
  const wrongOperation = entry('other-operation');
  const link = new URL(wrongOperation.oobLink);
  link.searchParams.set('continueUrl', `${webOrigin}/?firebaseEnrollment=00000000-0000-4000-8000-000000000002`);
  wrongOperation.oobLink = link.href;
  state.codes.push(entry('wrong-recipient', { email: 'other@cantos.test' }),
    entry('wrong-purpose', { requestType: 'PASSWORD_RESET' }), wrongOperation, entry('new-proof'));
  const action = await recipient.wait(previous);
  action.useUrl(value => {
    const parsed = new URL(value);
    assert.equal(parsed.origin, webOrigin);
    assert.equal(parsed.pathname, '/');
    assert.ok(parsed.searchParams.get('oobCode') === 'new-proof', 'the new matching proof is preserved');
    assert.equal(parsed.searchParams.get('continueUrl'), continueUrl);
    assert.deepEqual([...parsed.searchParams.keys()], ['mode', 'oobCode', 'continueUrl']);
  });
  assert.ok(state.paths.every(path => path === '/emulator/v1/projects/demo-cantos-recipient/oobCodes'));
  assert.ok(!inspect(action).includes('new-proof'));
  assert.ok(!JSON.stringify(action).includes('new-proof'));
  assert.ok(!inspect(recipient).includes(email));
});

test('ambiguous newly emitted siblings fail rather than selecting first or last', async t => {
  const { state, entry, recipient } = await fixture(t);
  const previous = await recipient.snapshot();
  state.codes.push(entry('sibling-one'), entry('sibling-two'));
  await assert.rejects(recipient.wait(previous), error('ambiguous_recipient_action'));
});

test('bounded wait observes a later emitted action and cancellation never waits indefinitely', async t => {
  const { state, entry, recipient } = await fixture(t);
  const previous = await recipient.snapshot();
  const timer = setTimeout(() => state.codes.push(entry('late-proof')), 30);
  t.after(() => clearTimeout(timer));
  assert.ok(await recipient.wait(previous, { timeoutMs: 1000 }));
  state.codes = [];
  await assert.rejects(recipient.wait(previous, { timeoutMs: 40 }), error('recipient_deadline'));
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(recipient.wait(previous, { signal: controller.signal }), error('recipient_cancelled'));
});

test('inventory protocol, redirects, overlarge responses and malformed bindings fail closed', async t => {
  const { state, entry, recipient, options } = await fixture(t);
  const previous = new Set();
  for (const changes of [
    { status: 302, body: '{}', contentType: 'application/json' },
    { status: 200, body: '{}', contentType: 'text/html' },
    { status: 200, body: '{', contentType: 'application/json' },
    { status: 200, body: '{}', contentType: 'application/json' },
  ]) {
    Object.assign(state, changes);
    await assert.rejects(recipient.wait(previous), error('inventory_protocol'));
  }
  state.body = ' '.repeat(65_537);
  await assert.rejects(recipient.wait(previous), error('inventory_too_large'));
  state.body = null;
  for (const badLink of [
    'https://identitytoolkit.googleapis.com/emulator/action',
    'http://127.0.0.1:9099/emulator/action',
    `${options.emulatorOrigin}/not-an-action`,
  ]) {
    state.codes = [entry('private-proof', { oobLink: badLink })];
    await assert.rejects(recipient.wait(previous), error('inventory_protocol'));
  }
});

test('constructor rejects cloud origins, credentials, non-demo projects and non-test recipients before I/O', () => {
  for (const value of ['https://127.0.0.1:8000', 'http://localhost:8000', 'http://127.0.0.2:8000',
    'http://127.0.0.1', 'http://user@127.0.0.1:8000', 'http://127.0.0.1:8000/path', 'https://google.com']) {
    assert.throws(() => localOrigin(value), error('invalid_local_origin'));
  }
  const options = { emulatorOrigin: 'http://127.0.0.1:9099', project: 'demo-cantos', email,
    webOrigin, continueUrl };
  for (const changes of [{ project: 'production-cantos' }, { email: 'real@example.com' }]) {
    assert.throws(() => new Recipient({ ...options, ...changes }), error('invalid_run_identity'));
  }
  for (const value of ['http://127.0.0.1:8083/?firebaseEnrollment=' + operation,
    continueUrl + '&extra=1', continueUrl + '#secret', `${webOrigin}/?firebaseEnrollment=not-an-operation`]) {
    assert.throws(() => new Recipient({ ...options, continueUrl: value }), error('invalid_continuation'));
  }
});
