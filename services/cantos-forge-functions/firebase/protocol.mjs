// Transport validation only. Text semantics live in the Rust library.
export const SCHEMA_VERSION = "cantos-forge.worker/v1";
export const MAX_REQUEST_BYTES = 32_768;
export const MAX_TEXT_BYTES = 16_384;
export const MAX_REPLY_BYTES = 8_192;

export class BoundaryError extends Error {
  constructor(code) {
    super(code);
    this.name = "BoundaryError";
    this.code = code;
  }
}

function exactKeys(value, expected) {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    && Object.keys(value).sort().join(",") === [...expected].sort().join(",");
}

export function parseRequest(bytes) {
  if (!Buffer.isBuffer(bytes)) throw new BoundaryError("invalid_request");
  if (bytes.length > MAX_REQUEST_BYTES) throw new BoundaryError("request_too_large");
  let request;
  try {
    // TextDecoder's fatal mode refuses malformed UTF-8 instead of replacing its bytes.
    request = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes));
  } catch {
    throw new BoundaryError("invalid_request");
  }
  if (!exactKeys(request, ["schema_version", "request_id", "operation"])) {
    throw new BoundaryError("invalid_request");
  }
  if (request.schema_version !== SCHEMA_VERSION) throw new BoundaryError("unsupported_version");
  if (typeof request.request_id !== "string" || !/^[A-Za-z0-9_-]{1,64}$/.test(request.request_id)) {
    throw new BoundaryError("invalid_request_id");
  }
  const operation = request.operation;
  if (operation?.kind === "capabilities" && exactKeys(operation, ["kind"])) return request;
  if (operation?.kind === "check_text" && exactKeys(operation, ["kind", "text"])
      && typeof operation.text === "string") {
    if (Buffer.byteLength(operation.text, "utf8") > MAX_TEXT_BYTES) throw new BoundaryError("text_too_large");
    return request;
  }
  throw new BoundaryError("invalid_request");
}

const REFUSALS = new Set([
  "request_too_large", "invalid_request", "unsupported_version", "invalid_request_id",
  "text_too_large", "request_read_failed",
]);
const TEXT_VIOLATIONS = new Set(["blank", "byte_order_mark", "control_character"]);

export function parseReply(bytes, request, exitCode) {
  let reply;
  try {
    reply = JSON.parse(new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(bytes));
  } catch {
    throw new BoundaryError("invalid_worker_reply");
  }
  if (!exactKeys(reply, ["schema_version", "request_id", "result"])
      || reply.schema_version !== SCHEMA_VERSION) throw new BoundaryError("invalid_worker_reply");
  const result = reply.result;
  if (result?.kind === "rejected" && exactKeys(result, ["kind", "code"])
      && REFUSALS.has(result.code) && exitCode === 2
      && (reply.request_id === null || reply.request_id === request.request_id)) return reply;
  if (exitCode !== 0 || reply.request_id !== request.request_id) {
    throw new BoundaryError("invalid_worker_reply");
  }
  if (request.operation.kind === "capabilities" && result?.kind === "capabilities"
      && exactKeys(result, ["kind", "operations", "generation"])
      && JSON.stringify(result.operations) === '["capabilities","check_text"]'
      && result.generation === "unavailable") return reply;
  if (request.operation.kind === "check_text" && result?.kind === "text_checked"
      && exactKeys(result, ["kind", "check", "outcome", "detail"])
      && result.check === "utf8-text-v1" && ["passed", "failed"].includes(result.outcome)) {
    const detail = result.detail;
    if (exactKeys(detail, ["byte_length", "char_count", "violations"])
        && detail.byte_length === Buffer.byteLength(request.operation.text, "utf8")
        && Number.isSafeInteger(detail.char_count) && detail.char_count >= 0 && detail.char_count <= MAX_TEXT_BYTES
        && Array.isArray(detail.violations) && detail.violations.length <= 3
        && detail.violations.every((code) => TEXT_VIOLATIONS.has(code))) return reply;
  }
  throw new BoundaryError("invalid_worker_reply");
}
