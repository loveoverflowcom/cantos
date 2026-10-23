// Implementation primitive of cargo xtask run, never a Firebase CLI/login/download entrypoint.
// Reuses the reviewed #242 internal-module contract, pinned to firebase-tools 14.2.2.
'use strict';
const path = require('node:path');
const net = require('node:net');
const tls = require('node:tls');
const output = process.stdout.write.bind(process.stdout);
const fail = (error) => {
  const code = ['EPERM', 'EACCES', 'EADDRINUSE', 'MODULE_NOT_FOUND'].includes(error?.code)
    ? error.code : 'STARTUP';
  output(JSON.stringify({ failure: code }) + '\n');
  process.exit(1);
};
process.once('uncaughtException', fail);
// Firebase's logger prints verification links. Nothing from it may enter an agent/run log.
process.stdout.write = () => true;
process.stderr.write = () => true;
const deny = () => { throw new Error('outbound sockets are forbidden'); };
net.Socket.prototype.connect = deny;
tls.connect = deny;
// Fail the launch if either negative control stops rejecting before a socket is opened.
for (const connect of [() => new net.Socket().connect(443, '192.0.2.1'),
  () => tls.connect(443, '192.0.2.1')]) {
  let rejected = false;
  try { connect(); } catch { rejected = true; }
  if (!rejected) process.exit(2);
}
const [modulePath, project] = process.argv.slice(2);
if (!path.isAbsolute(modulePath || '') || !/^demo-[a-z0-9-]{1,24}$/.test(project || '')) {
  process.exit(2);
}
const pkg = require(path.resolve(path.dirname(modulePath), '../../../package.json'));
if (pkg.name !== 'firebase-tools' || pkg.version !== '14.2.2') process.exit(2);
(async () => {
  const { createApp } = require(modulePath);
  const app = await createApp(project, 2);
  // Refuse wildcard/remote listeners even if a dependency tries to open another listener.
  const listen = net.Server.prototype.listen;
  net.Server.prototype.listen = function (port, host, ...rest) {
    if (port !== 0 || host !== '127.0.0.1') throw new Error('loopback listener required');
    return listen.call(this, port, host, ...rest);
  };
  const server = app.listen(0, '127.0.0.1');
  server.once('error', fail);
  server.once('listening', () => output(JSON.stringify({
    host: '127.0.0.1', port: server.address().port, project, version: pkg.version,
    outboundGuard: true,
  }) + '\n'));
})().catch(fail);
