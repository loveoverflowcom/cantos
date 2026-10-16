import { makeHttpHandler } from "./http.mjs";
import { runPackagedWorker } from "./worker.mjs";

// No Firebase trigger is registered. Deployment/ABI and trusted per-actor grants are separate
// acceptance gates. Installing firebase-functions alone must not expose producer operations.
export const forgeWorkerHttp = makeHttpHandler({
  authorize: async () => ({ kind: "unavailable" }),
  runWorker: runPackagedWorker,
});
