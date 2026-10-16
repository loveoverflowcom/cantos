import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { BoundaryError, MAX_REPLY_BYTES, parseReply, parseRequest } from "./protocol.mjs";

const PACKAGE_ROOT = fileURLToPath(new URL("./", import.meta.url));
const PACKAGED_WORKER = fileURLToPath(new URL("./bin/cantos-forge-worker", import.meta.url));
const DEADLINE_MS = 5_000;
const TERMINATION_GRACE_MS = 100;
const MAX_STDERR_BYTES = 8_192;

// This path is fixed by the package. No request or environment variable can override it.
export const runPackagedWorker = makeRunner({ spawnWorker: spawn, setTimer: setTimeout, clearTimer: clearTimeout });

// Only testing.mjs imports this dependency seam; the HTTP export uses the fixed runner above.
export function makeRunner({ spawnWorker, setTimer, clearTimer }) {
  return async (bytes, { signal } = {}) => {
    const request = parseRequest(bytes);
    if (signal?.aborted) throw new BoundaryError("request_aborted");
    return new Promise((resolve, reject) => {
      let child;
      try {
        child = spawnWorker(PACKAGED_WORKER, [], {
          cwd: PACKAGE_ROOT,
          env: { LANG: "C.UTF-8" },
          shell: false,
          stdio: ["pipe", "pipe", "pipe"],
          windowsHide: true,
        });
      } catch {
        reject(new BoundaryError("worker_start_failed"));
        return;
      }
      let failure;
      let deadline;
      let escalation;
      let stdoutBytes = 0;
      let stderrBytes = 0;
      const chunks = [];
      let closed = false;
      const terminate = (code) => {
        if (closed || failure) return;
        failure = new BoundaryError(code);
        child.kill("SIGTERM");
        escalation = setTimer(() => { if (!closed) child.kill("SIGKILL"); }, TERMINATION_GRACE_MS);
      };
      const abort = () => terminate("request_aborted");
      child.on("error", () => terminate("worker_start_failed"));
      child.stdin.on("error", () => terminate("worker_input_failed"));
      child.stdout.on("error", () => terminate("worker_output_failed"));
      child.stderr.on("error", () => terminate("worker_output_failed"));
      child.stdout.on("data", (chunk) => {
        stdoutBytes += chunk.length;
        if (stdoutBytes > MAX_REPLY_BYTES) terminate("worker_output_too_large");
        else if (!failure) chunks.push(Buffer.from(chunk));
      });
      // Never surface stderr: it can contain private data or platform paths.
      child.stderr.on("data", (chunk) => {
        stderrBytes += chunk.length;
        if (stderrBytes > MAX_STDERR_BYTES) terminate("worker_output_too_large");
      });
      child.on("close", (code, killedBy) => {
        closed = true;
        clearTimer(deadline);
        clearTimer(escalation);
        signal?.removeEventListener("abort", abort);
        if (failure) { reject(failure); return; }
        if (killedBy !== null || ![0, 2].includes(code)) {
          reject(new BoundaryError("worker_exit_failed"));
          return;
        }
        try { resolve(parseReply(Buffer.concat(chunks), request, code)); }
        catch (error) { reject(error); }
      });
      deadline = setTimer(() => terminate("worker_deadline_exceeded"), DEADLINE_MS);
      signal?.addEventListener("abort", abort, { once: true });
      if (signal?.aborted) abort();
      if (!failure) {
        try { child.stdin.end(bytes); }
        catch { terminate("worker_input_failed"); }
      }
    });
  };
}
