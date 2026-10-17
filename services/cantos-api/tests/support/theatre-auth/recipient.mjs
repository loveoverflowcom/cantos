// Private test recipient. Never serve this file or its inventory through a product route.
import http from 'node:http';
import { createHash } from 'node:crypto';
import { inspect } from 'node:util';

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
const CODE = /^[\x21-\x7e]{1,4096}$/;
const MAX_BYTES = 65_536;

export class RecipientError extends Error {
  constructor(code) {
    super(code);
    this.name = 'RecipientError';
    this.code = code;
  }
}

function reject(code) { throw new RecipientError(code); }

export function localOrigin(value) {
  let url;
  try { url = new URL(value); } catch { reject('invalid_local_origin'); }
  if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || !url.port
      || url.username || url.password || url.pathname !== '/' || url.search || url.hash) {
    reject('invalid_local_origin');
  }
  return url.origin;
}

function continuation(value, webOrigin) {
  let url;
  try { url = new URL(value); } catch { reject('invalid_continuation'); }
  const entries = [...url.searchParams];
  if (url.origin !== webOrigin || url.pathname !== '/' || url.username || url.password
      || url.hash || entries.length !== 1 || entries[0][0] !== 'firebaseEnrollment'
      || !UUID.test(entries[0][1])) reject('invalid_continuation');
  return url.href;
}

function fingerprint(value) { return createHash('sha256').update(value).digest('hex'); }

class Action {
  #url;
  constructor(url) { this.#url = url; }
  // The only escape hatch passes the capability directly to the browser, in RAM.
  useUrl(consume) { return consume(this.#url); }
  toJSON() { return '[REDACTED recipient action]'; }
  [inspect.custom]() { return 'RecipientAction([REDACTED])'; }
}

/** Bound to exactly one run-owned emulator project, recipient and operation. */
export class Recipient {
  #origin;
  #project;
  #email;
  #webOrigin;
  #continuation;

  constructor({ emulatorOrigin, project, email, webOrigin, continueUrl }) {
    this.#origin = localOrigin(emulatorOrigin);
    this.#webOrigin = localOrigin(webOrigin);
    if (!/^demo-[a-z0-9-]{1,58}$/.test(project)
        || typeof email !== 'string' || email.length > 254
        || !/^[a-z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-z0-9-]+(?:\.[a-z0-9-]+)*\.test$/.test(email)) {
      reject('invalid_run_identity');
    }
    this.#project = project;
    this.#email = email;
    this.#continuation = continuation(continueUrl, this.#webOrigin);
  }

  async #inventory(signal) {
    const path = `/emulator/v1/projects/${this.#project}/oobCodes`;
    const bytes = await new Promise((resolve, rejectPromise) => {
      let settled = false;
      const finish = (error, value) => {
        if (settled) return;
        settled = true;
        if (error) rejectPromise(new RecipientError(error)); else resolve(value);
      };
      // node:http uses this literal endpoint directly; no proxy, redirects or cloud credentials.
      const request = http.get(new URL(path, this.#origin), { agent: false, signal }, response => {
        if (response.statusCode !== 200
            || response.headers['content-type']?.split(';')[0].trim() !== 'application/json') {
          response.destroy();
          finish('inventory_protocol');
          return;
        }
        let length = 0;
        const chunks = [];
        response.on('data', chunk => {
          length += chunk.length;
          if (length > MAX_BYTES) {
            response.destroy();
            finish('inventory_too_large');
          } else chunks.push(chunk);
        });
        response.on('end', () => finish(null, Buffer.concat(chunks)));
        response.on('error', () => finish('inventory_unavailable'));
      });
      request.setTimeout(2000, () => request.destroy());
      request.on('error', () => finish(signal?.aborted ? 'recipient_cancelled' : 'inventory_unavailable'));
    });
    let inventory;
    try { inventory = JSON.parse(bytes); } catch { reject('inventory_protocol'); }
    if (!Array.isArray(inventory.oobCodes) || inventory.oobCodes.length > 256) reject('inventory_protocol');
    return inventory.oobCodes;
  }

  async snapshot(signal) {
    return new Set((await this.#inventory(signal)).map(entry => {
      if (typeof entry.oobCode !== 'string' || !CODE.test(entry.oobCode)) reject('inventory_protocol');
      return fingerprint(entry.oobCode);
    }));
  }

  #select(inventory, previous) {
    const actions = [];
    for (const entry of inventory) {
      if (entry.email !== this.#email || entry.requestType !== 'VERIFY_EMAIL') continue;
      if (typeof entry.oobCode !== 'string' || !CODE.test(entry.oobCode)
          || typeof entry.oobLink !== 'string') reject('inventory_protocol');
      if (previous.has(fingerprint(entry.oobCode))) continue;
      let issued;
      try { issued = new URL(entry.oobLink); } catch { reject('inventory_protocol'); }
      // Read the issued continuation; never visit the emulator's default consuming handler.
      if (issued.origin !== this.#origin || issued.pathname !== '/emulator/action'
          || issued.username || issued.password || issued.hash
          || issued.searchParams.getAll('mode').length !== 1
          || issued.searchParams.get('mode') !== 'verifyEmail'
          || issued.searchParams.getAll('oobCode').length !== 1
          || issued.searchParams.get('oobCode') !== entry.oobCode
          || issued.searchParams.getAll('continueUrl').length !== 1) reject('inventory_protocol');
      if (issued.searchParams.get('continueUrl') !== this.#continuation) continue;
      const action = new URL('/', this.#webOrigin);
      action.searchParams.set('mode', 'verifyEmail');
      action.searchParams.set('oobCode', entry.oobCode);
      action.searchParams.set('continueUrl', this.#continuation);
      actions.push(new Action(action.href));
    }
    if (actions.length > 1) reject('ambiguous_recipient_action');
    return actions[0];
  }

  async wait(previous, { timeoutMs = 5000, signal } = {}) {
    if (!(previous instanceof Set) || !Number.isInteger(timeoutMs) || timeoutMs < 1 || timeoutMs > 30_000) {
      reject('invalid_recipient_deadline');
    }
    const deadline = AbortSignal.timeout(timeoutMs);
    const bounded = signal ? AbortSignal.any([signal, deadline]) : deadline;
    while (!bounded.aborted) {
      let inventory;
      try { inventory = await this.#inventory(bounded); } catch (error) {
        if (bounded.aborted) break;
        throw error;
      }
      const action = this.#select(inventory, previous);
      if (action) return action;
      await new Promise(resolve => {
        const timer = setTimeout(done, 25);
        function done() { clearTimeout(timer); bounded.removeEventListener('abort', done); resolve(); }
        bounded.addEventListener('abort', done, { once: true });
      });
    }
    reject(signal?.aborted ? 'recipient_cancelled' : 'recipient_deadline');
  }

  [inspect.custom]() { return 'Recipient([REDACTED run binding])'; }
  toJSON() { return '[REDACTED run binding]'; }
}
