// Driver contract against a tiny HTTP fixture, not the Cantos application or Kanidm.
// Explicitly select an already installed Chrome; this test never downloads a browser.
import assert from 'node:assert/strict';
import http from 'node:http';
import { test } from 'node:test';
import { ProofBrowser } from './browser.mjs';
import { Recipient } from './recipient.mjs';

async function listen(t, handler) {
  const server = http.createServer(handler);
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  t.after(() => new Promise(resolve => server.close(resolve)));
  return `http://127.0.0.1:${server.address().port}`;
}

test('owned browser drives inputs, isolates recipient context, scrubs action and blocks a nonallowlisted destination', async t => {
  const executable = process.env.CANTOS_TEST_CHROME;
  assert.ok(executable, 'set CANTOS_TEST_CHROME to an installed Chrome executable');
  let forbiddenHits = 0;
  const forbidden = await listen(t, (_request, response) => { forbiddenHits++; response.end('forbidden'); });
  const state = { submissions: [], confirmed: 0, leakedCookie: false, codes: [] };
  const origin = await listen(t, (request, response) => {
    const path = new URL(request.url, 'http://fixture.invalid').pathname;
    response.setHeader('Cache-Control', 'no-store');
    response.setHeader('Referrer-Policy', 'no-referrer');
    if (path.startsWith('/emulator/')) {
      response.setHeader('Content-Type', 'application/json');
      response.end(JSON.stringify({ oobCodes: state.codes }));
    } else if (path === '/api/v1/theatre/session') {
      state.leakedCookie ||= Boolean(request.headers.cookie);
      response.writeHead(401, { 'Content-Type': 'application/json' }); response.end('{}');
    } else if (path === '/submit') {
      let body = '';
      request.on('data', chunk => { body += chunk; });
      request.on('end', () => { state.submissions.push(body); response.end('ok'); });
    } else if (path === '/confirm') {
      state.confirmed++;
      response.end('ok');
    } else if (path === '/worker.js') {
      response.setHeader('Content-Type', 'text/javascript');
      response.end(`fetch(${JSON.stringify(forbidden + '/worker-escape')}).then(() => postMessage('done'),
        () => postMessage('done'));`);
    } else if (path === '/register') {
      response.setHeader('Content-Type', 'text/html');
      response.setHeader('Set-Cookie', 'initiator=isolated; SameSite=Strict');
      response.end(`<form><input id="theatre-signup-email"><button id="theatre-signup-submit">Submit</button></form>
        <script>document.querySelector('form').onsubmit = event => {
          event.preventDefault(); fetch('/submit', {method:'POST', body:document.querySelector('input').value})
            .then(() => document.body.insertAdjacentHTML('beforeend', '<p id="submitted"></p>'));
        };</script>`);
    } else {
      response.setHeader('Content-Type', 'text/html');
      response.end(`<script>
        const proof = new URLSearchParams(location.search).get('oobCode');
        history.replaceState(null, '', '/#register?from=home');
        const deniedFetch = fetch(${JSON.stringify(forbidden + '/escape')}).catch(() => {});
        const deniedSocket = new Promise(resolve => {
          const socket = new WebSocket(${JSON.stringify(forbidden.replace('http:', 'ws:') + '/socket-escape')});
          socket.onerror = resolve; socket.onopen = () => { socket.close(); resolve(); };
        });
        const deniedWorker = new Promise(resolve => { const worker = new Worker('/worker.js');
          worker.onmessage = () => { worker.terminate(); resolve(); }; worker.onerror = resolve; });
        Promise.all([deniedFetch, deniedSocket, deniedWorker]).then(() =>
          document.body.insertAdjacentHTML('beforeend', '<p id=guard-complete></p>'));
      </script><button id="theatre-signup-confirm" onclick="fetch('/confirm', {method:'POST',body:proof})
        .then(() => document.body.insertAdjacentHTML('beforeend', '<p id=confirmed></p>'))">Confirm</button>`);
    }
  });
  const browser = await ProofBrowser.start(executable, origin);
  t.after(() => browser.close());
  const initiator = await browser.tab();
  await initiator.navigate(origin + '/register');
  await initiator.submitEmail('recipient@cantos.test');
  await initiator.waitFor('#submitted');
  assert.deepEqual(state.submissions, ['recipient@cantos.test']);

  const continueUrl = `${origin}/?firebaseEnrollment=00000000-0000-4000-8000-000000000001`;
  const recipient = new Recipient({ emulatorOrigin: origin, webOrigin: origin,
    project: 'demo-cantos-browser', email: 'recipient@cantos.test', continueUrl });
  const previous = await recipient.snapshot();
  const link = new URL('/emulator/action', origin);
  link.searchParams.set('mode', 'verifyEmail');
  link.searchParams.set('oobCode', 'private-driver-fixture');
  link.searchParams.set('continueUrl', continueUrl);
  state.codes.push({ email: 'recipient@cantos.test', requestType: 'VERIFY_EMAIL',
    oobCode: 'private-driver-fixture', oobLink: link.href });
  const action = await recipient.wait(previous);
  const recipientTab = await browser.tab();
  await recipientTab.openAction(action);
  await recipientTab.waitFor('#theatre-signup-confirm');
  await recipientTab.waitFor('#guard-complete');
  await recipientTab.assertScrubbed();
  await recipientTab.assertAnonymous();
  assert.equal(state.confirmed, 0, 'opening the action did not click Confirm');
  assert.equal(state.leakedCookie, false, 'the recipient uses a separate cookie context');
  await recipientTab.click('#theatre-signup-confirm');
  await recipientTab.waitFor('#confirmed');
  assert.equal(state.confirmed, 1);
  await recipientTab.reload();
  await recipientTab.waitFor('#theatre-signup-confirm');
  await recipientTab.assertAnonymous();
  assert.equal(state.confirmed, 1, 'reload did not submit confirmation');
  assert.equal(forbiddenHits, 0, 'a nonallowlisted local origin received no browser traffic');
  await assert.rejects(recipientTab.navigate(forbidden), /private_browser_step_failed/);
});

test('an unavailable executable fails with a redacted bounded error and cleans up', async () => {
  await assert.rejects(ProofBrowser.start('/nonexistent/cantos-chrome', 'http://127.0.0.1:8123'),
    /private_browser_step_failed/);
});
