// Copied beside functions/ by xtask. Runs the actual SDK trigger and fixed-path worker.
// Fault injection is confined to this disposable local bundle; never run against cloud files.
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once } from "node:events";
import { chmod, readFile, rename, writeFile, unlink } from "node:fs/promises";
import { setTimeout as pause } from "node:timers/promises";
import test, { after, before } from "node:test";
import { forgeSmokeProbe } from "./functions/functions.mjs";
import { forgeWorkerHttp } from "./functions/index.mjs";

const worker = new URL("./functions/bin/cantos-forge-worker", import.meta.url);
const saved = new URL("./functions/bin/worker.saved", import.meta.url);
let server;
let origin;
before(async () => {
  server = createServer((request, response) => {
    request.path = new URL(request.url, "http://localhost").pathname;
    response.status = function (code) { this.statusCode = code; return this; };
    response.json = function (value) {
      this.setHeader("Content-Type", "application/json");
      this.end(JSON.stringify(value)); return this;
    };
    const handler = request.path === "/producer" ? forgeWorkerHttp : forgeSmokeProbe;
    Promise.resolve(handler(request, response)).catch(() => { response.statusCode = 500; response.end(); });
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  origin = `http://127.0.0.1:${server.address().port}`;
});
after(async () => { server.closeAllConnections(); await new Promise((resolve) => server.close(resolve)); });

async function request(path, method = "GET") {
  const response = await fetch(origin + path, { method });
  return { status: response.status, headers: response.headers, body: await response.json() };
}
async function withFault(contents, exercise) {
  await rename(worker, saved);
  try {
    if (contents !== null) { await writeFile(worker, contents); await chmod(worker, 0o755); }
    await exercise();
  } finally {
    if (contents !== null) await unlink(worker);
    await rename(saved, worker);
  }
}

test("SDK discovery registers exactly a private resource-bounded Gen2 probe", async () => {
  const exports = await import("./functions/functions.mjs");
  assert.deepEqual(Object.keys(exports), ["forgeSmokeProbe"]);
  const endpoint = forgeSmokeProbe.__endpoint;
  assert.equal(endpoint.platform, "gcfv2");
  assert.deepEqual(endpoint.httpsTrigger.invoker, ["private"]);
  assert.equal(endpoint.concurrency, 1);
  assert.equal(endpoint.maxInstances, 1);
  assert.equal(endpoint.minInstances, 0);
  assert.equal(endpoint.timeoutSeconds, 10);
  assert.equal(endpoint.availableMemoryMb, 256);
  assert.equal(endpoint.region.length, 1);
});

test("actual repeated HTTP ping/health runs the bundled Rust capabilities", async () => {
  for (let attempt = 0; attempt < 3; attempt++) {
    const ping = await request("/ping");
    assert.equal(ping.status, 200);
    assert.equal(ping.body.status, "live");
    const health = await request("/health");
    assert.equal(health.status, 200);
    assert.deepEqual(health.body, { schema_version: "cantos-forge.probe/v1", status: "ready",
      worker: { generation: "unavailable" }, revision: null });
    assert.equal(health.headers.get("cache-control"), "private, no-store, max-age=0");
    assert.equal(health.headers.get("x-content-type-options"), "nosniff");
  }
});

test("actual HTTP wrong routes/methods and producer grant refusal", async () => {
  const method = await request("/health", "POST");
  assert.equal(method.status, 405);
  assert.equal(method.headers.get("allow"), "GET");
  assert.equal((await request("/unknown")).status, 404);
  const producer = await request("/producer", "POST");
  assert.equal(producer.status, 503);
  assert.equal(producer.body.error, "trusted_grant_unavailable");
});

test("missing binary degrades health while ping remains live", async () => {
  await withFault(null, async () => {
    assert.equal((await request("/ping")).status, 200);
    const reply = await request("/health");
    assert.equal(reply.status, 503);
    assert.equal(reply.body.error, "worker_start_failed");
    assert.equal(JSON.stringify(reply.body).includes("bin/"), false);
  });
  assert.equal((await request("/health")).status, 200);
});

test("malformed reply and private stderr never become a healthy or leaked response", async () => {
  await withFault("#!/bin/sh\nprintf 'synthetic-private-marker' >&2\nprintf 'broken'\n", async () => {
    const reply = await request("/health");
    assert.equal(reply.status, 503);
    assert.equal(reply.body.error, "invalid_worker_reply");
    assert.equal(JSON.stringify(reply.body).includes("synthetic-private-marker"), false);
  });
});

test("real stuck subprocess reaches deadline then closes before response; recovery works", { timeout: 15_000 }, async () => {
  await withFault("#!/bin/sh\nexec /bin/sleep 30\n", async () => {
    const reply = await request("/health");
    assert.equal(reply.status, 503);
    assert.equal(reply.body.error, "worker_deadline_exceeded");
  });
  assert.equal((await request("/health")).status, 200);
});

test("disconnecting a real HTTP health call closes its running child and allows retry", { timeout: 10_000 }, async () => {
  const marker = new URL("./functions/probe-child.pid", import.meta.url);
  async function until(observe) {
    const deadline = Date.now() + 2_000;
    while (Date.now() < deadline) {
      const result = await observe();
      if (result) return result;
      await pause(5);
    }
    assert.fail("observable child lifecycle deadline exceeded");
  }
  await withFault("#!/bin/sh\necho $$ > ./probe-child.pid\nexec /bin/sleep 30\n", async () => {
    const controller = new AbortController();
    const pending = fetch(origin + "/health", { signal: controller.signal });
    const refused = assert.rejects(pending, (error) => error.name === "AbortError");
    try {
      const pid = await until(async () => {
        try { return Number(await readFile(marker, "utf8")); }
        catch (error) { if (error.code === "ENOENT") return false; throw error; }
      });
      assert.ok(Number.isSafeInteger(pid) && pid > 1);
      controller.abort();
      await refused;
      await until(() => {
        try { process.kill(pid, 0); return false; }
        catch (error) { if (error.code === "ESRCH") return true; throw error; }
      });
    } finally {
      controller.abort();
      await unlink(marker).catch((error) => { if (error.code !== "ENOENT") throw error; });
    }
  });
  assert.equal((await request("/health")).status, 200);
});
