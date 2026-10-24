'use strict';
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { test } = require('node:test');
const fixture = require('./launcher.cjs');

const request = () => ({ method: 'POST', url: '/emails', headers: {
  host: '127.0.0.1:8123', authorization: fixture.marker, 'content-type': 'application/json',
  'content-length': String(Buffer.byteLength(fixture.payload)),
} });

test('only a bounded synthetic API request enters the patched local server', () => {
  assert.equal(fixture.requestAllowed(request(), 8123), true);
  for (const mutate of [r => { r.headers.host = 'localhost:8123'; },
    r => { r.headers.origin = 'https://foreign.invalid'; },
    r => { r.headers['next-action'] = 'anything'; }, r => { r.headers.rsc = '1'; },
    r => { r.headers['transfer-encoding'] = 'chunked'; },
    r => { r.url = '/dashboard'; }, r => { r.url = '/emails?anything'; },
    r => { r.headers['content-length'] = '99999'; },
    r => { r.headers.authorization = 'Bearer unrelated'; }]) {
    const changed = request(); mutate(changed);
    assert.equal(fixture.requestAllowed(changed, 8123), false);
  }
});

test('a changed body, attachment, recipient or proof is refused before provider listeners', () => {
  for (const text of [fixture.payload, fixture.payload.replace('recipient@cantos.invalid', 'other@cantos.invalid'),
    fixture.payload.replace('Nonfunctional fixture only.', 'https://example.invalid/#secret=not-a-real-proof'),
    fixture.payload + ' ']) {
    const stream = new EventEmitter(); stream.method = 'POST';
    let destroyed = false; let observed = '';
    stream.destroy = () => { destroyed = true; };
    fixture.guardBody(stream);
    stream.on('data', bytes => { observed += bytes; });
    stream.emit('data', Buffer.from(text)); stream.emit('end');
    assert.equal(destroyed, text !== fixture.payload);
    assert.equal(observed, text === fixture.payload ? text : '');
  }
});

test('TCP TLS UDP and process-spawn negative controls reject without effects', () => {
  fixture.installOutboundGuard();
  assert.throws(() => require('node:net').connect(443, '192.0.2.1'), /effect refused/);
  assert.throws(() => require('node:tls').connect(443, '192.0.2.1'), /effect refused/);
  assert.throws(() => require('node:dgram').createSocket('udp4'), /effect refused/);
  assert.throws(() => require('node:child_process').spawn('unused'), /effect refused/);
});
