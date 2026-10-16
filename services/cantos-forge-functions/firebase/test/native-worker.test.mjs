// Real local Node -> Rust process evidence. The injected executable is a test-build artifact,
// never a caller field or production override. This does not establish Linux/Functions ABI.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { SCHEMA_VERSION } from "../protocol.mjs";
import { createMockRunnerForTests } from "../testing.mjs";

const NATIVE_WORKER = fileURLToPath(new URL("../../../../target/debug/cantos-forge-worker", import.meta.url));
const EXPECTED_PACKAGE_WORKER = fileURLToPath(new URL("../bin/cantos-forge-worker", import.meta.url));
const requestBytes = (operation) => Buffer.from(JSON.stringify({
  schema_version: SCHEMA_VERSION, request_id: "native-01", operation,
}));

function nativeRunner(onChild = () => {}) {
  return createMockRunnerForTests({
    spawnWorker: (executable, args, options) => {
      assert.equal(executable, EXPECTED_PACKAGE_WORKER);
      const child = spawn(NATIVE_WORKER, args, options);
      onChild(child);
      return child;
    },
    setTimer: setTimeout,
    clearTimer: clearTimeout,
  });
}

test("actual Node adapter and Rust worker agree on capabilities after process exit", { timeout: 10_000 }, async () => {
  let closed = false;
  const run = nativeRunner((child) => child.once("close", () => { closed = true; }));
  assert.deepEqual(await run(requestBytes({ kind: "capabilities" })), {
    schema_version: SCHEMA_VERSION, request_id: "native-01",
    result: { kind: "capabilities", operations: ["capabilities", "check_text"], generation: "unavailable" },
  });
  assert.equal(closed, true);
});

test("actual worker returns golden Vietnamese byte measurements through Node", { timeout: 10_000 }, async () => {
  const run = nativeRunner();
  assert.deepEqual(await run(requestBytes({ kind: "check_text", text: "Lời hẹn.\n" })), {
    schema_version: SCHEMA_VERSION, request_id: "native-01",
    result: { kind: "text_checked", check: "utf8-text-v1", outcome: "passed",
      detail: { byte_length: 13, char_count: 9, violations: [] } },
  });
  assert.deepEqual(await run(requestBytes({ kind: "check_text", text: "\n\t" })), {
    schema_version: SCHEMA_VERSION, request_id: "native-01",
    result: { kind: "text_checked", check: "utf8-text-v1", outcome: "failed",
      detail: { byte_length: 2, char_count: 2, violations: ["blank"] } },
  });
});

test("Rust rejects duplicate fields even when the outer JSON parser can read them", { timeout: 10_000 }, async () => {
  const bytes = Buffer.from('{"schema_version":"cantos-forge.worker/v1","request_id":"first","request_id":"native-01","operation":{"kind":"capabilities"}}');
  assert.deepEqual(await nativeRunner()(bytes), {
    schema_version: SCHEMA_VERSION, request_id: null,
    result: { kind: "rejected", code: "invalid_request" },
  });
});

test("aborting the real local child waits for its close before rejecting", { timeout: 10_000 }, async () => {
  let closed = false;
  const run = nativeRunner((child) => child.once("close", () => { closed = true; }));
  const abort = new AbortController();
  const pending = run(requestBytes({ kind: "capabilities" }), { signal: abort.signal });
  abort.abort();
  await assert.rejects(pending, (error) => error.code === "request_aborted");
  assert.equal(closed, true);
});
