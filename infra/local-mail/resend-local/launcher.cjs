// Implementation primitive of cargo xtask run, never an installer or a proof provider.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const net = require('node:net');
const tls = require('node:tls');
const http = require('node:http');
const dgram = require('node:dgram');
const childProcess = require('node:child_process');
const { createRequire } = require('node:module');
const { pathToFileURL } = require('node:url');
const payload = fs.readFileSync(path.join(__dirname, 'synthetic-email.json'), 'utf8').trim();
const marker = 'Bearer re_cantos_synthetic_fixture'; // Public fixture marker, no authority.

function requestAllowed(request, port) {
  const headers = request.headers;
  const methodPath = (request.method === 'POST' && request.url === '/emails') ||
    (request.method === 'GET' && /^\/emails\/[0-9a-f-]{36}$/.test(request.url));
  return methodPath && headers.host === `127.0.0.1:${port}` &&
    headers.authorization === marker && !headers.origin && !headers['next-action'] &&
    !headers.rsc && !headers.expect && !headers['transfer-encoding'] &&
    (request.method === 'GET' ? !headers['content-length'] :
      headers['content-type'] === 'application/json' && headers['content-length'] === String(Buffer.byteLength(payload)));
}

function guardBody(request) {
  if (request.method !== 'POST') return;
  const expected = Buffer.from(payload);
  const emit = request.emit;
  let received = 0;
  request.emit = function (event, ...args) {
    if (event === 'data') {
      const chunk = args[0];
      if (!Buffer.isBuffer(chunk) || received + chunk.length > expected.length ||
          !chunk.equals(expected.subarray(received, received + chunk.length))) {
        this.destroy();
        return false;
      }
      received += chunk.length;
    }
    if (event === 'end' && received !== expected.length) {
      this.destroy();
      return false;
    }
    return emit.call(this, event, ...args);
  };
}

function deny() { throw new Error('local fixture effect refused'); }

function installOutboundGuard() {
  net.Socket.prototype.connect = deny;
  tls.connect = deny;
  dgram.createSocket = deny;
  for (const name of ['spawn', 'spawnSync', 'exec', 'execSync', 'execFile', 'execFileSync', 'fork']) {
    childProcess[name] = deny;
  }
  // Execute negative controls before loading any third-party code. No socket is opened.
  for (const connect of [() => new net.Socket().connect(443, '192.0.2.1'),
    () => tls.connect(443, '192.0.2.1'), () => dgram.createSocket('udp4'),
    () => childProcess.spawn('unused')]) {
    let rejected = false;
    try { connect(); } catch { rejected = true; }
    if (!rejected) throw new Error('outbound negative control failed');
  }
}

function inspectRuntime(root) {
  const metadata = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  const server = fs.realpathSync(path.join(root, 'dist/app/server.js'));
  // server.js creates its own require from import.meta.url. Resolve from that same real
  // filename, not the launcher, cwd, or an assumed hoisted npm layout. Resolution reads
  // metadata only: neither the Next entry nor start-server may execute before validation.
  const fromServer = createRequire(pathToFileURL(server));
  const nextPackage = fs.realpathSync(fromServer.resolve('next/package.json'));
  const nextEntry = fs.realpathSync(fromServer.resolve('next'));
  const nextStart = fs.realpathSync(fromServer.resolve('next/dist/server/lib/start-server'));
  const expectedRoot = fs.realpathSync(path.join(root, '../next'));
  const expectedPackage = path.join(expectedRoot, 'package.json');
  const fromExpected = createRequire(pathToFileURL(expectedPackage));
  if (nextPackage !== expectedPackage ||
      nextEntry !== fs.realpathSync(fromExpected.resolve('.')) ||
      nextStart !== fs.realpathSync(path.join(expectedRoot, 'dist/server/lib/start-server.js')) ||
      !nextEntry.startsWith(expectedRoot + path.sep) || !nextStart.startsWith(expectedRoot + path.sep)) {
    throw Object.assign(new Error('shadowed Resend local Next runtime'), { code: 'RESEND_RUNTIME_SHADOWED' });
  }
  const next = JSON.parse(fs.readFileSync(nextPackage, 'utf8'));
  if (metadata.name !== 'resend-local' || metadata.version !== '0.0.7' ||
      next.name !== 'next' || next.version !== '15.5.28') {
    throw Object.assign(new Error('unreviewed runtime'), { code: 'RESEND_RUNTIME_UNREVIEWED' });
  }
  return { server, nextPackage, nextEntry, nextStart, version: next.version };
}

async function start() {
  const [root, database] = process.argv.slice(2);
  if (!path.isAbsolute(root || '') || !path.isAbsolute(database || '') ||
      Number(process.versions.node.split('.')[0]) < 22) throw new Error('invalid fixture inputs');
  const runtime = inspectRuntime(root);
  const output = process.stdout.write.bind(process.stdout);
  // Nothing from the provider, including request bodies or API error details, becomes evidence.
  process.stdout.write = () => true;
  process.stderr.write = () => true;
  const fail = () => { output('{"failure":"STARTUP"}\n'); process.exit(1); };
  process.once('uncaughtException', fail);
  process.once('unhandledRejection', fail);
  installOutboundGuard();
  process.env.HOSTNAME = '127.0.0.1';
  process.env.PORT = '3000'; // The listen guard below always substitutes an ephemeral port.
  process.env.DATABASE_URL = `file:${database}`;
  process.env.NEXT_TELEMETRY_DISABLED = '1';
  const listen = net.Server.prototype.listen;
  let listener;
  net.Server.prototype.listen = function (port, host, ...rest) {
    if (listener || port !== 3000 || host !== '127.0.0.1') throw new Error('loopback listener required');
    listener = this;
    this.once('listening', () => output(JSON.stringify({
      host: '127.0.0.1', port: this.address().port, version: '0.0.7',
      outboundGuard: true, syntheticOnly: true,
    }) + '\n'));
    return listen.call(this, 0, '127.0.0.1', ...rest);
  };
  const emit = http.Server.prototype.emit;
  http.Server.prototype.emit = function (event, ...args) {
    if (['upgrade', 'connect', 'checkContinue', 'checkExpectation'].includes(event)) {
      args[0].destroy();
      return true;
    }
    if (event === 'request') {
      const [request, response] = args;
      if (!requestAllowed(request, this.address().port)) {
        response.writeHead(403, { Connection: 'close', 'Content-Length': '0' });
        response.end();
        return true;
      }
      guardBody(request);
    }
    return emit.call(this, event, ...args);
  };
  // Bypass upstream starter.js, which inherits environment and hardcodes a shared SQLite path.
  // server.js itself is byte-checked against the npm archive before this process is started.
  await import(pathToFileURL(runtime.server).href);
}

module.exports = { requestAllowed, guardBody, installOutboundGuard, inspectRuntime, payload, marker };
if (require.main === module) {
  if (process.argv[2] === '--inspect-runtime') {
    try { process.stdout.write(JSON.stringify(inspectRuntime(process.argv[3])) + '\n'); }
    catch (error) {
      const failure = ['RESEND_RUNTIME_SHADOWED', 'RESEND_RUNTIME_UNREVIEWED'].includes(error.code)
        ? error.code : 'RESEND_RUNTIME_INSPECTION';
      process.stdout.write(JSON.stringify({ failure }) + '\n');
      process.exitCode = 1;
    }
  } else start().catch(() => { process.exitCode = 1; });
}
