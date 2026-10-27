import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { forgeProbeHttp, forgeWorkerHttp } from "../index.mjs";
import { PROBE_SCHEMA_VERSION, makeProbeHandler } from "../probe.mjs";
import { BoundaryError, MAX_REPLY_BYTES, MAX_REQUEST_BYTES, MAX_TEXT_BYTES, SCHEMA_VERSION, parseRequest } from "../protocol.mjs";
import { createMockAuthorizedHandlerForTests, createMockRunnerForTests } from "../testing.mjs";

const requestBytes = (operation = { kind: "capabilities" }, fields = {}) => Buffer.from(JSON.stringify({
  schema_version: SCHEMA_VERSION, request_id: "test-01", operation, ...fields,
}));
const capabilities = (fields = {}) => ({
  schema_version: SCHEMA_VERSION, request_id: "test-01",
  result: { kind: "capabilities", operations: ["capabilities", "check_text"], generation: "unavailable" }, ...fields,
});

function fixture() {
  const child = new EventEmitter();
  child.stdin = new EventEmitter();
  child.stdin.end = (bytes) => { child.input = Buffer.from(bytes); };
  child.stdout = new EventEmitter();
  child.stderr = new EventEmitter();
  child.signals = [];
  child.kill = (signal) => { child.signals.push(signal); return true; };
  const timers = [];
  const spawns = [];
  const run = createMockRunnerForTests({
    spawnWorker: (...args) => { spawns.push(args); return child; },
    setTimer: (callback, ms) => { const timer = { callback, ms, cleared: false }; timers.push(timer); return timer; },
    clearTimer: (timer) => { if (timer) timer.cleared = true; },
  });
  const finish = (reply = capabilities(), code = 0, signal = null) => {
    child.stdout.emit("data", Buffer.from(JSON.stringify(reply)));
    child.emit("close", code, signal);
  };
  return { child, run, spawns, timers, finish };
}

const codeIs = (code) => (error) => error.code === code && error.message === code;

test("only finite exact request shapes are accepted before spawning", async () => {
  const f = fixture();
  for (const field of ["executable", "argv", "env", "root", "workspace", "credential", "unexpected"]) {
    await assert.rejects(f.run(requestBytes(undefined, { [field]: "private" })), codeIs("invalid_request"));
    await assert.rejects(f.run(requestBytes({ kind: "capabilities", [field]: "private" })), codeIs("invalid_request"));
  }
  for (const kind of ["generate", "status", "publish", "sync", "shell"]) {
    await assert.rejects(f.run(requestBytes({ kind })), codeIs("invalid_request"));
  }
  assert.equal(f.spawns.length, 0);
});

test("version identity encoding and byte limits are checked exactly", () => {
  assert.throws(() => parseRequest(requestBytes(undefined, { schema_version: "v2" })), codeIs("unsupported_version"));
  for (const id of ["", "a b", "Nguyễn", "a\n", "a".repeat(65)]) {
    assert.throws(() => parseRequest(requestBytes(undefined, { request_id: id })), codeIs("invalid_request_id"));
  }
  for (const bytes of [Buffer.from("{}"), Buffer.from("{"), Buffer.from("null"), Buffer.from([0xff]), Buffer.from("{} {}"), Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), requestBytes()])]) {
    assert.throws(() => parseRequest(bytes), codeIs("invalid_request"));
  }
  const atTextLimit = requestBytes({ kind: "check_text", text: "ệ".repeat(5_461) + "x" });
  assert.equal(parseRequest(atTextLimit).operation.text.length, 5_462);
  assert.throws(() => parseRequest(requestBytes({ kind: "check_text", text: "x".repeat(MAX_TEXT_BYTES + 1) })), codeIs("text_too_large"));
  const padded = Buffer.concat([requestBytes(), Buffer.alloc(MAX_REQUEST_BYTES - requestBytes().length, 32)]);
  assert.equal(parseRequest(padded).operation.kind, "capabilities");
  assert.throws(() => parseRequest(Buffer.concat([padded, Buffer.from(" ")])), codeIs("request_too_large"));
});

test("fixed executable argv environment and cwd cannot come from a request", async () => {
  const f = fixture();
  const bytes = requestBytes();
  const pending = f.run(bytes);
  const [executable, args, options] = f.spawns[0];
  assert.equal(executable.endsWith("/firebase/bin/cantos-forge-worker"), true);
  assert.deepEqual(args, []);
  assert.equal(options.shell, false);
  assert.deepEqual(options.env, { LANG: "C.UTF-8" });
  assert.equal(options.cwd.endsWith("/firebase/"), true);
  assert.deepEqual(f.child.input, bytes);
  f.finish();
  assert.deepEqual(await pending, capabilities());
  assert.equal(f.timers.every((timer) => timer.cleared), true);
});

test("a reply is never delivered until child close including after exit", async () => {
  const f = fixture();
  let settled = false;
  const pending = f.run(requestBytes()).finally(() => { settled = true; });
  f.child.stdout.emit("data", Buffer.from(JSON.stringify(capabilities())));
  f.child.emit("exit", 0, null);
  await Promise.resolve();
  assert.equal(settled, false);
  f.child.emit("close", 0, null);
  assert.deepEqual(await pending, capabilities());
});

test("deadline terminates escalates and waits for confirmed process exit", async () => {
  const f = fixture();
  let settled = false;
  const pending = f.run(requestBytes());
  const rejected = assert.rejects(pending.finally(() => { settled = true; }), codeIs("worker_deadline_exceeded"));
  assert.equal(f.timers[0].ms, 5_000);
  f.timers[0].callback();
  assert.deepEqual(f.child.signals, ["SIGTERM"]);
  assert.equal(f.timers[1].ms, 100);
  f.timers[1].callback();
  assert.deepEqual(f.child.signals, ["SIGTERM", "SIGKILL"]);
  await Promise.resolve();
  assert.equal(settled, false);
  f.child.emit("close", null, "SIGKILL");
  await rejected;
});

test("caller abort kills the child and a pre-aborted caller never spawns", async () => {
  const f = fixture();
  const abort = new AbortController();
  abort.abort();
  await assert.rejects(f.run(requestBytes(), { signal: abort.signal }), codeIs("request_aborted"));
  assert.equal(f.spawns.length, 0);
  const next = new AbortController();
  const pending = f.run(requestBytes(), { signal: next.signal });
  const rejected = assert.rejects(pending, codeIs("request_aborted"));
  next.abort();
  assert.deepEqual(f.child.signals, ["SIGTERM"]);
  f.child.emit("close", null, "SIGTERM");
  await rejected;
});

test("stdout and stderr are bounded and private stderr is never reflected", async () => {
  for (const stream of ["stdout", "stderr"]) {
    const f = fixture();
    const pending = f.run(requestBytes());
    const rejected = assert.rejects(pending, codeIs("worker_output_too_large"));
    f.child[stream].emit("data", Buffer.alloc(MAX_REPLY_BYTES + 1, 120));
    assert.deepEqual(f.child.signals, ["SIGTERM"]);
    f.child.emit("close", null, "SIGTERM");
    await rejected;
  }
  const f = fixture();
  const pending = f.run(requestBytes());
  f.child.stderr.emit("data", Buffer.from("private provider token"));
  f.finish(undefined, 1);
  await assert.rejects(pending, codeIs("worker_exit_failed"));
});

test("spawn stream and child failures have closed nonprivate errors", async () => {
  for (const [emitter, event, expected] of [
    ["child", "error", "worker_start_failed"], ["stdin", "error", "worker_input_failed"],
    ["stdout", "error", "worker_output_failed"], ["stderr", "error", "worker_output_failed"],
  ]) {
    const f = fixture();
    const pending = f.run(requestBytes());
    const rejected = assert.rejects(pending, codeIs(expected));
    (emitter === "child" ? f.child : f.child[emitter]).emit(event, new Error("private /operator/path"));
    f.child.emit("close", 1, null);
    await rejected;
  }
  const run = createMockRunnerForTests({ spawnWorker: () => { throw new Error("private"); }, setTimer: setTimeout, clearTimer: clearTimeout });
  await assert.rejects(run(requestBytes()), codeIs("worker_start_failed"));
  const f = fixture();
  f.child.stdin.end = () => { throw new Error("private path"); };
  const pending = f.run(requestBytes());
  const rejected = assert.rejects(pending, codeIs("worker_input_failed"));
  assert.deepEqual(f.child.signals, ["SIGTERM"]);
  f.child.emit("close", 1, null);
  await rejected;
});

test("mismatched unbounded or invented success replies fail closed", async () => {
  const badReplies = [
    capabilities({ schema_version: "v2" }), capabilities({ request_id: "other" }),
    capabilities({ extra: "private" }), capabilities({ result: { kind: "generated", artifact: "fake" } }),
    capabilities({ result: { kind: "capabilities", operations: ["generate"], generation: "available" } }),
    capabilities({ result: { kind: "capabilities", operations: ["capabilities", "check_text"], generation: "unavailable", env: {} } }),
  ];
  for (const reply of badReplies) {
    const f = fixture();
    const pending = f.run(requestBytes());
    f.finish(reply);
    await assert.rejects(pending, codeIs("invalid_worker_reply"));
  }
  const f = fixture();
  const pending = f.run(requestBytes());
  f.child.stdout.emit("data", Buffer.from("{} {}"));
  f.child.emit("close", 0, null);
  await assert.rejects(pending, codeIs("invalid_worker_reply"));
});

test("worker replies cannot add an encoding BOM that the Rust protocol never emits", async () => {
  const f = fixture();
  const pending = f.run(requestBytes());
  f.child.stdout.emit("data", Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from(JSON.stringify(capabilities()))]));
  f.child.emit("close", 0, null);
  await assert.rejects(pending, codeIs("invalid_worker_reply"));
});

test("Rust rejections and technical text failures stay distinct from generation or admission", async () => {
  const f = fixture();
  const pending = f.run(requestBytes({ kind: "check_text", text: " " }));
  const reply = { schema_version: SCHEMA_VERSION, request_id: "test-01", result: {
    kind: "text_checked", check: "utf8-text-v1", outcome: "failed",
    detail: { byte_length: 1, char_count: 1, violations: ["blank"] },
  } };
  f.finish(reply);
  assert.deepEqual(await pending, reply);
  const next = fixture();
  const rejected = next.run(requestBytes());
  const refusal = capabilities({ request_id: null, result: { kind: "rejected", code: "invalid_request" } });
  next.finish(refusal, 2);
  assert.deepEqual(await rejected, refusal);
});

function httpFixture(bytes = requestBytes(), method = "POST") {
  const request = new EventEmitter();
  Object.assign(request, { method, headers: { "content-type": "application/json" }, rawBody: bytes });
  const response = new EventEmitter();
  response.headers = {};
  response.writableEnded = false;
  response.setHeader = (name, value) => { response.headers[name] = value; };
  response.status = (status) => { response.statusCode = status; return response; };
  response.json = (body) => { response.body = body; response.writableEnded = true; return response; };
  return { request, response };
}

test("production HTTP export refuses caller auth claims with private no-cache responses", async () => {
  const f = httpFixture(requestBytes(undefined, { actor: "owner", authorized: true }));
  f.request.headers.authorization = "Bearer unverified";
  await forgeWorkerHttp(f.request, f.response);
  assert.equal(f.response.statusCode, 503);
  assert.deepEqual(f.response.body, { schema_version: SCHEMA_VERSION, error: "trusted_grant_unavailable" });
  assert.equal(f.response.headers["Cache-Control"], "private, no-store, max-age=0");
  assert.equal(f.response.headers.Pragma, "no-cache");
});

test("explicit mock HTTP composition enforces method content type and input before runner", async () => {
  let calls = 0;
  const handler = createMockAuthorizedHandlerForTests(async () => { calls += 1; return capabilities(); });
  for (const [method, type, bytes, status] of [
    ["GET", "application/json", requestBytes(), 405],
    ["POST", "text/plain", requestBytes(), 415],
    ["POST", "application/json", requestBytes(undefined, { root: "/tmp" }), 400],
    ["POST", "application/json", Buffer.alloc(MAX_REQUEST_BYTES + 1), 413],
  ]) {
    const f = httpFixture(bytes, method);
    f.request.headers["content-type"] = type;
    await handler(f.request, f.response);
    assert.equal(f.response.statusCode, status);
  }
  assert.equal(calls, 0);
  const f = httpFixture();
  await handler(f.request, f.response);
  assert.equal(f.response.statusCode, 200);
  assert.deepEqual(f.response.body, capabilities());
});

test("HTTP disconnect propagates abort and never emits a late result", async () => {
  let finish;
  let observedSignal;
  const handler = createMockAuthorizedHandlerForTests((bytes, { signal }) => {
    observedSignal = signal;
    return new Promise((resolve) => { finish = resolve; });
  });
  const f = httpFixture();
  const pending = handler(f.request, f.response);
  await Promise.resolve();
  f.response.emit("close");
  assert.equal(observedSignal.aborted, true);
  finish(capabilities());
  await pending;
  assert.equal(f.response.body, undefined);
  assert.equal(f.request.listenerCount("aborted"), 0);
  assert.equal(f.response.listenerCount("close"), 0);
});

test("an already disconnected HTTP caller never starts a child", async () => {
  let calls = 0;
  const handler = createMockAuthorizedHandlerForTests(async () => { calls += 1; return capabilities(); });
  const f = httpFixture();
  f.request.aborted = true;
  await handler(f.request, f.response);
  assert.equal(calls, 0);
  assert.equal(f.response.body, undefined);
});

function probeFixture(path, method = "GET") {
  const { request, response } = httpFixture(Buffer.alloc(0), method);
  request.path = path;
  return { request, response };
}
const probeCapabilities = { ...capabilities(), request_id: "probe-health" };
const probeBody = (status, extra = {}, revision = "forge-probe-00001-abc") => (
  { schema_version: PROBE_SCHEMA_VERSION, status, ...extra, revision });

test("ping answers live without running the worker", async () => {
  let calls = 0;
  const handler = makeProbeHandler({ runWorker: async () => { calls += 1; }, revision: "forge-probe-00001-abc" });
  const f = probeFixture("/ping");
  await handler(f.request, f.response);
  assert.equal(f.response.statusCode, 200);
  assert.deepEqual(f.response.body, probeBody("live"));
  assert.equal(f.response.headers["Cache-Control"], "private, no-store, max-age=0");
  assert.equal(calls, 0);
});

test("health is ready only from a validated capabilities run with a fixed request", async () => {
  let seen;
  const handler = makeProbeHandler({
    runWorker: async (bytes, options) => { seen = { bytes, options }; return probeCapabilities; },
    revision: "forge-probe-00001-abc",
  });
  const f = probeFixture("/health");
  f.request.headers["content-type"] = "application/json";
  f.request.rawBody = requestBytes({ kind: "check_text", text: "caller supplied" });
  await handler(f.request, f.response);
  assert.equal(f.response.statusCode, 200);
  assert.deepEqual(f.response.body, probeBody("ready", { worker: { generation: "unavailable" } }));
  assert.deepEqual(parseRequest(seen.bytes), {
    schema_version: SCHEMA_VERSION, request_id: "probe-health", operation: { kind: "capabilities" } });
  assert.equal(seen.options.signal.aborted, false);
});

test("health degrades to 503 with a closed code for every worker failure", async () => {
  const failures = [
    [() => { throw new BoundaryError("worker_start_failed"); }, "worker_start_failed"],
    [() => { throw new BoundaryError("worker_deadline_exceeded"); }, "worker_deadline_exceeded"],
    [() => { throw new BoundaryError("invalid_worker_reply"); }, "invalid_worker_reply"],
    [() => { throw new Error("private /operator/path token"); }, "worker_failed"],
    [() => ({ ...probeCapabilities, request_id: null, result: { kind: "rejected", code: "invalid_request" } }), "worker_not_ready"],
  ];
  for (const [run, error] of failures) {
    const f = probeFixture("/health");
    await makeProbeHandler({ runWorker: async () => run(), revision: undefined })(f.request, f.response);
    assert.equal(f.response.statusCode, 503);
    assert.deepEqual(f.response.body, { schema_version: PROBE_SCHEMA_VERSION, status: "degraded", error, revision: null });
  }
});

test("probe routes accept only GET and only the two named paths", async () => {
  let calls = 0;
  const handler = makeProbeHandler({ runWorker: async () => { calls += 1; return probeCapabilities; } });
  for (const [path, method, status] of [
    ["/ping", "POST", 405], ["/health", "DELETE", 405], ["/health", "HEAD", 405],
    ["/", "GET", 404], ["/ping/", "GET", 404], ["/health/x", "GET", 404], ["/producer", "POST", 404],
  ]) {
    const f = probeFixture(path, method);
    await handler(f.request, f.response);
    assert.equal(f.response.statusCode, status, `${method} ${path}`);
    assert.equal(f.response.headers.Allow, status === 405 ? "GET" : undefined);
  }
  assert.equal(calls, 0);
});

test("a disconnected health probe aborts the worker and never emits a late result", async () => {
  let finish;
  let observed;
  const handler = makeProbeHandler({ runWorker: (bytes, { signal }) => { observed = signal; return new Promise((resolve) => { finish = resolve; }); } });
  const f = probeFixture("/health");
  const pending = handler(f.request, f.response);
  f.response.emit("close");
  assert.equal(observed.aborted, true);
  finish(probeCapabilities);
  await pending;
  assert.equal(f.response.statusCode, undefined);
});

test("revision is reflected only as a plain identifier and the probe never grants producer access", async () => {
  for (const revision of ["/srv/path", "a b", "x".repeat(129), "", 7, undefined]) {
    const f = probeFixture("/ping");
    await makeProbeHandler({ runWorker: async () => probeCapabilities, revision })(f.request, f.response);
    assert.equal(f.response.body.revision, null);
  }
  const f = probeFixture("/ping");
  await forgeProbeHttp(f.request, f.response);
  assert.equal(f.response.statusCode, 200);
  const producer = httpFixture(requestBytes());
  await forgeWorkerHttp(producer.request, producer.response);
  assert.equal(producer.response.statusCode, 503);
});
