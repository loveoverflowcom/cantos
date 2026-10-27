import { makeHttpHandler } from "./http.mjs";
import { makeProbeHandler } from "./probe.mjs";
import { runPackagedWorker } from "./worker.mjs";

// functions.mjs registers only the private deterministic probe. Producer operations stay
// unavailable even when the probe is deployed; no trigger exports this producer handler.
export const forgeProbeHttp = makeProbeHandler({
  runWorker: runPackagedWorker,
  revision: process.env.K_REVISION,
});

export const forgeWorkerHttp = makeHttpHandler({
  authorize: async () => ({ kind: "unavailable" }),
  runWorker: runPackagedWorker,
});
