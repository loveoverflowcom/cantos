import { BoundaryError, SCHEMA_VERSION } from "./protocol.mjs";

export const PROBE_SCHEMA_VERSION = "cantos-forge.probe/v1";
// Fixed, deterministic and secret-free. The probe never forwards a caller-supplied request.
const HEALTH_REQUEST_ID = "probe-health";
const HEALTH_REQUEST = Buffer.from(JSON.stringify({
  schema_version: SCHEMA_VERSION, request_id: HEALTH_REQUEST_ID, operation: { kind: "capabilities" },
}));
const REVISION_SHAPE = /^[A-Za-z0-9._-]{1,128}$/;

// `ping` proves only that the function answered. `health` is ready only when the packaged
// worker really ran `capabilities` and answered exactly as the protocol requires. Neither
// route authorizes, reaches producer operations or accepts a body; `forgeWorkerHttp` keeps
// its own grant gate. `revision` names the deployed revision and is dropped unless it has a
// plain identifier shape, so no runtime path or environment value is reflected.
export function makeProbeHandler({ runWorker, revision }) {
  const deployedRevision = typeof revision === "string" && REVISION_SHAPE.test(revision) ? revision : null;
  return async (request, response) => {
    response.setHeader("Cache-Control", "private, no-store, max-age=0");
    response.setHeader("Pragma", "no-cache");
    response.setHeader("X-Content-Type-Options", "nosniff");
    const send = (status, body) => response.status(status).json(
      { schema_version: PROBE_SCHEMA_VERSION, ...body, revision: deployedRevision });
    if (request.path !== "/ping" && request.path !== "/health") { send(404, { status: "not_found" }); return; }
    if (request.method !== "GET") { response.setHeader("Allow", "GET"); send(405, { status: "method_not_allowed" }); return; }
    if (request.path === "/ping") { send(200, { status: "live" }); return; }

    const abort = new AbortController();
    if (request.aborted || response.destroyed) return;
    const disconnected = () => { if (!response.writableEnded) abort.abort(); };
    request.on("aborted", disconnected);
    response.on("close", disconnected);
    try {
      const reply = await runWorker(HEALTH_REQUEST, { signal: abort.signal });
      if (abort.signal.aborted) return;
      if (reply.result.kind === "capabilities") send(200, { status: "ready", worker: { generation: reply.result.generation } });
      else send(503, { status: "degraded", error: "worker_not_ready" });
    } catch (error) {
      if (abort.signal.aborted) return;
      send(503, { status: "degraded", error: error instanceof BoundaryError ? error.code : "worker_failed" });
    } finally {
      request.removeListener("aborted", disconnected);
      response.removeListener("close", disconnected);
    }
  };
}
