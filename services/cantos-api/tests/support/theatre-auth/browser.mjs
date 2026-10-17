// Private proof-page driver, not a Kanidm/OIDC or full-journey acceptance runner.
import { spawn } from 'node:child_process';
import { constants } from 'node:fs';
import { access, mkdtemp, rm } from 'node:fs/promises';
import http from 'node:http';
import { tmpdir } from 'node:os';
import { isAbsolute, join } from 'node:path';
import { localOrigin } from './recipient.mjs';

const failure = () => new Error('private_browser_step_failed');

class Pipe {
  #child;
  #buffer = Buffer.alloc(0);
  #next = 0;
  #pending = new Map();
  onEvent = () => {};

  constructor(child) {
    this.#child = child;
    child.stdio[4].on('data', chunk => {
      this.#buffer = Buffer.concat([this.#buffer, chunk]);
      if (this.#buffer.length > 4_194_304) { child.kill(); return; }
      let end;
      while ((end = this.#buffer.indexOf(0)) !== -1) {
        let message;
        try { message = JSON.parse(this.#buffer.subarray(0, end)); } catch { child.kill(); return; }
        this.#buffer = this.#buffer.subarray(end + 1);
        if (message.id) {
          const pending = this.#pending.get(message.id);
          if (pending) {
            clearTimeout(pending.timer);
            this.#pending.delete(message.id);
            if (message.error) pending.reject(failure()); else pending.resolve(message.result);
          }
        } else this.onEvent(message);
      }
    });
    child.once('exit', () => {
      for (const pending of this.#pending.values()) { clearTimeout(pending.timer); pending.reject(failure()); }
      this.#pending.clear();
    });
    child.once('error', () => {
      for (const pending of this.#pending.values()) { clearTimeout(pending.timer); pending.reject(failure()); }
      this.#pending.clear();
    });
  }

  call(method, params = {}, sessionId) {
    const id = ++this.#next;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { this.#pending.delete(id); reject(failure()); }, 5000);
      this.#pending.set(id, { resolve, reject, timer });
      this.#child.stdio[3].write(JSON.stringify({ id, method, params, sessionId }) + '\0', error => {
        if (error) { clearTimeout(timer); this.#pending.delete(id); reject(failure()); }
      });
    });
  }
}

/** Fresh Chrome profile + separate incognito contexts; all non-allowlisted requests refused. */
export class ProofBrowser {
  #child;
  #pipe;
  #directory;
  #denyProxy;
  #origin;
  #guardFailure = false;

  static async start(executable, webOrigin) {
    const origin = localOrigin(webOrigin);
    try {
      if (!isAbsolute(executable)) throw failure();
      await access(executable, constants.X_OK);
    } catch { throw failure(); }
    const directory = await mkdtemp(join(tmpdir(), 'cantos-auth-browser-'));
    // Every browser HTTP request, including workers/background requests, crosses this proxy.
    // It forwards only the exact owned Web origin; CONNECT and websocket upgrades are refused.
    const denyProxy = http.createServer((request, response) => {
      let target;
      try { target = new URL(request.url); } catch { /* refused */ }
      if (!target || target.origin !== origin || target.username || target.password) {
        response.writeHead(403); response.end(); return;
      }
      const headers = { ...request.headers, host: target.host };
      delete headers['proxy-authorization'];
      delete headers['proxy-connection'];
      const upstream = http.request(target, { method: request.method, headers, agent: false }, incoming => {
        response.writeHead(incoming.statusCode, incoming.headers);
        incoming.pipe(response);
      });
      upstream.setTimeout(2000, () => upstream.destroy());
      upstream.on('error', () => { if (!response.headersSent) response.writeHead(502); response.end(); });
      request.on('aborted', () => upstream.destroy());
      request.pipe(upstream);
    });
    denyProxy.on('connect', (_request, socket) => socket.end('HTTP/1.1 403 Forbidden\r\n\r\n'));
    denyProxy.on('upgrade', (_request, socket) => socket.end('HTTP/1.1 403 Forbidden\r\n\r\n'));
    await new Promise(resolve => denyProxy.listen(0, '127.0.0.1', resolve));
    const child = spawn(executable, [
      '--headless=new', '--remote-debugging-pipe', `--user-data-dir=${directory}`,
      '--no-first-run', '--no-default-browser-check', '--disable-background-networking',
      '--disable-component-update', '--disable-extensions', '--disable-sync', '--disable-quic',
      '--force-webrtc-ip-handling-policy=disable_non_proxied_udp',
      `--proxy-server=http://127.0.0.1:${denyProxy.address().port}`,
      '--proxy-bypass-list=<-loopback>',
      '--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1', 'about:blank',
    ], { env: { PATH: process.env.PATH ?? '', HOME: directory, XDG_CONFIG_HOME: directory },
      stdio: ['ignore', 'ignore', 'ignore', 'pipe', 'pipe'] });
    const browser = new ProofBrowser(child, directory, denyProxy, origin);
    try { await browser.#pipe.call('Browser.getVersion'); } catch { await browser.close(); throw failure(); }
    return browser;
  }

  constructor(child, directory, denyProxy, origin) {
    this.#child = child;
    this.#pipe = new Pipe(child);
    this.#directory = directory;
    this.#denyProxy = denyProxy;
    this.#origin = origin;
    this.#pipe.onEvent = message => {
      if (message.method !== 'Fetch.requestPaused') return;
      let allowed = false;
      try { allowed = new URL(message.params.request.url).origin === this.#origin; } catch { /* refused */ }
      const method = allowed ? 'Fetch.continueRequest' : 'Fetch.failRequest';
      const params = { requestId: message.params.requestId, ...(!allowed && { errorReason: 'BlockedByClient' }) };
      this.#pipe.call(method, params, message.sessionId).catch(() => { this.#guardFailure = true; });
    };
  }

  async tab() {
    const { browserContextId } = await this.#pipe.call('Target.createBrowserContext');
    const { targetId } = await this.#pipe.call('Target.createTarget', { url: 'about:blank', browserContextId });
    const { sessionId } = await this.#pipe.call('Target.attachToTarget', { targetId, flatten: true });
    await this.#pipe.call('Page.enable', {}, sessionId);
    await this.#pipe.call('Network.enable', {}, sessionId);
    await this.#pipe.call('Network.setBypassServiceWorker', { bypass: true }, sessionId);
    await this.#pipe.call('Fetch.enable', { patterns: [{ urlPattern: '*' }] }, sessionId);
    const call = (method, params) => {
      if (this.#guardFailure) throw failure();
      return this.#pipe.call(method, params, sessionId);
    };
    return new ProofTab(call, this.#origin);
  }

  async close() {
    // Only our directly spawned process/profile/proxy are ever cleaned up.
    if (this.#child.pid && this.#child.exitCode === null && this.#child.signalCode === null) {
      const exited = new Promise(resolve => this.#child.once('exit', resolve));
      this.#child.kill('SIGTERM');
      const timer = setTimeout(() => this.#child.kill('SIGKILL'), 2000);
      await exited;
      clearTimeout(timer);
    }
    await new Promise(resolve => this.#denyProxy.close(resolve));
    await rm(this.#directory, { recursive: true, force: true });
  }
}

class ProofTab {
  #call;
  #origin;
  constructor(call, origin) { this.#call = call; this.#origin = origin; }

  async #evaluate(expression) {
    const response = await this.#call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (response.exceptionDetails) throw failure();
    return response.result.value;
  }

  async navigate(url) {
    const target = new URL(url);
    if (target.origin !== this.#origin || target.username || target.password) throw failure();
    const result = await this.#call('Page.navigate', { url });
    if (result.errorText) throw failure();
  }

  async openAction(action) { await action.useUrl(url => this.navigate(url)); }
  async reload() { await this.#call('Page.reload', { ignoreCache: true }); }

  async waitFor(selector) {
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      if (await this.#evaluate(`Boolean(document.querySelector(${JSON.stringify(selector)}))`)) return;
      await new Promise(resolve => setTimeout(resolve, 25));
    }
    throw failure();
  }

  async click(selector) {
    await this.waitFor(selector);
    const point = await this.#evaluate(`(() => {
      const node = document.querySelector(${JSON.stringify(selector)});
      if (node.disabled) return null;
      node.scrollIntoView(); const rect = node.getBoundingClientRect();
      return rect.width && rect.height ? { x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 } : null;
    })()`);
    if (!point) throw failure();
    await this.#call('Input.dispatchMouseEvent', { type: 'mousePressed', ...point, button: 'left', clickCount: 1 });
    await this.#call('Input.dispatchMouseEvent', { type: 'mouseReleased', ...point, button: 'left', clickCount: 1 });
  }

  async submitEmail(email) {
    if (!/^[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-z0-9-]+(?:\.[a-z0-9-]+)*\.test$/.test(email)) throw failure();
    await this.click('#theatre-signup-email');
    await this.#call('Input.insertText', { text: email });
    await this.click('#theatre-signup-submit');
  }

  async assertScrubbed() {
    const safe = await this.#evaluate(`location.origin === ${JSON.stringify(this.#origin)}
      && location.pathname === '/' && location.search === ''
      && location.hash === '#register?from=home' && history.state === null`);
    if (!safe) throw failure();
  }

  async assertAnonymous() {
    const status = await this.#evaluate(`fetch('/api/v1/theatre/session', {
      credentials: 'include', cache: 'no-store', redirect: 'error'
    }).then(response => response.status)`);
    if (status !== 401) throw failure();
  }
}
