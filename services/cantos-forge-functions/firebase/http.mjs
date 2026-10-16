import { BoundaryError, SCHEMA_VERSION, parseRequest } from "./protocol.mjs";

// Production composition is in index.mjs. An authorization result is never read from the body.
export function makeHttpHandler({ authorize, runWorker }) {
  return async (request, response) => {
    response.setHeader("Cache-Control", "private, no-store, max-age=0");
    response.setHeader("Pragma", "no-cache");
    response.setHeader("X-Content-Type-Options", "nosniff");
    const send = (status, code) => response.status(status).json({ schema_version: SCHEMA_VERSION, error: code });
    let authorization;
    try { authorization = await authorize(request); }
    catch { send(503, "trusted_grant_unavailable"); return; }
    if (authorization?.kind !== "authorized") {
      send(authorization?.kind === "denied" ? 403 : 503,
        authorization?.kind === "denied" ? "forbidden" : "trusted_grant_unavailable");
      return;
    }
    if (request.method !== "POST") { response.setHeader("Allow", "POST"); send(405, "method_not_allowed"); return; }
    if (!/^application\/json(?:\s*;\s*charset=utf-8)?$/i.test(request.headers["content-type"] ?? "")) {
      send(415, "unsupported_media_type"); return;
    }
    try { parseRequest(request.rawBody); }
    catch (error) { send(error.code === "request_too_large" ? 413 : 400, error.code ?? "invalid_request"); return; }
    const abort = new AbortController();
    if (request.aborted || response.destroyed) return;
    const disconnected = () => { if (!response.writableEnded) abort.abort(); };
    request.on("aborted", disconnected);
    response.on("close", disconnected);
    try {
      const reply = await runWorker(request.rawBody, { signal: abort.signal });
      if (!abort.signal.aborted) response.status(reply.result.kind === "rejected" ? 400 : 200).json(reply);
    } catch (error) {
      if (!abort.signal.aborted) {
        const code = error instanceof BoundaryError ? error.code : "worker_failed";
        send(code === "worker_deadline_exceeded" ? 504 : 502, code);
      }
    } finally {
      request.removeListener("aborted", disconnected);
      response.removeListener("close", disconnected);
    }
  };
}
