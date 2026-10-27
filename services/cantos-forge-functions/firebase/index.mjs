import { makeHttpHandler } from "./http.mjs";
import { makeProbeHandler } from "./probe.mjs";
import { runPackagedWorker } from "./worker.mjs";

// No Firebase trigger is registered. Deployment/ABI and trusted per-actor grants are separate
// acceptance gates. Installing firebase-functions alone must not expose producer operations.
// `forgeProbeHttp` is the grant-free deterministic ping/health slice for #486; wrapping it in
// a Gen2 trigger (private invoker by default) is the next, operator-gated step.
export const forgeProbeHttp = makeProbeHandler({
  runWorker: runPackagedWorker,
  revision: process.env.K_REVISION,
});

export const forgeWorkerHttp = makeHttpHandler({
  authorize: async () => ({ kind: "unavailable" }),
  runWorker: runPackagedWorker,
});
