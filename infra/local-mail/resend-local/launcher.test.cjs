'use strict';
const assert = require('node:assert/strict');
const { EventEmitter } = require('node:events');
const { test } = require('node:test');
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
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

function preparedInstall(shadowAt) {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'cantos-resend-resolution-'));
  const root = path.join(directory, 'node_modules/resend-local');
  const marker = path.join(directory, 'module-executed');
  function write(relative, text) {
    const file = path.join(directory, relative);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, text);
  }
  function next(relative, version) {
    write(`${relative}/package.json`, JSON.stringify({ name: 'next', version, main: 'index.js' }));
    // These inert stubs fail if inspection accidentally executes either module.
    const code = `require('node:fs').writeFileSync(${JSON.stringify(marker)}, 'unexpected execution'); throw new Error('fixture module executed');`;
    write(`${relative}/index.js`, code);
    write(`${relative}/dist/server/lib/start-server.js`, code);
  }
  write('node_modules/resend-local/package.json', JSON.stringify({ name: 'resend-local', version: '0.0.7' }));
  write('node_modules/resend-local/dist/app/server.js', 'throw new Error("server entry executed during inspection");');
  next('node_modules/next', '15.5.28');
  if (shadowAt) next(`node_modules/resend-local/${shadowAt}/node_modules/next`, '15.5.2');
  return { directory, root, marker };
}

test('patched sibling with no nearer copy resolves without executing package code', () => {
  const install = preparedInstall();
  try {
    const runtime = fixture.inspectRuntime(install.root);
    assert.equal(runtime.version, '15.5.28');
    assert.equal(runtime.nextPackage, fs.realpathSync(path.join(install.directory, 'node_modules/next/package.json')));
    assert.equal(runtime.nextStart, fs.realpathSync(path.join(install.directory, 'node_modules/next/dist/server/lib/start-server.js')));
    assert.equal(fs.existsSync(install.marker), false);
  } finally { fs.rmSync(install.directory, { recursive: true }); }
});

for (const shadowAt of ['', 'dist/app']) {
  test(`patched sibling cannot admit a stale nearer Next at ${shadowAt || 'package root'}`, () => {
    const install = preparedInstall(shadowAt || '.');
    try {
      assert.throws(() => fixture.inspectRuntime(install.root), error => error.code === 'RESEND_RUNTIME_SHADOWED');
      assert.equal(fs.existsSync(install.marker), false);
    } finally { fs.rmSync(install.directory, { recursive: true }); }
  });
}

test('Rust preflight inspection mode and launch use the same resolver without imports', () => {
  for (const shadowAt of [undefined, '.', 'dist/app']) {
    const install = preparedInstall(shadowAt);
    try {
      const inspection = require('node:child_process').spawnSync(process.execPath,
        [path.join(__dirname, 'launcher.cjs'), '--inspect-runtime', install.root],
        { encoding: 'utf8', env: {}, timeout: 3000 });
      assert.equal(inspection.status, shadowAt ? 1 : 0, inspection.stderr);
      const result = JSON.parse(inspection.stdout);
      if (shadowAt) assert.equal(result.failure, 'RESEND_RUNTIME_SHADOWED');
      else assert.equal(result.version, fixture.inspectRuntime(install.root).version);
      assert.equal(fs.existsSync(install.marker), false);
    } finally { fs.rmSync(install.directory, { recursive: true }); }
  }
});

test('TCP TLS UDP and process-spawn negative controls reject without effects', () => {
  fixture.installOutboundGuard();
  assert.throws(() => require('node:net').connect(443, '192.0.2.1'), /effect refused/);
  assert.throws(() => require('node:tls').connect(443, '192.0.2.1'), /effect refused/);
  assert.throws(() => require('node:dgram').createSocket('udp4'), /effect refused/);
  assert.throws(() => require('node:child_process').spawn('unused'), /effect refused/);
});
