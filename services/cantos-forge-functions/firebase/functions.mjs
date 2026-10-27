import { onRequest } from "firebase-functions/v2/https";
import config from "./probe-config.json" with { type: "json" };
import { forgeProbeHttp } from "./index.mjs";

// Only the run-owned package has this config. Import/discovery never starts a worker.
// The producer handler is deliberately not a Firebase trigger.
export const forgeSmokeProbe = onRequest({
  region: config.region,
  invoker: "private",
  cors: false,
  minInstances: 0,
  maxInstances: 1,
  concurrency: 1,
  cpu: 1,
  memory: "256MiB",
  timeoutSeconds: 10,
}, forgeProbeHttp);
